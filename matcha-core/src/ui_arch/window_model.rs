use std::sync::Arc;

use parking_lot::Mutex;

use super::component::TaskHandler;
use super::widget::{View, WidgetPod};
use crate::window::WindowConfig;
use crate::window_manager::WindowHandle;

// ----------------------------------------------------------------------------
// WindowDecl
// ----------------------------------------------------------------------------

/// Declares a single window: its OS configuration and the root View that fills it.
///
/// Not a `View<T>` — only `UiArch` can turn this into a live window.
pub struct WindowDecl<T: 'static> {
    /// Stable identity key used for diffing. Windows with the same key across
    /// consecutive `windows()` calls are treated as the same window.
    pub key: String,
    pub config: WindowConfig,
    pub root: Box<dyn View<T>>,
}

// ----------------------------------------------------------------------------
// WindowModelContext
// ----------------------------------------------------------------------------

/// Context passed to [`WindowModel`] methods.
///
/// MVP: empty marker trait. Will hold screen info, font systems, etc. in future.
pub trait WindowModelContext {}

// ----------------------------------------------------------------------------
// WindowModel
// ----------------------------------------------------------------------------

/// Top-level trait for declaring which windows exist and managing window-level state.
///
/// Analogous to [`Component`](super::component::Component) but operates at the
/// window layer. Implementations hold [`SharedValue`](shared_buffer::SharedValue)
/// fields to drive dynamic window creation/destruction.
pub trait WindowModel: Send + Sync + 'static {
    /// Discrete commands from the application backend.
    type Message: Send + Sync + 'static;
    /// Event type propagated from widget trees to the backend.
    type Event: Send + Sync + 'static;

    /// Called once when the model is first attached. Use to spawn background tasks.
    fn setup(&self, task_handler: &mut TaskHandler, ctx: &dyn WindowModelContext);

    /// Called when a discrete [`Message`](WindowModel::Message) is delivered.
    fn update(
        &self,
        task_handler: &mut TaskHandler,
        msg: Self::Message,
        ctx: &dyn WindowModelContext,
    );

    /// Returns the current set of window declarations.
    ///
    /// Called on every [`UiArch::update()`] cycle. The result is diffed against
    /// the previous set by [`WindowDecl::key`].
    fn windows(&self, ctx: &dyn WindowModelContext) -> Vec<WindowDecl<Self::Event>>;
}

// ----------------------------------------------------------------------------
// WindowModelPod
// ----------------------------------------------------------------------------

/// Owns a [`WindowModel`] together with its [`TaskHandler`].
///
/// Analogous to [`ComponentPod`](super::component::ComponentPod).
pub struct WindowModelPod<M: WindowModel> {
    label: Option<String>,
    model: Arc<M>,
    task_handler: Arc<Mutex<TaskHandler>>,
}

impl<M: WindowModel> WindowModelPod<M> {
    pub fn new(label: Option<&str>, model: M) -> Self {
        Self {
            label: label.map(|s| s.to_string()),
            model: Arc::new(model),
            task_handler: Arc::new(Mutex::new(TaskHandler::new())),
        }
    }

    /// Returns a cloned `Arc` to share model state with the backend.
    pub fn arc(&self) -> Arc<M> {
        self.model.clone()
    }
}

impl<M: WindowModel> WindowModelPod<M> {
    pub fn setup(&self, ctx: &dyn WindowModelContext) {
        self.model.setup(&mut self.task_handler.lock(), ctx);
    }

    pub fn update(&self, msg: M::Message, ctx: &dyn WindowModelContext) {
        self.model.update(&mut self.task_handler.lock(), msg, ctx);
    }

    pub fn windows(&self, ctx: &dyn WindowModelContext) -> Vec<WindowDecl<M::Event>> {
        self.model.windows(ctx)
    }
}

// ----------------------------------------------------------------------------
// WindowState (crate-internal)
// ----------------------------------------------------------------------------

/// Live state for one OS window managed by [`UiArch`](super::UiArch).
pub(crate) struct WindowState<T: 'static> {
    /// Matches [`WindowDecl::key`] for diffing.
    pub(crate) key: String,
    /// Keeps the native window alive. Dropping this removes the window from
    /// [`WindowManager`](crate::window_manager::WindowManager).
    pub(crate) handle: Arc<WindowHandle>,
    /// Root widget pod for this window. `None` until first build.
    pub(crate) widget_pod: Option<WidgetPod<T>>,
}
