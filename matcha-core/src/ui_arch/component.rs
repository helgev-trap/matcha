use std::{collections::HashMap, future::Future, sync::Arc};

use parking_lot::{Mutex, RwLock};
use renderer::RenderNode;
use tokio::sync::mpsc;

use super::widget::{View, Widget, WidgetContext, WidgetInteractionResult, WidgetPod};
use crate::{
    event::device_event::DeviceEvent,
    ui_arch::{metrics, update_flag::WakeupHandle, widget::WidgetUpdateError},
};

// ----------------------------------------------------------------------------
// TaskHandler
// ----------------------------------------------------------------------------

/// Manages background tasks spawned by a [`Component`].
///
/// Passed as `&mut TaskHandler<C::Message>` to each [`Component`] method. Tasks spawned
/// via [`TaskHandler::spawn_msg`] deliver their return value as a `Message` to
/// [`Component::update`] when they complete.
pub struct TaskHandler<Message: Send + 'static> {
    tasks: HashMap<String, tokio::task::JoinHandle<()>>,
    /// Both ends of the message channel live here.
    /// Sender is cloned per-task; receiver is drained by [`ComponentPod::poll_messages`]
    /// through the Mutex guard.
    message_tx: mpsc::UnboundedSender<Message>,
    message_rx: mpsc::UnboundedReceiver<Message>,
    wakeup: WakeupHandle,
}

impl<Message: Send + 'static> TaskHandler<Message> {
    fn new(wakeup: WakeupHandle) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            tasks: HashMap::new(),
            message_tx: tx,
            message_rx: rx,
            wakeup,
        }
    }

    /// Spawn a fire-and-forget background task (no return value).
    ///
    /// Returns [`TaskError::AlreadyExists`] if a task with the same `id` is already running.
    pub fn spawn<F, Fut>(&mut self, id: impl Into<String>, task: F) -> Result<(), TaskError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let id = id.into();
        if self.tasks.contains_key(&id) {
            return Err(TaskError::AlreadyExists);
        }
        let handle = tokio::spawn(task());
        self.tasks.insert(id, handle);
        Ok(())
    }

    /// Spawn a background task whose return value is delivered as a [`Component::update`]
    /// message when the task completes.
    ///
    /// The task is `Send + 'static`; use cloned `Arc`s or `Copy` values to share state
    /// with the running task. When the future resolves:
    ///
    /// 1. The returned `Message` is sent through the internal channel.
    /// 2. [`WakeupHandle::wake`] is called (if set) to alert the event loop.
    /// 3. The owning [`ComponentPod::poll_messages`] drains the channel and calls
    ///    [`Component::update`] on the UI thread.
    ///
    /// Returns [`TaskError::AlreadyExists`] if a task with the same `id` is already running.
    pub fn spawn_msg(
        &mut self,
        id: impl Into<String>,
        task: impl Future<Output = Message> + Send + 'static,
    ) -> Result<(), TaskError> {
        let id = id.into();
        if self.tasks.contains_key(&id) {
            return Err(TaskError::AlreadyExists);
        }
        let tx = self.message_tx.clone();
        let wakeup = self.wakeup;

        let handle = tokio::spawn(async move {
            let msg = task.await;
            let _ = tx.send(msg);
            wakeup.wake();
        });

        self.tasks.insert(id, handle);
        Ok(())
    }

    /// Abort the task identified by `id`.
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

    /// Returns `true` if a task with the given `id` is currently registered.
    pub fn is_running(&self, id: &str) -> bool {
        self.tasks.contains_key(id)
    }
}

impl<Message: Send + 'static> Drop for TaskHandler<Message> {
    fn drop(&mut self) {
        // Abort all running tasks when the handler is dropped (i.e., when
        // ComponentPod is dropped) to avoid dangling background work.
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

/// A trait for building stateful, Elm-like UI components.
///
/// # Lifecycle
///
/// 1. [`setup`](Component::setup) is called once when the component is first attached.
/// 2. [`view`](Component::view) is called to build the widget tree.
/// 3. [`update`](Component::update) is called when a [`Message`](Component::Message) arrives
///    (from a background task via [`TaskHandler::spawn_msg`], or directly from the framework).
/// 4. [`event`](Component::event) translates a child widget's `InnerEvent` into an outward
///    `Event`, optionally bubbling it up to a parent component.
pub trait Component: Send + Sync + 'static {
    type Message: Send + Sync + 'static;
    type Event: Send + Sync + 'static;
    type InnerEvent: Send + Sync + 'static;

    /// Called once when the component is first attached to the widget tree.
    /// Use this to start any initial background tasks.
    fn setup(&mut self, task_handler: &mut TaskHandler<Self::Message>, ctx: &dyn WidgetContext);

    /// Called when a [`Message`](Component::Message) is received.
    ///
    /// Messages arrive either from background tasks ([`TaskHandler::spawn_msg`]) or from
    /// the application layer. Spawn follow-up tasks here to continue background work.
    ///
    /// # TODO
    /// Return a flag indicating whether a re-render is needed.
    fn update(
        &mut self,
        task_handler: &mut TaskHandler<Self::Message>,
        message: Self::Message,
        ctx: &dyn WidgetContext,
    );

    /// Build the view tree for this frame.
    fn view(&mut self, ctx: &dyn WidgetContext) -> Box<dyn View<Self::InnerEvent>>;

    /// Translate an `InnerEvent` from a child widget into an optional outward `Event`.
    fn event(
        &mut self,
        task_handler: &mut TaskHandler<Self::Message>,
        event: Self::InnerEvent,
        ctx: &dyn WidgetContext,
    ) -> Option<Self::Event>;

    /// Handle a raw device event (keyboard, mouse, etc.).
    /// Provides a default no-op implementation.
    fn input(
        &mut self,
        task_handler: &mut TaskHandler<Self::Message>,
        device_event: &DeviceEvent,
        ctx: &dyn WidgetContext,
    ) {
        let _ = task_handler;
        let _ = device_event;
        let _ = ctx;
    }
}

// ----------------------------------------------------------------------------
// ComponentPod
// ----------------------------------------------------------------------------

/// Wraps a [`Component`] together with its [`TaskHandler`] and message channel.
///
/// Owned by the UI framework. Users interact with the component via its
/// [`Component`] trait methods, which are dispatched through this pod.
pub struct ComponentPod<C: Component> {
    label: Option<String>,
    /// Shared with [`ComponentWidget`] so the widget layer can call
    /// `input` and `event` without going through the pod.
    task_handler: Arc<Mutex<TaskHandler<C::Message>>>,
    /// Shared with [`ComponentWidget`] for the same reason.
    component: Arc<RwLock<C>>,
}

impl<C: Component> ComponentPod<C> {
    pub fn new(label: Option<&str>, component: C) -> Self {
        Self {
            label: label.map(|s| s.to_string()),
            task_handler: Arc::new(Mutex::new(TaskHandler::new(super::global_wakeup_handle()))),
            component: Arc::new(RwLock::new(component)),
        }
    }
}

impl<C: Component> ComponentPod<C> {
    /// Call [`Component::setup`]. Invoke once after construction.
    pub fn setup(&mut self, ctx: &dyn WidgetContext) {
        let mut task_handler = self.task_handler.lock();
        self.component.write().setup(&mut task_handler, ctx);
    }

    /// Deliver a message directly (e.g., from a parent component or application layer).
    pub fn update(&mut self, message: C::Message, ctx: &dyn WidgetContext) {
        let mut task_handler = self.task_handler.lock();
        self.component
            .write()
            .update(&mut task_handler, message, ctx);
    }

    /// Drain all pending messages from background tasks and call [`Component::update`]
    /// for each one. Messages are processed in task-completion order (FIFO).
    ///
    /// Returns `true` if at least one message was processed, indicating that
    /// [`view`](ComponentPod::view) should be called again to reflect the new state.
    pub fn poll_messages(&mut self, ctx: &dyn WidgetContext) -> bool {
        let mut did_update = false;
        loop {
            // Lock task_handler to access the receiver, then extract one message.
            // The lock is re-acquired each iteration so that update() can spawn
            // new tasks (which also need the lock) without deadlocking.
            let mut task_handler = self.task_handler.lock();
            let msg = match task_handler.message_rx.try_recv() {
                Ok(msg) => msg,
                Err(_) => break,
            };
            self.component.write().update(&mut *task_handler, msg, ctx);
            did_update = true;
            // Lock released here; next iteration re-acquires.
        }
        did_update
    }

    /// Build a [`ComponentView`] from the current state.
    /// Call after [`setup`](ComponentPod::setup) and whenever the model changes.
    pub fn view(&mut self, ctx: &dyn WidgetContext) -> ComponentView<C> {
        ComponentView {
            label: self.label.clone(),
            task_handler: self.task_handler.clone(),
            component: self.component.clone(),
            inner_view: self.component.write().view(ctx),
        }
    }
}

// ----------------------------------------------------------------------------
// ComponentView
// ----------------------------------------------------------------------------

pub struct ComponentView<C: Component> {
    label: Option<String>,
    task_handler: Arc<Mutex<TaskHandler<C::Message>>>,
    component: Arc<RwLock<C>>,
    inner_view: Box<dyn View<C::InnerEvent>>,
}

impl<C: Component> View<C::Event> for ComponentView<C> {
    fn build(&self) -> WidgetPod<C::Event> {
        WidgetPod::new(
            self.label.as_deref(),
            ComponentWidget {
                task_handler: self.task_handler.clone(),
                component: self.component.clone(),
                inner_widget: self.inner_view.build(),
            },
        )
    }
}

// ----------------------------------------------------------------------------
// ComponentWidget
// ----------------------------------------------------------------------------

struct ComponentWidget<C: Component> {
    task_handler: Arc<Mutex<TaskHandler<C::Message>>>,
    component: Arc<RwLock<C>>,
    inner_widget: WidgetPod<C::InnerEvent>,
}

impl<C: Component> Widget<C::Event> for ComponentWidget<C> {
    type View = ComponentView<C>;

    fn update(&mut self, view: &Self::View) -> WidgetInteractionResult {
        match self.inner_widget.try_update(view.inner_view.as_ref()) {
            Ok(interaction_result) => interaction_result,
            Err(WidgetUpdateError::TypeMismatch) => {
                self.inner_widget = view.inner_view.build();
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
        // Call input() with a short-lived lock to avoid holding it across inner_widget calls.
        {
            let mut task_handler = self.task_handler.lock();
            self.component.write().input(&mut task_handler, event, ctx);
        }

        let (inner_event, interaction_result) = self.inner_widget.device_input(bounds, event, ctx);

        if let Some(inner_event) = inner_event {
            let mut task_handler = self.task_handler.lock();
            let event = self
                .component
                .write()
                .event(&mut task_handler, inner_event, ctx);
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
