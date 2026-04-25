pub mod component;
pub mod context;
pub mod metrics;
pub mod sub_widgets;
pub mod widget;
pub mod window;

use dashmap::DashMap;
use parking_lot::Mutex;
use std::sync::{Arc, OnceLock, Weak};

use crate::adapter::{EventLoop, EventLoopProxy};
use crate::application::Application;
use crate::event::device_event::DeviceEvent;
use crate::event::raw_device_event::{RawDeviceEvent, RawDeviceId};
use crate::event::window_event::WindowEvent;
use crate::window::WindowId;

use component::{Component, ComponentPod};
use context::{AppContext, EventReceiver, EventSender, SharedCtx, UiContext};
use gpu_utils::texture_atlas::atlas_simple::atlas::TextureAtlas;
use shared_buffer::BufferContext;
use widget::{View, WidgetPod, WidgetUpdateError};
use window::AnyWindowWidgetInstance;

// ----------------------------------------------------------------------------
// TreeApp
// ----------------------------------------------------------------------------

/// Owns the component tree, drives widget reconciliation, and routes events
/// to the correct window widget instance.
///
/// Implements [`Application`] so it can be wrapped in an [`Adapter`](crate::adapter::Adapter):
///
/// ```rust,ignore
/// let gpu = /* initialize gpu_utils::gpu::Gpu */;
/// let app = TreeApp::new(MyComponent::new(), gpu);
/// Adapter::new(app).run_on_winit()?;
/// ```
pub struct TreeApp<C: Component> {
    /// GPU device / queue / instance.  Stored outside the lock so that
    /// `Application::render` (which takes `&self`) can access it without
    /// holding the state mutex.
    gpu: gpu_utils::gpu::Gpu,

    root: ComponentPod<C>,

    /// Built widget tree.  `None` until the first `create_window` / `buffer_updated`.
    widget_pod: Mutex<Option<WidgetPod>>,

    /// Weak registry keyed by [`WindowId`].
    /// The strong `Arc` lives inside [`WindowWidget`](window::WindowWidget);
    /// dropping a window from the view tree destroys the OS window automatically.
    window_registry: DashMap<WindowId, Weak<Mutex<dyn AnyWindowWidgetInstance>>>,

    event_sender: EventSender,

    /// Receiver end of the backend message channel.
    /// Wrapped in `Mutex<Option<>>` solely to satisfy `Sync` (`UnboundedReceiver: !Sync`).
    /// Extracted once in `init()` via `Mutex::get_mut()` — no runtime locking occurs.
    event_receiver: Mutex<Option<EventReceiver>>,

    /// Handle to the bridge task spawned in `init()`.
    /// Set once; `OnceLock` provides `Sync` without a runtime mutex.
    bridge_handle: OnceLock<tokio::task::JoinHandle<()>>,

    /// Shared texture atlas for widget rendering (format: Rgba8UnormSrgb).
    texture_atlas: std::sync::Arc<TextureAtlas>,
}

// ----------------------------------------------------------------------------
// Construction
// ----------------------------------------------------------------------------

impl<C: Component> TreeApp<C> {
    pub fn new(root: C, gpu: gpu_utils::gpu::Gpu) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        let (gpu_device, _) = gpu.context().unwrap();
        let texture_atlas = TextureAtlas::new(
            &gpu_device,
            wgpu::Extent3d {
                width: 4096,
                height: 4096,
                depth_or_array_layers: 4,
            },
            wgpu::TextureFormat::Rgba8UnormSrgb,
            TextureAtlas::DEFAULT_MARGIN_PX,
        );

        Self {
            gpu,
            root: ComponentPod::new(None, root),
            widget_pod: Mutex::new(None),
            window_registry: DashMap::new(),
            event_sender: EventSender::new(tx),
            event_receiver: Mutex::new(Some(EventReceiver::new(rx))),
            bridge_handle: OnceLock::new(),
            texture_atlas,
        }
    }

    /// Returns a cloned `Arc` to the inner component.
    ///
    /// The backend holds this `Arc` and writes to
    /// [`SharedValue`](shared_buffer::SharedValue) fields to update UI state.
    pub fn component(&self) -> Arc<C> {
        self.root.arc()
    }
}

// ----------------------------------------------------------------------------
// TreeAppInner core logic
// ----------------------------------------------------------------------------

impl<C: Component> TreeApp<C> {
    /// Drains pending component messages, rebuilds the view tree, and
    /// reconciles the widget tree.  Prunes dead window registry entries.
    fn run_update(
        &self,
        runtime: &tokio::runtime::Handle,
        event_loop: &dyn EventLoop,
        gpu: &gpu_utils::gpu::Gpu,
    ) {
        let gpu_instance = gpu.instance();
        let (gpu_device, gpu_queue) = gpu.context().unwrap();

        let shared = SharedCtx {
            runtime_handle: runtime,
            event_sender: &self.event_sender,
            window_registry: &self.window_registry,
            gpu_instance,
            gpu_device,
            gpu_queue,
            texture_atlas: self.texture_atlas.as_ref(),
        };
        let ctx = UiContext {
            shared: &shared,
            event_loop: Some(event_loop),
            window: None,
        };

        let view = self.root.view(&ctx);

        let mut widget_pod = self.widget_pod.lock();
        match &mut *widget_pod {
            None => {
                *widget_pod = Some(view.build(&ctx));
            }
            Some(pod) => {
                if let Err(WidgetUpdateError::TypeMismatch) = pod.try_update(&view, &ctx) {
                    *pod = view.build(&ctx);
                }
            }
        }

        // Prune dead window references left over from removed Window widgets.
        self.window_registry
            .retain(|_, weak| weak.strong_count() > 0);
    }
}

// ----------------------------------------------------------------------------
// Application impl
// ----------------------------------------------------------------------------

#[async_trait::async_trait]
impl<C: Component> Application for TreeApp<C> {
    type Command = TreeAppCommand<C::Message>;

    // -------------------------------------------------------------------------
    // Lifecycle
    // -------------------------------------------------------------------------

    fn init(
        &mut self,
        runtime: &tokio::runtime::Handle,
        proxy: Box<dyn EventLoopProxy<Self> + Send>,
        event_loop: &impl EventLoop,
    ) {
        // Extract the receiver without locking — safe because `init` has `&mut self`.
        let mut receiver = self
            .event_receiver
            .get_mut()
            .take()
            .expect("TreeApp::init called more than once");

        let buffer_ctx = BufferContext::global().clone();

        let handle = runtime.spawn(async move {
            loop {
                tokio::select! {
                    _ = buffer_ctx.notified() => {
                        proxy.send_command(TreeAppCommand::BufferUpdated);
                    }
                    msg = receiver.recv() => match msg {
                        Some(boxed) => {
                            if let Ok(m) = boxed.downcast::<C::Message>() {
                                proxy.send_command(TreeAppCommand::BackendMessage(*m));
                            }
                        }
                        None => break,
                    }
                }
            }
        });

        self.bridge_handle.set(handle).ok();

        let ctx = AppContext {
            runtime_handle: runtime,
            event_sender: &self.event_sender,
            event_loop,
        };
        self.root.init(&ctx);
    }

    fn resumed(&self, runtime: &tokio::runtime::Handle, event_loop: &impl EventLoop) {
        let ctx = AppContext {
            runtime_handle: runtime,
            event_sender: &self.event_sender,
            event_loop: event_loop,
        };
        self.root.resumed(&ctx);
    }

    /// Builds the initial widget tree (creates OS windows declared in the view).
    ///
    /// Called by [`Adapter`](crate::adapter::Adapter) immediately after `resumed`.
    fn create_surface(&self, runtime: &tokio::runtime::Handle, event_loop: &impl EventLoop) {
        self.run_update(runtime, event_loop, &self.gpu);
    }

    /// Drops the entire widget tree, which destroys all OS windows via `Arc` ref-counting.
    ///
    /// Dead `Weak` entries in the window registry are pruned on the next
    /// `create_window` / `buffer_updated` call.
    fn destroy_surface(&self, _runtime: &tokio::runtime::Handle, _event_loop: &impl EventLoop) {
        *self.widget_pod.lock() = None;
    }

    fn suspended(&self, runtime: &tokio::runtime::Handle, event_loop: &impl EventLoop) {
        let ctx = AppContext {
            runtime_handle: runtime,
            event_sender: &self.event_sender,
            event_loop: event_loop,
        };
        self.root.suspended(&ctx);
    }

    fn exiting(&self, runtime: &tokio::runtime::Handle, event_loop: &impl EventLoop) {
        let ctx = AppContext {
            runtime_handle: runtime,
            event_sender: &self.event_sender,
            event_loop: event_loop,
        };
        self.root.exiting(&ctx);
    }

    // -------------------------------------------------------------------------
    // Rendering
    // -------------------------------------------------------------------------

    /// Renders a single window by walking its widget tree and collecting a
    /// [`RenderNode`](renderer::RenderNode).
    ///
    /// GPU surface submission is not yet implemented (see TODO below).
    async fn render(&self, runtime: &tokio::runtime::Handle, window_id: WindowId) {
        let op_arc = self
            .window_registry
            .get(&window_id)
            .and_then(|w| w.upgrade());

        if let Some(arc) = op_arc {
            let gpu_instance = self.gpu.instance();
            let (gpu_device, gpu_queue) = self.gpu.context().unwrap();

            let shared = SharedCtx {
                runtime_handle: runtime,
                event_sender: &self.event_sender,
                window_registry: &self.window_registry,
                gpu_instance,
                gpu_device,
                gpu_queue,
                texture_atlas: self.texture_atlas.as_ref(),
            };
            let ctx = UiContext {
                shared: &shared,
                event_loop: None,
                window: None,
            };

            let mut instance = arc.lock();
            let size = instance.size();
            let _render_node = instance.render(size, &ctx);
            // TODO: submit _render_node to the window's wgpu surface.
            // Requires extracting the surface from `WindowWidgetInstance` and
            // running the renderer pipeline via `self.gpu`.
        }
    }

    // -------------------------------------------------------------------------
    // Events
    // -------------------------------------------------------------------------

    fn window_event(
        &self,
        runtime: &tokio::runtime::Handle,
        event_loop: &impl EventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
    }

    fn window_destroyed(
        &self,
        runtime: &tokio::runtime::Handle,
        event_loop: &impl EventLoop,
        window_id: WindowId,
    ) {
        // TODO
    }

    /// Routes a device event to the widget tree of the target window.
    fn device_event(
        &self,
        runtime: &tokio::runtime::Handle,
        event_loop: &impl EventLoop,
        window_id: WindowId,
        event: DeviceEvent,
    ) {
        let handle = tokio::runtime::Handle::current();

        let op_arc = self
            .window_registry
            .get(&window_id)
            .and_then(|w| w.upgrade());

        if let Some(arc) = op_arc {
            let gpu_instance = self.gpu.instance();
            let (gpu_device, gpu_queue) = self.gpu.context().unwrap();

            let shared = SharedCtx {
                runtime_handle: runtime,
                event_sender: &self.event_sender,
                window_registry: &self.window_registry,
                gpu_instance,
                gpu_device,
                gpu_queue,
                texture_atlas: self.texture_atlas.as_ref(),
            };
            let ctx = UiContext {
                shared: &shared,
                event_loop: Some(event_loop),
                window: None,
            };

            let mut instance = arc.lock();
            instance.device_input(&event, &ctx);
        }
    }

    fn raw_device_event(
        &self,
        runtime: &tokio::runtime::Handle,
        event_loop: &impl EventLoop,
        raw_device_id: RawDeviceId,
        raw_event: RawDeviceEvent,
    ) {
        // TODO
    }

    // -------------------------------------------------------------------------
    // Ui commands
    // -------------------------------------------------------------------------

    fn ui_command(
        &self,
        runtime: &tokio::runtime::Handle,
        event_loop: &impl EventLoop,
        command: Self::Command,
    ) {
        match command {
            TreeAppCommand::BufferUpdated => {
                self.run_update(runtime, event_loop, &self.gpu);
            }
            TreeAppCommand::BackendMessage(msg) => {
                let gpu_instance = self.gpu.instance();
                let (gpu_device, gpu_queue) = self.gpu.context().unwrap();

                let shared = SharedCtx {
                    runtime_handle: runtime,
                    event_sender: &self.event_sender,
                    window_registry: &self.window_registry,
                    gpu_instance,
                    gpu_device,
                    gpu_queue,
                    texture_atlas: self.texture_atlas.as_ref(),
                };
                let ctx = UiContext {
                    shared: &shared,
                    event_loop: Some(event_loop),
                    window: None,
                };

                self.root.update(msg, &ctx);
            }
        }
    }
}

pub enum TreeAppCommand<Msg: Send + 'static> {
    BufferUpdated,
    BackendMessage(Msg),
}
