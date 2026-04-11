use std::sync::{Arc, Mutex, Weak};

use crate::application::ApplicationControler;
use crate::event::device_event::DeviceEvent;
use crate::event::raw_device_event::{RawDeviceEvent, RawDeviceId};
use crate::event::window_event::WindowEvent;
use crate::event_sender::EventSender;
use crate::window::{WindowConfig, WindowError, WindowId};
use crate::window_manager::{WindowHandle, WindowManager};

pub mod component;
pub mod metrics;
pub mod widget;
pub mod window;

use component::{Component, ComponentPod};
use dashmap::DashMap;
use widget::{View, WidgetPod, WidgetUpdateError};
use window::AnyWindowWidgetInstance;

// ----------------------------------------------------------------------------
// AppContext
// ----------------------------------------------------------------------------

pub struct AppContext<'a> {
    runtime_handle: &'a tokio::runtime::Handle,
    event_sender: &'a EventSender,
    app_ctrl: &'a dyn ApplicationControler,
}

impl AppContext<'_> {
    pub fn runtime_handle(&self) -> tokio::runtime::Handle {
        self.runtime_handle.clone()
    }

    pub fn event_sender(&self) -> EventSender {
        self.event_sender.clone()
    }

    pub fn emit_event(&self, event: Box<dyn std::any::Any + Send>) {
        self.event_sender.emit(event);
    }
}

// ----------------------------------------------------------------------------
// UiArchContext — concrete UiContext used during tree traversal
// ----------------------------------------------------------------------------

pub struct UiContext<'a> {
    runtime_handle: &'a tokio::runtime::Handle,
    event_sender: &'a EventSender,
    app_ctrl: &'a dyn ApplicationControler,
    gpu: &'a gpu_utils::gpu::Gpu,
    window_registry: &'a DashMap<WindowId, Weak<Mutex<dyn AnyWindowWidgetInstance>>>,
    window_manager: &'a WindowManager,
}

impl UiContext<'_> {
    pub fn runtime_handle(&self) -> tokio::runtime::Handle {
        self.runtime_handle.clone()
    }

    pub fn event_sender(&self) -> EventSender {
        self.event_sender.clone()
    }

    pub fn emit_event(&self, event: Box<dyn std::any::Any + Send>) {
        self.event_sender.emit(event);
    }

    fn register_window_instance(&self, instance: Arc<Mutex<dyn AnyWindowWidgetInstance>>) {
        let id = instance.lock().unwrap().window_id();
        self.window_registry.insert(id, Arc::downgrade(&instance));
    }

    fn create_window(&self, config: &WindowConfig) -> Result<WindowHandle, WindowError> {
        self.gpu.with_device_queue(|device, _queue| {
            self.window_manager
                .create_window(self.app_ctrl, config, self.gpu.instance(), device)
        })
    }
}

// ----------------------------------------------------------------------------
// UiArch
// ----------------------------------------------------------------------------

pub struct UiArch<C: Component> {
    root: ComponentPod<C>,

    /// The built widget tree for the root component.
    /// `None` until the first `update()` call.
    widget_pod: Option<WidgetPod>,

    /// Keyed by WindowId. The strong Arc lives in the WindowWidget inside the tree;
    /// UiArch only holds a Weak so that dropping a window from the view tree
    /// automatically destroys the OS window.
    ///
    /// Wrapped in `Arc<Mutex<...>>` so that a `UiContext` impl created during
    /// `update()` can write to it directly without borrow-conflict with `widget_pod`.
    window_registry: DashMap<WindowId, Weak<Mutex<dyn AnyWindowWidgetInstance>>>,

    event_sender: EventSender,
}

impl<C: Component> UiArch<C> {
    pub(crate) fn new(root: C, event_sender: EventSender) -> Self {
        Self {
            root: ComponentPod::new(None, root),
            widget_pod: None,
            window_registry: DashMap::new(),
            event_sender,
        }
    }
}

// ----------------------------------------------------------------------------
// App Lifecycle methods
// ----------------------------------------------------------------------------

impl<C: Component> UiArch<C> {
    pub(crate) fn init(
        &mut self,
        runtime_handle: &tokio::runtime::Handle,
        app_ctrl: &impl ApplicationControler,
    ) {
        let ctx = AppContext {
            runtime_handle,
            event_sender: &self.event_sender,
            app_ctrl,
        };

        self.root.init(&ctx);
    }

    pub(crate) fn resumed(
        &mut self,
        runtime_handle: &tokio::runtime::Handle,
        app_ctrl: &impl ApplicationControler,
    ) {
        let ctx = AppContext {
            runtime_handle,
            event_sender: &self.event_sender,
            app_ctrl,
        };

        self.root.resumed(&ctx);
    }

    pub(crate) fn suspended(
        &mut self,
        runtime_handle: &tokio::runtime::Handle,
        app_ctrl: &impl ApplicationControler,
    ) {
        let ctx = AppContext {
            runtime_handle,
            event_sender: &self.event_sender,
            app_ctrl,
        };

        self.root.suspended(&ctx);
    }

    pub(crate) fn exiting(
        &mut self,
        runtime_handle: &tokio::runtime::Handle,
        app_ctrl: &impl ApplicationControler,
    ) {
        let ctx = AppContext {
            runtime_handle,
            event_sender: &self.event_sender,
            app_ctrl,
        };

        self.root.exiting(&ctx);
    }
}

/// GPU Device Lost
impl<C: Component> UiArch<C> {
    pub(crate) fn gpu_device_lost(&mut self, _app_ctrl: &impl ApplicationControler) {
        // destroy all widgets and clear the window registry
        self.widget_pod = None;

        // reconfigure surface
    }
}

// ----------------------------------------------------------------------------
// UI update
// ----------------------------------------------------------------------------

impl<C: Component> UiArch<C> {
    /// Rebuilds the view tree and reconciles the window registry.
    ///
    /// Called on every `BufferUpdated` event or continuous-render tick.
    /// Creates a short-lived `UiArchContext` and drives `Component::view()` → widget
    /// reconciliation. New `Window` views register themselves via
    /// `ctx.register_window_instance()`; dead `Weak` references are pruned afterwards.
    pub(crate) fn update(
        &mut self,
        runtime_handle: &tokio::runtime::Handle,
        app_ctrl: &impl ApplicationControler,
        window_manager: &WindowManager,
        gpu: &gpu_utils::gpu::Gpu,
    ) {
        let ctx = UiContext {
            runtime_handle,
            event_sender: &self.event_sender,
            app_ctrl,
            window_registry: &self.window_registry,
            window_manager,
            gpu,
        };

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
// Event handlers
// ----------------------------------------------------------------------------

impl<C: Component> UiArch<C> {
    pub(crate) fn window_event(
        &mut self,
        _app_ctrl: &impl ApplicationControler,
        _window_id: WindowId,
        _event: WindowEvent,
    ) {
        // TODO
    }

    /// Routes a device event to the widget tree of the target window.
    ///
    /// Looks up `window_id` in the registry, upgrades the `Weak`, then forwards
    /// the event to `AnyWindowWidgetInstance::device_input`. Events emitted by widgets
    /// reach the application via `ctx.event_sender().emit(...)`.
    pub(crate) fn device_event(
        &mut self,
        window_id: WindowId,
        event: DeviceEvent,
        runtime_handle: &tokio::runtime::Handle,
        app_ctrl: &impl ApplicationControler,
        window_manager: &WindowManager,
        gpu: &gpu_utils::gpu::Gpu,
    ) {
        let ctx = UiContext {
            runtime_handle,
            event_sender: &self.event_sender,
            app_ctrl,
            window_registry: &self.window_registry,
            window_manager,
            gpu,
        };

        // Upgrade the Weak *before* taking any other lock to avoid a potential deadlock
        // between the registry lock and the instance lock.
        let op_arc_window = self
            .window_registry
            .get(&window_id)
            .and_then(|w| w.upgrade());

        if let Some(arc_window) = op_arc_window {
            let mut instance = arc_window.lock().unwrap();
            instance.device_input(&event, &ctx);
        }
    }

    pub(crate) fn raw_device_event(
        &mut self,
        _app_ctrl: &impl ApplicationControler,
        _raw_device_id: RawDeviceId,
        _raw_event: RawDeviceEvent,
    ) {
        // TODO
    }

    /// Delivers a `C::Message` to the root component.
    pub(crate) fn user_event(
        &mut self,
        msg: C::Message,
        runtime_handle: &tokio::runtime::Handle,
        app_ctrl: &impl ApplicationControler,
        window_manager: &WindowManager,
        gpu: &gpu_utils::gpu::Gpu,
    ) {
        let ctx = UiContext {
            runtime_handle,
            event_sender: &self.event_sender,
            app_ctrl,
            window_registry: &self.window_registry,
            window_manager,
            gpu,
        };

        self.root.update(msg, &ctx);
    }
}
