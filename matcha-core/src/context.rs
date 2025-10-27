use fxhash::FxBuildHasher;
use gpu_utils::gpu::Gpu;
use gpu_utils::gpu_type_map::GpuTypeMap;
use gpu_utils::texture_atlas::TextureAtlas;
use log::{debug, trace, warn};
use parking_lot::RwLock;
use parking_lot::lock_api::RwLockReadGuard;
use std::sync::{Arc, Weak};
use std::time::Duration;
use utils::type_map::TypeMap;

use crate::debug_config::DebugConfig;
use crate::window_surface::WindowSurface;

pub struct GlobalResources {
    gpu: Arc<Gpu>,
    texture: Arc<TextureAtlas>,
    stencil: Arc<TextureAtlas>,
    gpu_resource: Arc<GpuTypeMap>,
    any_resource: Arc<TypeMap>,

    current_time: Arc<RwLock<std::time::Instant>>,
    debug_config: Arc<RwLock<DebugConfig>>,

    command_receiver: tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<ApplicationCommand>>,
    command_sender: tokio::sync::mpsc::UnboundedSender<ApplicationCommand>,
}

impl GlobalResources {
    pub fn new(gpu: Arc<Gpu>) -> Self {
        debug!(
            "GlobalResources::new: initializing with max_texture_dimension_2d={}",
            gpu.limits().max_texture_dimension_2d
        );
        let max_size_2d = gpu.limits().max_texture_dimension_2d;
        let texture = TextureAtlas::new(
            &gpu.device(),
            wgpu::Extent3d {
                width: max_size_2d,
                height: max_size_2d,
                depth_or_array_layers: 1,
            },
            wgpu::TextureFormat::Rgba8UnormSrgb,
        );
        let stencil = TextureAtlas::new(
            &gpu.device(),
            wgpu::Extent3d {
                width: max_size_2d,
                height: max_size_2d,
                depth_or_array_layers: 1,
            },
            wgpu::TextureFormat::R8Unorm,
        );

        let gpu_resource = Arc::new(GpuTypeMap::new());
        let any_resource = Arc::new(TypeMap::new());

        let current_time = Arc::new(RwLock::new(std::time::Instant::now()));
        let debug_config = Arc::new(RwLock::new(DebugConfig::default()));

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        trace!("GlobalResources::new: command channel initialized");

        let resources = Self {
            gpu,
            texture,
            stencil,
            gpu_resource,
            any_resource,
            current_time,
            debug_config,
            command_receiver: tokio::sync::Mutex::new(rx),
            command_sender: tx,
        };

        trace!("GlobalResources::new: completed setup of shared resources");
        resources
    }
}

impl GlobalResources {
    pub fn gpu(&self) -> &Gpu {
        &self.gpu
    }

    pub fn texture_atlas(&self) -> &TextureAtlas {
        &self.texture
    }

    pub fn stencil_atlas(&self) -> &TextureAtlas {
        &self.stencil
    }

    pub fn gpu_resource(&self) -> &GpuTypeMap {
        &self.gpu_resource
    }

    pub fn any_resource(&self) -> &TypeMap {
        &self.any_resource
    }

    pub fn current_time(&self) -> Duration {
        self.current_time.read().elapsed()
    }

    pub(crate) fn debug_config(&self) -> RwLockReadGuard<'_, parking_lot::RawRwLock, DebugConfig> {
        self.debug_config.read()
    }

    pub fn try_recv_command(
        &self,
    ) -> Result<ApplicationCommand, tokio::sync::mpsc::error::TryRecvError> {
        self.command_receiver.blocking_lock().try_recv()
    }

    // pub fn command_receiver(
    //     &self,
    // ) -> &tokio::sync::mpsc::UnboundedReceiver<ApplicationCommand> {
    //     &self.command_receiver
    // }

    // pub fn command_sender(&self) -> &tokio::sync::mpsc::UnboundedSender<ApplicationCommand> {
    //     &self.command_sender
    // }
}

impl GlobalResources {
    pub fn widget_context(
        &self,
        task_executor: &tokio::runtime::Handle,
        window_surface: &Arc<RwLock<WindowSurface>>,
    ) -> Option<WidgetContext> {
        trace!("GlobalResources::widget_context: creating widget context");
        Some(WidgetContext {
            task_executor: task_executor.clone(),
            window_surface: Arc::downgrade(window_surface),
            current_time: Arc::downgrade(&self.current_time),
            debug_config: Arc::downgrade(&self.debug_config),
            gpu: Arc::downgrade(&self.gpu),
            texture_atlas: Arc::downgrade(&self.texture),
            stencil_atlas: Arc::downgrade(&self.stencil),
            gpu_resource: Arc::downgrade(&self.gpu_resource),
            any_resource: Arc::downgrade(&self.any_resource),
            scoped_config: AnyConfig::new(),
            window_id: window_surface.read().window_id(),
            command_sender: self.command_sender.downgrade(),
        })
    }

    pub fn application_context(
        &self,
        task_executor: &tokio::runtime::Handle,
        window_surface: &Arc<RwLock<WindowSurface>>,
    ) -> Option<ApplicationContext> {
        trace!("GlobalResources::application_context: creating application context");
        Some(ApplicationContext {
            task_executor: task_executor.clone(),
            window_surface: Arc::downgrade(window_surface),
            debug_config: Arc::downgrade(&self.debug_config),
            current_time: Arc::downgrade(&self.current_time),
            window_id: window_surface.read().window_id(),
            command_sender: self.command_sender.downgrade(),
        })
    }
}

/// Provides contextual information available to all widgets during their lifecycle.
///
/// This includes access to the GPU, window properties, shared resources, and timing information.
/// It is passed down the widget tree during layout and rendering.
#[derive(Clone)]
pub struct WidgetContext {
    // Stores weak references to avoid cyclic references and memory leaks.
    // But `GlobalResources` should outlive all `WidgetContext` instances.
    // So these `upgrade()` calls should always succeed.

    // async runtime
    task_executor: tokio::runtime::Handle,

    // ui rendering
    window_surface: Weak<RwLock<WindowSurface>>,
    current_time: Weak<RwLock<std::time::Instant>>,
    debug_config: Weak<RwLock<DebugConfig>>,

    // gpu resources
    gpu: Weak<Gpu>,
    texture_atlas: Weak<TextureAtlas>,
    stencil_atlas: Weak<TextureAtlas>,
    gpu_resource: Weak<GpuTypeMap>,
    any_resource: Weak<TypeMap>,

    // nested config
    scoped_config: AnyConfig,

    // current window id
    window_id: winit::window::WindowId,

    // commands (for create application context)
    command_sender: tokio::sync::mpsc::WeakUnboundedSender<ApplicationCommand>,
}

impl WidgetContext {
    pub(crate) fn application_context(&self) -> ApplicationContext {
        trace!(
            "WidgetContext::application_context: promoting widget context to application context"
        );
        ApplicationContext {
            task_executor: self.task_executor.clone(),
            window_surface: self.window_surface.clone(),
            debug_config: self.debug_config.clone(),
            current_time: self.current_time.clone(),
            window_id: self.window_id,
            command_sender: self.command_sender.clone(),
        }
    }
}

// todo: consider removing Option from return types of some of these methods
#[allow(clippy::unwrap_used)]
impl WidgetContext {
    /// Returns a reference to the WGPU device.
    pub fn device(&self) -> wgpu::Device {
        self.gpu.upgrade().unwrap().device()
    }

    /// Returns a reference to the WGPU queue.
    pub fn queue(&self) -> wgpu::Queue {
        self.gpu.upgrade().unwrap().queue()
    }

    /// Provides access to a type-safe, shared resource storage.
    pub fn any_resource(&self) -> Arc<TypeMap> {
        self.any_resource.upgrade().unwrap().clone()
    }

    /// Returns the texture format of the surface.
    pub fn surface_format(&self) -> Option<wgpu::TextureFormat> {
        self.window_surface
            .upgrade()
            .map(|surface| surface.read().format())
    }

    /// Returns the texture format for color used by the texture atlas.
    pub fn texture_format(&self) -> wgpu::TextureFormat {
        self.texture_atlas.upgrade().unwrap().format()
    }

    /// Returns a reference to the texture allocator.
    pub fn texture_atlas(&self) -> Arc<TextureAtlas> {
        self.texture_atlas.upgrade().unwrap().clone()
    }

    /// Returns the texture format for stencil used by the texture atlas.
    pub fn stencil_format(&self) -> wgpu::TextureFormat {
        self.stencil_atlas.upgrade().unwrap().format()
    }

    /// Returns a reference to the stencil atlas.
    pub fn stencil_atlas(&self) -> Arc<TextureAtlas> {
        self.stencil_atlas.upgrade().unwrap().clone()
    }

    /// Returns the DPI scaling factor of the window.
    pub fn dpi(&self) -> Option<f64> {
        self.window_surface
            .upgrade()
            .map(|surface| surface.read().dpi())
    }

    /// Returns the logical size of the viewport.
    pub fn viewport_size(&self) -> Option<[f32; 2]> {
        self.window_surface.upgrade().map(|surface| {
            let size = surface.read().inner_size();
            [size.width as f32, size.height as f32]
        })
    }

    /// Returns the current absolute time since the application started.
    pub fn current_time(&self) -> Duration {
        self.current_time.upgrade().unwrap().read().elapsed()
    }

    pub(crate) fn debug_config_always_rebuild_widget(&self) -> bool {
        self.debug_config
            .upgrade()
            .unwrap()
            .read()
            .always_rebuild_widget()
    }

    pub(crate) fn debug_config_disable_layout_measure_cache(&self) -> bool {
        self.debug_config
            .upgrade()
            .unwrap()
            .read()
            .disable_layout_measure_cache()
    }

    pub(crate) fn debug_config_disable_layout_arrange_cache(&self) -> bool {
        self.debug_config
            .upgrade()
            .unwrap()
            .read()
            .disable_layout_arrange_cache()
    }

    pub(crate) fn debug_config_disable_render_node_cache(&self) -> bool {
        self.debug_config
            .upgrade()
            .unwrap()
            .read()
            .disable_render_node_cache()
    }
}

/// ApplicationHandler is owned by the window / WinitInstance and holds the
/// shared command buffer. Components receive `ApplicationHandle` clones to
/// enqueue commands.
#[derive(Clone)]
pub struct ApplicationContext {
    task_executor: tokio::runtime::Handle,

    window_surface: Weak<RwLock<WindowSurface>>,
    debug_config: Weak<RwLock<DebugConfig>>,
    // todo: replace this by `Ticker`
    current_time: Weak<RwLock<std::time::Instant>>,

    window_id: winit::window::WindowId,

    command_sender: tokio::sync::mpsc::WeakUnboundedSender<ApplicationCommand>,
}

/// Commands that can be enqueued from components / handlers.
/// Extend this enum when new application-level commands are needed.
pub enum ApplicationCommand {
    /// Signal the application to exit.
    Exit,
    /// Close window with given ID.
    CloseWindow { id: winit::window::WindowId },
    // future: Custom(Box<dyn FnOnce(&mut AppState) + Send>), etc.
}

impl ApplicationContext {
    /// Enqueue a Exit command.
    /// This will signal the entire application to exit gracefully.
    pub fn exit(&self) {
        if let Some(sender) = self.command_sender.upgrade() {
            if sender.send(ApplicationCommand::Exit).is_err() {
                warn!("ApplicationContext::exit: receiver dropped before handling exit command");
            } else {
                trace!("ApplicationContext::exit: exit command sent");
            }
        } else {
            warn!("ApplicationContext::exit: command sender unavailable");
        }
    }

    pub fn close_current_window(&self) {
        if let Some(sender) = self.command_sender.upgrade()
            && let Ok(_) = sender.send(ApplicationCommand::CloseWindow { id: self.window_id })
        {
            trace!("ApplicationContext::close_current_window: close window command sent");
        } else {
            warn!("ApplicationContext::close_current_window: command sender unavailable");
        }
    }

    // future: push_custom, query_with_oneshot, etc.
}

#[derive(Default, Clone)]
pub(crate) struct AnyConfig {
    configs: std::collections::HashMap<
        std::any::TypeId,
        Arc<dyn std::any::Any + Send + Sync>,
        FxBuildHasher,
    >,
}

impl AnyConfig {
    pub fn new() -> Self {
        let configs = std::collections::HashMap::with_hasher(FxBuildHasher::default());
        trace!("AnyConfig::new: creating empty nested config store");
        Self { configs }
    }

    /// Insert a nested configuration of type T.
    pub fn set<T>(&mut self, config: T)
    where
        T: Send + Sync + 'static,
    {
        trace!(
            "AnyConfig::set: storing config type_id={:?}",
            std::any::TypeId::of::<T>()
        );
        self.configs
            .insert(std::any::TypeId::of::<T>(), Arc::new(config));
    }

    /// Retrieve a reference to a nested configuration of type T, if it exists.
    pub fn get<T>(&self) -> Option<&T>
    where
        T: Send + Sync + 'static,
    {
        self.configs
            .get(&std::any::TypeId::of::<T>())
            .and_then(|arc_any| arc_any.downcast_ref::<T>())
    }

    pub fn get_or_insert<T>(&mut self, v: T) -> &T
    where
        T: Send + Sync + 'static,
    {
        self.get_or_insert_with(|| v)
    }

    pub fn get_or_insert_default<T>(&mut self) -> &T
    where
        T: Default + Send + Sync + 'static,
    {
        self.get_or_insert_with(T::default)
    }

    pub fn get_or_insert_with<T, F>(&mut self, f: F) -> &T
    where
        T: Send + Sync + 'static,
        F: FnOnce() -> T,
    {
        self.configs
            .entry(std::any::TypeId::of::<T>())
            .or_insert_with(|| Arc::new(f()))
            .downcast_ref::<T>()
            .expect("Type map in `NestedConfig` should ensure `key == value.type_id()`")
    }
}

impl WidgetContext {
    /// Create a minimal WidgetContext suitable for unit tests.
    ///
    /// This constructs local placeholder resources and returns a WidgetContext that
    /// tests can use without a real GPU / GlobalResources. It intentionally uses
    /// weak references where appropriate so the returned context is self-contained.
    pub(crate) fn new_for_tests() -> Self {
        use parking_lot::RwLock as PLRwLock;
        use std::sync::Arc as StdArc;

        // task executor: prefer existing tokio handle, otherwise create a
        // dedicated current-thread runtime and leak it so the handle remains valid
        trace!("WidgetContext::new_for_tests: constructing test context");
        let task_executor: tokio::runtime::Handle = match tokio::runtime::Handle::try_current() {
            Ok(h) => h,
            Err(_) => {
                // Build a current-thread runtime and leak it for the test lifetime.
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to build test runtime");
                let handle = rt.handle().clone();
                // Enter the runtime to set it as the current reactor for this thread.
                // Leak both the runtime and the enter guard so the reactor stays active
                // for the lifetime of the test process.
                let guard = rt.enter();
                Box::leak(Box::new(guard));
                Box::leak(Box::new(rt));
                handle
            }
        };

        // window surface is not available in tests; use an empty Weak reference
        let window_surface_weak: std::sync::Weak<PLRwLock<crate::window_surface::WindowSurface>> =
            std::sync::Weak::new();

        let debug_cfg = StdArc::new(PLRwLock::new(crate::debug_config::DebugConfig::default()));
        let debug_cfg_weak = StdArc::downgrade(&debug_cfg);
        Box::leak(Box::new(debug_cfg));

        let current_time = StdArc::new(PLRwLock::new(std::time::Instant::now()));
        let current_time_weak = StdArc::downgrade(&current_time);
        Box::leak(Box::new(current_time));

        // Other shared resources: create Weak placeholders
        let gpu_weak = std::sync::Weak::new();
        let texture_atlas_weak = std::sync::Weak::new();
        let stencil_atlas_weak = std::sync::Weak::new();
        let gpu_resource_weak = std::sync::Weak::new();
        let any_resource_weak = std::sync::Weak::new();

        // command sender/receiver pair for test context
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel::<ApplicationCommand>();
        let command_sender_weak = tx.downgrade();
        // keep sender alive so WeakUnboundedSender::upgrade() succeeds in tests
        Box::leak(Box::new(tx));

        WidgetContext {
            task_executor,
            window_surface: window_surface_weak,
            current_time: current_time_weak,
            debug_config: debug_cfg_weak,
            gpu: gpu_weak,
            texture_atlas: texture_atlas_weak,
            stencil_atlas: stencil_atlas_weak,
            gpu_resource: gpu_resource_weak,
            any_resource: any_resource_weak,
            scoped_config: AnyConfig::new(),
            window_id: winit::window::WindowId::dummy(),
            command_sender: command_sender_weak,
        }
    }
}
