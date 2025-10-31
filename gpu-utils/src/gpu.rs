use fxhash::FxBuildHasher;
use log::{debug, error, trace, warn};
use parking_lot::{Mutex, RwLock};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

/// Descriptor used to configure and create a `Gpu` instance.
pub struct GpuDescriptor {
    /// Which wgpu backends to enable.
    pub backends: wgpu::Backends,
    /// Power preference for adapter selection.
    pub power_preference: wgpu::PowerPreference,
    /// Features that must be available on the device.
    pub required_features: wgpu::Features,
    /// Optional device limits to request. If `None`, the adapter's limits are used.
    pub required_limits: Option<wgpu::Limits>,
    /// Preferred surface format for swapchains or surfaces created using this GPU.
    pub preferred_surface_format: wgpu::TextureFormat,

    /// When true will attempt automatic recovery after device lost.
    pub auto_recover_enabled: bool,
}

impl Default for GpuDescriptor {
    fn default() -> Self {
        Self {
            backends: wgpu::Backends::PRIMARY,
            power_preference: wgpu::PowerPreference::LowPower,
            required_features: wgpu::Features::empty(),
            required_limits: None,
            preferred_surface_format: wgpu::TextureFormat::Bgra8UnormSrgb,
            auto_recover_enabled: false,
        }
    }
}

static CALLBACK_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallbackId {
    id: u64,
}
#[allow(clippy::new_without_default)]
impl CallbackId {
    pub fn new() -> Self {
        let id = CALLBACK_ID.fetch_add(1, Ordering::Relaxed);
        Self { id }
    }
}

/// High-level GPU wrapper that owns a `wgpu::Instance`, chosen adapter and the current
/// device/queue pair. This type also manages device-lost detection and optional recovery.
#[allow(clippy::type_complexity)]
pub struct Gpu {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,

    features: wgpu::Features,
    limits: wgpu::Limits,

    preferred_surface_format: wgpu::TextureFormat,

    device_queue: RwLock<GpuDeviceQueue>,

    device_lost: AtomicBool,
    device_lost_details: RwLock<Option<(wgpu::DeviceLostReason, String)>>,
    device_lost_callback: Mutex<
        HashMap<
            CallbackId,
            Arc<dyn Fn(&wgpu::DeviceLostReason, &str) + Send + Sync>,
            FxBuildHasher,
        >,
    >,

    /// Whether automatic recovery is allowed
    auto_recover_enabled: AtomicBool,
    /// Whether a recovery attempt is currently running
    is_recovering: AtomicBool,
    device_recover_callback: Mutex<
        HashMap<CallbackId, Arc<dyn Fn(wgpu::Device, wgpu::Queue) + Send + Sync>, FxBuildHasher>,
    >,
    device_recover_failed_callback: Mutex<
        HashMap<CallbackId, Arc<dyn Fn(&wgpu::RequestDeviceError) + Send + Sync>, FxBuildHasher>,
    >,

    weak_self: Weak<Gpu>,
}

struct GpuDeviceQueue {
    device: wgpu::Device,
    queue: wgpu::Queue,
}

/* ----------------------
Public API (constructors / getters)
---------------------- */
impl Gpu {
    /// Create a new `Gpu` from descriptor.
    ///
    /// This validates required features against the chosen adapter, requests a device and queue,
    /// installs device-level handlers (device-lost and uncaptured error) and returns an `Arc<Gpu>`.
    pub async fn new(desc: GpuDescriptor) -> Result<Arc<Self>, GpuError> {
        let GpuDescriptor {
            backends,
            power_preference,
            required_features,
            required_limits,
            preferred_surface_format,
            auto_recover_enabled,
        } = desc;

        trace!(
            "Gpu::new: creating instance with backends={backends:?}, power_preference={power_preference:?}, auto_recover_enabled={auto_recover_enabled}"
        );
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        trace!("Gpu::new: requesting adapter");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await?;
        debug!("Gpu::new: adapter received: {:#?}", adapter.get_info());

        // Validate features requested by user are supported by the adapter.
        let adapter_features = adapter.features();
        if !adapter_features.contains(required_features) {
            warn!(
                "Gpu::new: adapter does not support required features: required={required_features:?} available={adapter_features:?}"
            );
            return Err(GpuError::AdapterFeatureUnsupported);
        }

        // Determine limits (use adapter limits if not provided)
        let limits = required_limits.unwrap_or_else(|| adapter.limits());
        let features = required_features;
        trace!(
            "Gpu::new: requesting device with features={features:?}, limits={limits:?}, preferred_surface_format={preferred_surface_format:?}"
        );

        // Request device
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Gpu: request device"),
                required_features: features,
                required_limits: limits.clone(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        // Build Arc<Gpu> with cyclic weak reference so callbacks can upgrade to Arc<Gpu>.
        let arc_self = Arc::new_cyclic(|weak: &Weak<Gpu>| {
            let device_queue = RwLock::new(GpuDeviceQueue {
                device: device.clone(),
                queue: queue.clone(),
            });

            // Install callbacks on the initial device so device-lost and uncaptured errors are handled.
            Self::install_device_callbacks(&device, weak);
            Self::install_uncaptured_error_handler(&device);

            Self {
                instance,
                adapter,
                device_queue,
                features,
                limits,
                preferred_surface_format,
                device_lost: AtomicBool::new(false),
                device_lost_details: RwLock::new(None),
                device_lost_callback: Default::default(),
                auto_recover_enabled: AtomicBool::new(auto_recover_enabled),
                is_recovering: AtomicBool::new(false),
                device_recover_callback: Default::default(),
                device_recover_failed_callback: Default::default(),
                weak_self: weak.clone(),
            }
        });

        trace!("Gpu::new: device and queue successfully created");
        Ok(arc_self)
    }

    /// Add a callback to be invoked when the device is lost.
    pub fn add_device_lost_callback(
        &self,
        callback: impl Fn(&wgpu::DeviceLostReason, &str) + Send + Sync + 'static,
    ) -> CallbackId {
        let id = CallbackId::new();
        let callback: Arc<dyn Fn(&wgpu::DeviceLostReason, &str) + Send + Sync> = Arc::new(callback);
        self.device_lost_callback
            .lock()
            .insert(id, Arc::clone(&callback));
        id
    }

    /// Remove a previously added device-lost callback by its ID.
    pub fn remove_device_lost_callback(&self, id: CallbackId) {
        self.device_lost_callback.lock().remove(&id);
    }

    /// Remove all previously added device-lost callbacks.
    pub fn remove_all_device_lost_callbacks(&self) {
        self.device_lost_callback.lock().clear();
    }

    /// Add a callback to be invoked after successful device recovery.
    pub fn add_device_recover_callback(
        &self,
        callback: impl Fn(wgpu::Device, wgpu::Queue) + Send + Sync + 'static,
    ) -> CallbackId {
        let id = CallbackId::new();
        let callback: Arc<dyn Fn(wgpu::Device, wgpu::Queue) + Send + Sync> = Arc::new(callback);
        self.device_recover_callback
            .lock()
            .insert(id, Arc::clone(&callback));
        id
    }

    /// Remove a previously added device-recover callback by its ID.
    pub fn remove_device_recover_callback(&self, id: CallbackId) {
        self.device_recover_callback.lock().remove(&id);
    }

    /// Remove all previously added device-recover callbacks.
    pub fn remove_all_device_recover_callbacks(&self) {
        self.device_recover_callback.lock().clear();
    }

    /// Add a callback to be invoked when device recovery fails.
    pub fn add_device_recover_failed_callback(
        &self,
        callback: impl Fn(&wgpu::RequestDeviceError) + Send + Sync + 'static,
    ) -> CallbackId {
        let id = CallbackId::new();
        let callback: Arc<dyn Fn(&wgpu::RequestDeviceError) + Send + Sync> = Arc::new(callback);
        self.device_recover_failed_callback
            .lock()
            .insert(id, Arc::clone(&callback));
        id
    }

    /// Remove a previously added device-recover-failed callback by its ID.
    pub fn remove_device_recover_failed_callback(&self, id: CallbackId) {
        self.device_recover_failed_callback.lock().remove(&id);
    }

    /// Remove all previously added device-recover-failed callbacks.
    pub fn remove_all_device_recover_failed_callbacks(&self) {
        self.device_recover_failed_callback.lock().clear();
    }
}

impl Gpu {
    /// Execute closure with a consistent view of device+queue under a single read lock.
    ///
    /// This avoids races where caller clones device and queue separately and they get swapped in between.
    pub fn with_device_queue<R>(&self, f: impl FnOnce(&wgpu::Device, &wgpu::Queue) -> R) -> R {
        let guard = self.device_queue.read();
        f(&guard.device, &guard.queue)
    }

    /// Get reference to the underlying wgpu Instance.
    pub fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }

    /// Get reference to the chosen adapter.
    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    /// Clone and return the current device.
    pub fn device(&self) -> wgpu::Device {
        self.device_queue.read().device.clone()
    }

    /// Clone and return the current queue.
    pub fn queue(&self) -> wgpu::Queue {
        self.device_queue.read().queue.clone()
    }

    /// Get features requested at creation.
    pub fn features(&self) -> &wgpu::Features {
        &self.features
    }

    /// Get limits requested at creation.
    pub fn limits(&self) -> &wgpu::Limits {
        &self.limits
    }

    /// Return preferred surface format stored in the descriptor.
    pub fn preferred_surface_format(&self) -> wgpu::TextureFormat {
        self.preferred_surface_format
    }

    /// Query whether the device is currently marked lost.
    pub fn is_device_lost(&self) -> bool {
        self.device_lost.load(Ordering::Acquire)
    }

    /// If device is lost, return the reason if available.
    pub fn device_lost_reason(&self) -> Option<wgpu::DeviceLostReason> {
        self.device_lost_details
            .read()
            .as_ref()
            .map(|(reason, _)| *reason)
    }

    /// Return the recorded device-lost reason and its message, if available.
    pub fn device_lost_details(&self) -> Option<(wgpu::DeviceLostReason, String)> {
        self.device_lost_details
            .read()
            .as_ref()
            .map(|(reason, message)| (*reason, message.clone()))
    }

    /// Enable or disable automatic recovery on device-lost.
    pub fn enable_auto_recover(&self, enabled: bool) {
        self.auto_recover_enabled.store(enabled, Ordering::Release);
    }

    /// Query whether a recovery attempt is currently running.
    pub fn is_recovering(&self) -> bool {
        self.is_recovering.load(Ordering::Acquire)
    }
}

/* ----------------------
Private helpers and callback handlers
---------------------- */
impl Gpu {
    /// Install device-lost callback on the provided device.
    ///
    /// The callback will attempt to upgrade the provided weak pointer and call into
    /// `handle_device_lost` on success.
    fn install_device_callbacks(device: &wgpu::Device, weak: &Weak<Gpu>) {
        let weak_clone = weak.clone();
        device.set_device_lost_callback(move |reason, s| {
            debug!("Gpu::device lost callback triggered: reason={reason:?} message={s}");
            if let Some(gpu) = weak_clone.upgrade() {
                gpu.handle_device_lost(reason, s);
            }
        });
    }

    /// Install the uncaptured error handler for the device.
    ///
    /// This uses a boxed handler as required by wgpu.
    fn install_uncaptured_error_handler(device: &wgpu::Device) {
        device.on_uncaptured_error(Box::new(|err| {
            error!("gpu-utils: uncaptured wgpu error: {err:?}");
        }));
    }

    /// Internal handler executed when the device is lost.
    ///
    /// Responsibilities:
    /// - mark device as lost and store reason
    /// - call user-provided device_lost callback
    /// - if auto recovery is enabled, spawn a worker to request a new device and swap it in
    ///
    /// NOTE: Recovery uses a dedicated thread and `futures::executor::block_on` to avoid blocking
    /// the wgpu-internal callback thread.
    /// TODO: allow injecting an async executor instead of spawning a dedicated thread that blocks.
    fn handle_device_lost(&self, reason: wgpu::DeviceLostReason, s: String) {
        // Mark device as lost
        {
            self.device_lost.store(true, Ordering::Release);
            *self.device_lost_details.write() = Some((reason, s.clone()));
        }
        warn!("Gpu::handle_device_lost: device lost with reason={reason:?}, message={s}");

        // Call user callback if provided
        let callbacks: Vec<_> = self.device_lost_callback.lock().values().cloned().collect();
        for cb in callbacks {
            cb(&reason, &s);
        }

        // Attempt automatic recovery if enabled and not already recovering
        if self.auto_recover_enabled.load(Ordering::Acquire)
            && self
                .is_recovering
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
        {
            debug!("Gpu::handle_device_lost: starting recovery workflow");
            // Attempt recovery on a dedicated thread.
            let arc_self = self
                .weak_self
                .upgrade()
                .expect("`Gpu::handle_device_lost` takes &self, so weak_self must be valid");

            std::thread::spawn(move || {
                let result = futures::executor::block_on(arc_self.adapter.request_device(
                    &wgpu::DeviceDescriptor {
                        label: Some("Gpu: request device (recovery)"),
                        required_features: arc_self.features,
                        required_limits: arc_self.limits.clone(),
                        memory_hints: wgpu::MemoryHints::default(),
                        trace: wgpu::Trace::Off,
                    },
                ));

                match result {
                    Ok((new_device, new_queue)) => {
                        trace!("Gpu::handle_device_lost: recovery device acquired");
                        // Swap new device and queue under write lock.
                        {
                            let mut dq = arc_self.device_queue.write();
                            dq.device = new_device.clone();
                            dq.queue = new_queue.clone();
                        }

                        // Reinstall callbacks on the new device.
                        Self::install_device_callbacks(&new_device, &arc_self.weak_self);
                        Self::install_uncaptured_error_handler(&new_device);

                        // Reset lost flags.
                        arc_self.device_lost.store(false, Ordering::Release);
                        *arc_self.device_lost_details.write() = None;

                        // Mark recovery finished.
                        arc_self.is_recovering.store(false, Ordering::Release);
                        debug!("Gpu::handle_device_lost: recovery completed successfully");

                        // Invoke recovery callback if provided.
                        let callbacks: Vec<_> = arc_self
                            .device_recover_callback
                            .lock()
                            .values()
                            .cloned()
                            .collect();
                        for cb in callbacks {
                            cb(new_device.clone(), new_queue.clone());
                        }
                    }
                    Err(e) => {
                        error!("Gpu::handle_device_lost: recovery failed: {e:?}");
                        // Mark recovery finished.
                        arc_self.is_recovering.store(false, Ordering::Release);

                        let callbacks: Vec<_> = arc_self
                            .device_recover_failed_callback
                            .lock()
                            .values()
                            .cloned()
                            .collect();
                        for cb in callbacks {
                            cb(&e);
                        }
                    }
                }
            });
        } else {
            trace!("Gpu::handle_device_lost: auto recovery disabled or already in progress");
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum GpuError {
    #[error("Failed to request adapter")]
    AdapterRequestFailed(#[from] wgpu::RequestAdapterError),
    #[error("Adapter does not support required features")]
    AdapterFeatureUnsupported,
    #[error("Failed to request device")]
    DeviceRequestFailed(#[from] wgpu::RequestDeviceError),
}
