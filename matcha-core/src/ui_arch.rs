use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak},
};

use crate::application::ApplicationControler;
use crate::event::device_event::DeviceEvent;
use crate::event::raw_device_event::{RawDeviceEvent, RawDeviceId};
use crate::event::window_event::WindowEvent;
use crate::window::WindowId;
use crate::window_manager::WindowManager;

pub mod component;
pub mod metrics;
pub mod ui_context;
pub mod widget;
pub mod window;

use component::{Component, ComponentPod};
use widget::WidgetPod;
use window::AnyWindowWidgetInstance;

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
    window_registry: Arc<Mutex<HashMap<WindowId, Weak<Mutex<dyn AnyWindowWidgetInstance>>>>>,
}

impl<C: Component> UiArch<C> {
    pub fn new(root: C) -> Self {
        Self {
            root: ComponentPod::new(None, root),
            widget_pod: None,
            window_registry: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// Lifecycle methods
impl<C: Component> UiArch<C> {
    pub(crate) fn init(&mut self, _app_ctrl: &impl ApplicationControler) {}

    pub(crate) fn resumed(&mut self, _app_ctrl: &impl ApplicationControler) {}

    pub(crate) fn suspended(&mut self, _app_ctrl: &impl ApplicationControler) {}

    pub(crate) fn exiting(&mut self, _app_ctrl: &impl ApplicationControler) {}
}

/// GPU Device Lost
impl<C: Component> UiArch<C> {
    pub(crate) fn gpu_device_lost(&mut self, _app_ctrl: &impl ApplicationControler) {}
}

/// UI update
impl<C: Component> UiArch<C> {
    /// Rebuilds the view tree and reconciles the window registry.
    ///
    /// Called on every `BufferUpdated` event or continuous-render tick.
    /// The concrete `UiContext` implementation (provided by the platform integration)
    /// collects `register_window_instance` calls during tree traversal and updates
    /// `window_registry` accordingly.
    pub(crate) fn update(
        &mut self,
        _window_manager: &WindowManager,
        _app_ctrl: &impl ApplicationControler,
        _gpu: &gpu_utils::gpu::Gpu,
    ) {}
}

/// Event handlers
impl<C: Component> UiArch<C> {
    pub(crate) fn window_event(
        &mut self,
        _app_ctrl: &impl ApplicationControler,
        _window_id: WindowId,
        _event: WindowEvent,
    ) {
        todo!()
    }

    pub(crate) fn device_event(
        &mut self,
        _app_ctrl: &impl ApplicationControler,
        _window_id: WindowId,
        _event: DeviceEvent,
    ) {
        todo!()
    }

    pub(crate) fn raw_device_event(
        &mut self,
        _app_ctrl: &impl ApplicationControler,
        _raw_device_id: RawDeviceId,
        _raw_event: RawDeviceEvent,
    ) {
    }

    pub(crate) fn user_event(&mut self, _app_ctrl: &impl ApplicationControler, _msg: C::Message) {
        todo!()
    }
}
