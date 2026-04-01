use std::{collections::HashMap, sync::Arc};

use parking_lot::{Mutex, RwLock};
use renderer::RenderNode;

use super::widget::{View, Widget, WidgetContext, WidgetInteractionResult, WidgetPod};
use crate::{
    event::device_event::DeviceEvent,
    ui_arch::{metrics, widget::WidgetUpdateError},
};

// TODO: consider this.
//
// // ----------------------------------------------------------------------------
// // Context
// // ----------------------------------------------------------------------------
//
// pub trait ComponentContext {
//     // TODO
// }

// ----------------------------------------------------------------------------
// TaskHandler
// ----------------------------------------------------------------------------

pub struct TaskHandler {
    tasks: HashMap<String, tokio::task::JoinHandle<()>>,
}

impl TaskHandler {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    pub fn spawn<F, Fut>(&mut self, id: impl Into<String>, task: F) -> Result<(), TaskError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = ()> + Send + 'static,
    {
        let id = id.into();
        if self.tasks.contains_key(&id) {
            return Err(TaskError::AlreadyExists);
        }
        let task = tokio::spawn(task());
        self.tasks.insert(id, task);
        Ok(())
    }

    pub fn abort(&mut self, id: impl Into<String>) -> Result<(), TaskError> {
        let id = id.into();
        if let Some(task) = self.tasks.remove(&id) {
            task.abort();
            Ok(())
        } else {
            Err(TaskError::NotFound)
        }
    }
}

pub enum TaskError {
    AlreadyExists,
    NotFound,
}

// ----------------------------------------------------------------------------
// Component
// ----------------------------------------------------------------------------

/// A trait for building stateful, Elm-like components.
pub trait Component: Send + Sync + 'static {
    type Message: Send + Sync + 'static;
    type Event: Send + Sync + 'static;
    type InnerEvent: Send + Sync + 'static;

    fn setup(&mut self, task_handler: &mut TaskHandler, ctx: &dyn WidgetContext);

    fn update(
        &mut self,
        task_handler: &mut TaskHandler,
        message: Self::Message,
        ctx: &dyn WidgetContext,
    )
    /* TODO: return a info about if the component need to be re-rendered */;

    fn view(&mut self, ctx: &dyn WidgetContext) -> Box<dyn View<Self::InnerEvent>>;

    fn event(
        &mut self,
        task_handler: &mut TaskHandler,
        event: Self::InnerEvent,
        ctx: &dyn WidgetContext,
    ) -> Option<Self::Event>;

    fn input(
        &mut self,
        task_handler: &mut TaskHandler,
        device_event: &DeviceEvent,
        ctx: &dyn WidgetContext,
    )
    /* TODO: return a info about if the component need to be re-rendered */
    {
        let _ = task_handler;
        let _ = device_event;
        let _ = ctx;

        // TODO:
        // Prepare a default input handler.
    }
}

// ----------------------------------------------------------------------------
// ComponentPod
// ----------------------------------------------------------------------------

pub struct ComponentPod<C: Component> {
    label: Option<String>,
    task_handler: Arc<Mutex<TaskHandler>>,
    component: Arc<RwLock<C>>,
}

impl<C: Component> ComponentPod<C> {
    pub fn new(label: Option<&str>, component: C) -> Self {
        Self {
            label: label.map(|s| s.to_string()),
            task_handler: Arc::new(Mutex::new(TaskHandler::new())),
            component: Arc::new(RwLock::new(component)),
        }
    }
}

impl<C: Component> ComponentPod<C> {
    pub fn setup(&mut self, ctx: &dyn WidgetContext) {
        let mut task_handler = self.task_handler.lock();
        self.component.write().setup(&mut task_handler, ctx);
    }

    pub fn update(&mut self, message: C::Message, ctx: &dyn WidgetContext) {
        let mut task_handler = self.task_handler.lock();
        self.component.write().update(&mut task_handler, message, ctx);
    }

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
    task_handler: Arc<Mutex<TaskHandler>>,
    component: Arc<RwLock<C>>,
    inner_view: Box<dyn View<C::InnerEvent>>,
}

impl<C: Component> View<C::Event> for ComponentView<C> {
    fn build(&self) -> WidgetPod<C::Event> {
        // todo: Spawn setup task of component.

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
    task_handler: Arc<Mutex<TaskHandler>>,
    component: Arc<RwLock<C>>,
    inner_widget: WidgetPod<C::InnerEvent>,
}

impl<C: Component> Widget<C::Event> for ComponentWidget<C>
{
    type View = ComponentView<C>;

    fn update(&mut self, view: &Self::View) -> WidgetInteractionResult {
        match self.inner_widget.try_update(view.inner_view.as_ref()) {
            Ok(interaction_result) => interaction_result,
            Err(WidgetUpdateError::TypeMismatch) => {
                let new_inner_widget = view.inner_view.build();
                self.inner_widget = new_inner_widget;
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
        let mut task_handler = self.task_handler.lock();
        self.component.write().input(&mut task_handler, event, ctx);

        let (inner_event, interaction_result) = self.inner_widget.device_input(bounds, event, ctx);

        if let Some(inner_event) = inner_event {
            let event = self.component.write().event(&mut task_handler, inner_event, ctx);
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
