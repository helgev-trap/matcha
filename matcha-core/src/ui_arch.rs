use std::collections::HashMap;

use crate::application::ApplicationControler;
use crate::event::device_event::DeviceEvent;
use crate::event::raw_device_event::{RawDeviceEvent, RawDeviceId};
use crate::event::window_event::WindowEvent;
use crate::window::WindowId;
use crate::window_manager::WindowManager;

pub mod component;
pub mod metrics;
pub mod widget;
pub mod window_model;

use widget::{WidgetContext, WidgetUpdateError};
use window_model::{WindowDecl, WindowModel, WindowModelContext, WindowModelPod, WindowState};

// ----------------------------------------------------------------------------
// Context implementations (MVP: empty structs)
// ----------------------------------------------------------------------------

struct UiArchWindowModelContext;
impl WindowModelContext for UiArchWindowModelContext {}

struct UiArchWidgetContext;
impl WidgetContext for UiArchWidgetContext {}

// ----------------------------------------------------------------------------
// UiArch
// ----------------------------------------------------------------------------

pub struct UiArch<M: WindowModel> {
    model_pod: WindowModelPod<M>,
    window_states: Vec<WindowState<M::Event>>,
}

impl<M: WindowModel> UiArch<M> {
    pub fn new(model: M) -> Self {
        Self {
            model_pod: WindowModelPod::new(None, model),
            window_states: Vec::new(),
        }
    }
}

/// Lifecycle methods
impl<M: WindowModel> UiArch<M> {
    pub(crate) fn init(&mut self, _app_ctrl: &impl ApplicationControler) {
        let ctx = UiArchWindowModelContext;
        self.model_pod.setup(&ctx);
    }

    pub(crate) fn resumed(&mut self, _app_ctrl: &impl ApplicationControler) {}

    pub(crate) fn suspended(&mut self, _app_ctrl: &impl ApplicationControler) {}

    pub(crate) fn exiting(&mut self, _app_ctrl: &impl ApplicationControler) {}
}

/// GPU Device Lost
impl<M: WindowModel> UiArch<M> {
    pub(crate) fn gpu_device_lost(&mut self, _app_ctrl: &impl ApplicationControler) {}
}

/// UI update
impl<M: WindowModel> UiArch<M> {
    /// Rebuilds the window list and reconciles native windows + widget trees.
    ///
    /// Called on every `BufferUpdated` event or continuous-render tick.
    pub(crate) fn update(
        &mut self,
        window_manager: &WindowManager,
        app_ctrl: &impl ApplicationControler,
        gpu: &gpu_utils::gpu::Gpu,
    ) {
        let model_ctx = UiArchWindowModelContext;
        let new_decls = self.model_pod.windows(&model_ctx);
        let widget_ctx = UiArchWidgetContext;

        // Drain existing states into a key-indexed map for O(1) lookup.
        let mut old_states: HashMap<String, WindowState<M::Event>> = self
            .window_states
            .drain(..)
            .map(|s| (s.key.clone(), s))
            .collect();

        let instance = gpu.instance();

        for decl in new_decls {
            let state = if let Some(mut existing) = old_states.remove(&decl.key) {
                // Same key → diff-update the widget tree.
                match &mut existing.widget_pod {
                    Some(pod) => {
                        if pod.try_update(decl.root.as_ref(), &widget_ctx).is_err() {
                            existing.widget_pod = Some(decl.root.build(&widget_ctx));
                        }
                    }
                    None => {
                        existing.widget_pod = Some(decl.root.build(&widget_ctx));
                    }
                }
                existing
            } else {
                // New key → create a native window and build the widget tree.
                let handle = gpu.with_device_queue(|device, _| {
                    window_manager.create_window(app_ctrl, &decl.config, instance, device)
                });

                match handle {
                    Ok(handle) => WindowState {
                        key: decl.key,
                        handle,
                        widget_pod: Some(decl.root.build(&widget_ctx)),
                    },
                    Err(e) => {
                        log::error!("Failed to create window: {e}");
                        continue;
                    }
                }
            };

            self.window_states.push(state);
        }

        // States remaining in `old_states` are dropped here.
        // WindowHandle::Drop removes them from WindowManager automatically.
    }
}

/// Event handlers
impl<M: WindowModel> UiArch<M> {
    pub(crate) fn window_event(
        &mut self,
        _app_ctrl: &impl ApplicationControler,
        _window_id: WindowId,
        _event: WindowEvent,
    ) -> bool {
        false // TODO: route to matching WindowState's widget_pod
    }

    pub(crate) fn device_event(
        &mut self,
        _app_ctrl: &impl ApplicationControler,
        _window_id: WindowId,
        _event: DeviceEvent,
    ) -> bool {
        false // TODO
    }

    pub(crate) fn raw_device_event(
        &mut self,
        _app_ctrl: &impl ApplicationControler,
        _raw_device_id: RawDeviceId,
        _raw_event: RawDeviceEvent,
    ) {
    }

    pub(crate) fn user_event(&mut self, _app_ctrl: &impl ApplicationControler, msg: M::Message) {
        let ctx = UiArchWindowModelContext;
        self.model_pod.update(msg, &ctx);
    }
}
