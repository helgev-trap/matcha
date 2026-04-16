pub mod component;
pub mod context;
pub mod metrics;
pub mod sub_widgets;
pub mod widget;
pub mod window;

use dashmap::DashMap;
use parking_lot::Mutex;
use std::sync::{Arc, Weak};

use crate::adapter::EventLoop;
use crate::application::Application;
use crate::event::device_event::DeviceEvent;
use crate::event::raw_device_event::{RawDeviceEvent, RawDeviceId};
use crate::event::window_event::WindowEvent;
use crate::window::WindowId;

use component::{Component, ComponentPod};
use context::{AppContext, EventReceiver, EventSender, RenderCtx, UiContext};
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

    state: Mutex<TreeAppInner<C>>,
}

// ----------------------------------------------------------------------------
// TreeAppInner — the mutable guts, protected by the Mutex above
// ----------------------------------------------------------------------------

struct TreeAppInner<C: Component> {
    root: ComponentPod<C>,

    /// Built widget tree.  `None` until the first `create_window` / `buffer_updated`.
    widget_pod: Option<WidgetPod>,

    /// Weak registry keyed by [`WindowId`].
    /// The strong `Arc` lives inside [`WindowWidget`](window::WindowWidget);
    /// dropping a window from the view tree destroys the OS window automatically.
    window_registry: DashMap<WindowId, Weak<Mutex<dyn AnyWindowWidgetInstance>>>,

    event_sender: EventSender,
    event_receiver: EventReceiver,
}

// ----------------------------------------------------------------------------
// Construction
// ----------------------------------------------------------------------------

impl<C: Component> TreeApp<C> {
    pub fn new(root: C, gpu: gpu_utils::gpu::Gpu) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        Self {
            gpu,
            state: Mutex::new(TreeAppInner {
                root: ComponentPod::new(None, root),
                widget_pod: None,
                window_registry: DashMap::new(),
                event_sender: EventSender::new(tx),
                event_receiver: EventReceiver::new(rx),
            }),
        }
    }

    /// Returns a cloned `Arc` to the inner component.
    ///
    /// The backend holds this `Arc` and writes to
    /// [`SharedValue`](shared_buffer::SharedValue) fields to update UI state.
    pub fn component(&self) -> Arc<C> {
        self.state.lock().root.arc()
    }
}

// ----------------------------------------------------------------------------
// TreeAppInner helpers — context construction
// ----------------------------------------------------------------------------

impl<C: Component> TreeAppInner<C> {
    fn app_ctx<'a>(&'a self, runtime: &'a tokio::runtime::Handle) -> AppContext<'a> {
        AppContext::new(runtime, &self.event_sender)
    }

    fn render_ctx<'a>(
        &'a self,
        runtime: &'a tokio::runtime::Handle,
        gpu: &'a gpu_utils::gpu::Gpu,
    ) -> RenderCtx<'a> {
        RenderCtx {
            runtime_handle: runtime,
            event_sender: &self.event_sender,
            gpu,
            window_registry: &self.window_registry,
        }
    }

    fn ui_ctx<'a>(
        &'a self,
        runtime: &'a tokio::runtime::Handle,
        event_loop: &'a dyn EventLoop,
        gpu: &'a gpu_utils::gpu::Gpu,
    ) -> UiContext<'a> {
        UiContext {
            render_ctx: self.render_ctx(runtime, gpu),
            event_loop,
        }
    }
}

// ----------------------------------------------------------------------------
// TreeAppInner core logic
// ----------------------------------------------------------------------------

impl<C: Component> TreeAppInner<C> {
    /// Drains pending component messages, rebuilds the view tree, and
    /// reconciles the widget tree.  Prunes dead window registry entries.
    fn run_update(
        &mut self,
        runtime: &tokio::runtime::Handle,
        event_loop: &dyn EventLoop,
        gpu: &gpu_utils::gpu::Gpu,
    ) {
        // Collect pending messages before building ctx so that
        // &mut self.event_receiver and the &self.event_sender borrow inside ctx
        // do not overlap.
        let mut msgs: Vec<C::Message> = Vec::new();
        while let Ok(raw) = self.event_receiver.try_recv() {
            if let Ok(msg) = raw.downcast::<C::Message>() {
                msgs.push(*msg);
            }
        }

        // Use an explicit struct literal so the borrow checker sees that ctx
        // borrows only self.event_sender and self.window_registry — not
        // self.widget_pod or self.root.  This allows the disjoint mutable
        // access to self.widget_pod below.
        let ctx = UiContext {
            render_ctx: RenderCtx {
                runtime_handle: runtime,
                event_sender: &self.event_sender,
                gpu,
                window_registry: &self.window_registry,
            },
            event_loop,
        };

        for msg in msgs {
            self.root.update(msg, &ctx);
        }

        let view = self.root.view(&ctx);

        match &mut self.widget_pod {
            None => {
                self.widget_pod = Some(view.build(&ctx));
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
    type Msg = C::Message;

    // -------------------------------------------------------------------------
    // Lifecycle
    // -------------------------------------------------------------------------

    fn init(&self, runtime: &tokio::runtime::Handle, _event_loop: &impl EventLoop) {
        let inner = self.state.lock();
        inner.root.init(&inner.app_ctx(runtime));
    }

    fn resumed(&self, runtime: &tokio::runtime::Handle, _event_loop: &impl EventLoop) {
        let inner = self.state.lock();
        inner.root.resumed(&inner.app_ctx(runtime));
    }

    /// Builds the initial widget tree (creates OS windows declared in the view).
    ///
    /// Called by [`Adapter`](crate::adapter::Adapter) immediately after `resumed`.
    fn create_surface(&self, runtime: &tokio::runtime::Handle, event_loop: &impl EventLoop) {
        let mut inner = self.state.lock();
        inner.run_update(runtime, event_loop, &self.gpu);
    }

    /// Drops the entire widget tree, which destroys all OS windows via `Arc` ref-counting.
    ///
    /// Dead `Weak` entries in the window registry are pruned on the next
    /// `create_window` / `buffer_updated` call.
    fn destroy_surface(&self, _runtime: &tokio::runtime::Handle, _event_loop: &impl EventLoop) {
        self.state.lock().widget_pod = None;
    }

    fn suspended(&self, runtime: &tokio::runtime::Handle, _event_loop: &impl EventLoop) {
        let inner = self.state.lock();
        inner.root.suspended(&inner.app_ctx(runtime));
    }

    fn exiting(&self, runtime: &tokio::runtime::Handle, _event_loop: &impl EventLoop) {
        let inner = self.state.lock();
        inner.root.exiting(&inner.app_ctx(runtime));
    }

    // -------------------------------------------------------------------------
    // Rendering
    // -------------------------------------------------------------------------

    /// Renders a single window by walking its widget tree and collecting a
    /// [`RenderNode`](renderer::RenderNode).
    ///
    /// GPU surface submission is not yet implemented (see TODO below).
    async fn render(&self, window_id: WindowId) {
        let handle = tokio::runtime::Handle::current();
        let inner = self.state.lock();

        let op_arc = inner
            .window_registry
            .get(&window_id)
            .and_then(|w| w.upgrade());

        if let Some(arc) = op_arc {
            let ctx = inner.render_ctx(&handle, &self.gpu);
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
        // TODO
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
        let inner = self.state.lock();

        let op_arc = inner
            .window_registry
            .get(&window_id)
            .and_then(|w| w.upgrade());

        if let Some(arc) = op_arc {
            let ctx = inner.ui_ctx(&handle, event_loop, &self.gpu);
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
    // User events
    // -------------------------------------------------------------------------

    /// Rebuilds the view tree after `SharedValue::store()` signals a change.
    fn buffer_updated(&self, runtime: &tokio::runtime::Handle, event_loop: &impl EventLoop) {
        let mut inner = self.state.lock();
        inner.run_update(runtime, event_loop, &self.gpu);
    }

    /// Delivers a typed message directly to the root component.
    ///
    /// Typically the component will call `SharedValue::store()` in response,
    /// which triggers a `BufferUpdated` event → `buffer_updated()` → full
    /// view rebuild.
    fn backend_message(&self, runtime: &tokio::runtime::Handle, event_loop: &impl EventLoop, msg: C::Message) {
        let inner = self.state.lock();
        let ctx = inner.ui_ctx(runtime, event_loop, &self.gpu);
        inner.root.update(msg, &ctx);
    }
}
