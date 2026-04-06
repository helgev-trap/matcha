use std::{collections::HashMap, future::Future, sync::Arc};

use parking_lot::Mutex;
use renderer::RenderNode;

use super::widget::{View, Widget, WidgetContext, WidgetInteractionResult, WidgetPod};
use crate::{
    event::device_event::DeviceEvent,
    ui_arch::{metrics, widget::WidgetUpdateError},
};

// ----------------------------------------------------------------------------
// TaskHandler
// ----------------------------------------------------------------------------

/// Manages background tasks spawned by a [`Component`].
///
/// Passed as `&mut TaskHandler` to each [`Component`] method.
///
/// Tasks write UI state directly via [`SharedValue::store()`](shared_buffer::SharedValue::store),
/// which automatically signals the event loop. Discrete commands from external
/// sources are delivered through [`Component::update()`].
pub struct TaskHandler {
    tasks: HashMap<String, tokio::task::JoinHandle<()>>,
}

impl TaskHandler {
    pub(crate) fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    /// Spawns a fire-and-forget background task.
    ///
    /// The task writes UI state by calling
    /// [`SharedValue::store()`](shared_buffer::SharedValue::store) directly.
    ///
    /// Returns [`TaskError::AlreadyExists`] if a task with the same `id` is running.
    pub fn spawn<F, Fut>(&mut self, id: impl Into<String>, task: F) -> Result<(), TaskError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let id = id.into();
        if let Some(handle) = self.tasks.get(&id) {
            if !handle.is_finished() {
                return Err(TaskError::AlreadyExists);
            }
        }
        let handle = tokio::spawn(task());
        self.tasks.insert(id, handle);
        Ok(())
    }

    /// Removes all finished task entries from the internal map.
    pub fn gc(&mut self) {
        self.tasks.retain(|_, h| !h.is_finished());
    }

    /// Aborts the task with the given `id`.
    ///
    /// Returns [`TaskError::NotFound`] if no such task exists.
    pub fn abort(&mut self, id: impl Into<String>) -> Result<(), TaskError> {
        let id = id.into();
        if let Some(handle) = self.tasks.remove(&id) {
            handle.abort();
            Ok(())
        } else {
            Err(TaskError::NotFound)
        }
    }

    /// Returns `true` if a task with the given `id` is currently running.
    pub fn is_running(&self, id: &str) -> bool {
        self.tasks
            .get(id)
            .map(|h| !h.is_finished())
            .unwrap_or(false)
    }
}

impl Drop for TaskHandler {
    fn drop(&mut self) {
        for (_, handle) in self.tasks.drain() {
            handle.abort();
        }
    }
}

#[derive(Debug)]
pub enum TaskError {
    AlreadyExists,
    NotFound,
}

// ----------------------------------------------------------------------------
// Component
// ----------------------------------------------------------------------------

/// A trait for building stateful UI components.
///
/// # Lifecycle
///
/// 1. [`setup`](Component::setup) is called once when the component is first attached.
/// 2. [`view`](Component::view) is called to build the widget tree.
/// 3. [`update`](Component::update) is called when a discrete [`Message`](Component::Message)
///    arrives from the application layer.
/// 4. [`event`](Component::event) translates a child widget's `InnerEvent` into an outward
///    `Event`, optionally bubbling it up to a parent.
///
/// # State management
///
/// State is held as [`SharedValue`](shared_buffer::SharedValue) fields.
/// Calling `SharedValue::store()` automatically signals the event loop to schedule
/// a redraw. If no `store()` is called, no redraw is triggered.
///
/// All methods take `&self`, so a component can be shared as `Arc<C>` with
/// the backend without requiring exclusive access.
pub trait Component: Send + Sync + 'static {
    /// Discrete commands delivered from the application layer.
    type Message: Send + Sync + 'static;
    /// Event propagated upward to the parent component.
    type Event: Send + Sync + 'static;
    /// Event emitted by child widgets, consumed by [`event()`](Component::event).
    type InnerEvent: Send + Sync + 'static;

    /// Called once when the component is first attached to the widget tree.
    fn setup(&self, task_handler: &mut TaskHandler, ctx: &dyn WidgetContext);

    /// Called when a discrete [`Message`](Component::Message) is received.
    ///
    /// Typically used to trigger background tasks or write to [`SharedValue`](shared_buffer::SharedValue)
    /// fields. A redraw occurs only if `store()` is called.
    fn update(
        &self,
        task_handler: &mut TaskHandler,
        message: Self::Message,
        ctx: &dyn WidgetContext,
    );

    /// Builds the view tree from the current state.
    fn view(&self, ctx: &dyn WidgetContext) -> Box<dyn View<Self::InnerEvent>>;

    /// Translates a child widget's `InnerEvent` into an optional outward `Event`.
    fn event(
        &self,
        task_handler: &mut TaskHandler,
        event: Self::InnerEvent,
        ctx: &dyn WidgetContext,
    ) -> Option<Self::Event>;

    /// Handles a raw device event (keyboard, mouse, etc.). Default implementation is a no-op.
    fn input(
        &self,
        task_handler: &mut TaskHandler,
        device_event: &DeviceEvent,
        ctx: &dyn WidgetContext,
    ) {
        let _ = (task_handler, device_event, ctx);
    }
}

// ----------------------------------------------------------------------------
// ComponentPod
// ----------------------------------------------------------------------------

/// Owns a [`Component`] together with its [`TaskHandler`].
///
/// Held by the UI framework. Use [`ComponentPod::arc()`] to share the component
/// state with the backend.
pub struct ComponentPod<C: Component> {
    label: Option<String>,
    component: Arc<C>,
    task_handler: Arc<Mutex<TaskHandler>>,
}

impl<C: Component> ComponentPod<C> {
    pub fn new(label: Option<&str>, component: C) -> Self {
        Self {
            label: label.map(|s| s.to_string()),
            component: Arc::new(component),
            task_handler: Arc::new(Mutex::new(TaskHandler::new())),
        }
    }

    /// Returns a cloned `Arc` to the component.
    ///
    /// The backend holds this `Arc` and writes to [`SharedValue`](shared_buffer::SharedValue)
    /// fields directly to update UI state.
    pub fn arc(&self) -> Arc<C> {
        self.component.clone()
    }
}

impl<C: Component> ComponentPod<C> {
    /// Calls [`Component::setup`]. Invoke once after construction.
    pub fn setup(&self, ctx: &dyn WidgetContext) {
        self.component.setup(&mut self.task_handler.lock(), ctx);
    }

    /// Delivers a discrete [`Message`](Component::Message) to the component.
    pub fn update(&self, message: C::Message, ctx: &dyn WidgetContext) {
        self.component
            .update(&mut self.task_handler.lock(), message, ctx);
    }

    /// Builds a [`ComponentView`] from the current state.
    pub fn view(&self, ctx: &dyn WidgetContext) -> ComponentView<C> {
        ComponentView {
            label: self.label.clone(),
            task_handler: self.task_handler.clone(),
            component: self.component.clone(),
            inner_view: self.component.view(ctx),
        }
    }
}

// ----------------------------------------------------------------------------
// ComponentView
// ----------------------------------------------------------------------------

pub struct ComponentView<C: Component> {
    label: Option<String>,
    task_handler: Arc<Mutex<TaskHandler>>,
    component: Arc<C>,
    inner_view: Box<dyn View<C::InnerEvent>>,
}

impl<C: Component> View<C::Event> for ComponentView<C> {
    fn build(&self, ctx: &dyn WidgetContext) -> WidgetPod<C::Event> {
        WidgetPod::new(
            self.label.as_deref(),
            ComponentWidget {
                task_handler: self.task_handler.clone(),
                component: self.component.clone(),
                inner_widget: self.inner_view.build(ctx),
            },
        )
    }
}

// ----------------------------------------------------------------------------
// ComponentWidget
// ----------------------------------------------------------------------------

struct ComponentWidget<C: Component> {
    task_handler: Arc<Mutex<TaskHandler>>,
    component: Arc<C>,
    inner_widget: WidgetPod<C::InnerEvent>,
}

impl<C: Component> Widget<C::Event> for ComponentWidget<C> {
    type View = ComponentView<C>;

    fn update(&mut self, view: &Self::View, ctx: &dyn WidgetContext) -> WidgetInteractionResult {
        match self.inner_widget.try_update(view.inner_view.as_ref(), ctx) {
            Ok(interaction_result) => interaction_result,
            Err(WidgetUpdateError::TypeMismatch) => {
                self.inner_widget = view.inner_view.build(ctx);
                WidgetInteractionResult::LayoutNeeded
            }
        }
    }

    fn device_input(
        &mut self,
        bounds: [f32; 2],
        event: &DeviceEvent,
        ctx: &dyn WidgetContext,
    ) -> (Option<C::Event>, WidgetInteractionResult) {
        self.component
            .input(&mut self.task_handler.lock(), event, ctx);

        let (inner_event, interaction_result) = self.inner_widget.device_input(bounds, event, ctx);

        if let Some(inner_event) = inner_event {
            let event = self
                .component
                .event(&mut self.task_handler.lock(), inner_event, ctx);
            (event, interaction_result)
        } else {
            (None, interaction_result)
        }
    }

    fn is_inside(&self, bounds: [f32; 2], position: [f32; 2], ctx: &dyn WidgetContext) -> bool {
        self.inner_widget.is_inside(bounds, position, ctx)
    }

    fn measure(&self, constraints: &metrics::Constraints, ctx: &dyn WidgetContext) -> [f32; 2] {
        self.inner_widget.measure(constraints, ctx)
    }

    fn render(&mut self, bounds: [f32; 2], ctx: &dyn WidgetContext) -> RenderNode {
        self.inner_widget.render(bounds, ctx)
    }
}
