use std::sync::Arc;

use parking_lot::Mutex;
use renderer::RenderNode;

use super::widget::{View, Widget, WidgetContext, WidgetInteractionResult, WidgetPod};
use crate::{
    event::device_event::DeviceEvent,
    ui_arch::{metrics, widget::WidgetUpdateError},
};

// TODO: consider this
//
// // ----------------------------------------------------------------------------
// // Context
// // ----------------------------------------------------------------------------
//
// pub trait ComponentContext {
//     // TODO
// }

// ----------------------------------------------------------------------------
// Component
// ----------------------------------------------------------------------------

/// A trait for building stateful, Elm-like components.
pub trait Component: Send + Sync + 'static {
    type Message: Send + Sync + 'static;
    type Event: Send + Sync + 'static;
    type InnerEvent: Send + Sync + 'static;

    // -----------------------
    // Set up

    fn setup(&mut self, ctx: &dyn WidgetContext);

    // -----------------------
    // Use in model

    fn update(&mut self, message: Self::Message, ctx: &dyn WidgetContext) /* TODO: return a info about if the component need to be re-rendered */;

    fn view(&mut self, ctx: &dyn WidgetContext) -> WidgetPod<Self::Event>;

    // -----------------------
    // Use in widget implement

    fn event(&mut self, event: Self::InnerEvent, ctx: &dyn WidgetContext) -> Option<Self::Event>;

    fn input(&mut self, device_event: &DeviceEvent, ctx: &dyn WidgetContext) /* TODO: return a info about if the component need to be re-rendered */ {
        // TODO:
        // Prepare a default input handler.
    }
}

trait AnyComponent<Event: Send + Sync + 'static, InnerEvent: Send + Sync + 'static>:
    Send + Sync + 'static
{
    fn event(&mut self, event: InnerEvent, ctx: &dyn WidgetContext) -> Option<Event>;
    fn input(&mut self, device_event: &DeviceEvent, ctx: &dyn WidgetContext) /* TODO */;
}

impl<C: Component> AnyComponent<C::Event, C::InnerEvent> for C {
    fn event(&mut self, event: C::InnerEvent, ctx: &dyn WidgetContext) -> Option<C::Event> {
        Component::event(self, event, ctx)
    }

    fn input(&mut self, device_event: &DeviceEvent, ctx: &dyn WidgetContext) /* TODO */
    {
        Component::input(self, device_event, ctx)
    }
}

// ----------------------------------------------------------------------------
// ComponentPod
// ----------------------------------------------------------------------------

pub struct ComponentPod<C: Component> {
    label: Option<String>,

    component: Arc<Mutex<C>>,
}

impl<C: Component> ComponentPod<C> {
    pub fn new(label: Option<&str>, component: C) -> Self {
        Self {
            label: label.map(|s| s.to_string()),
            component: Arc::new(Mutex::new(component)),
        }
    }
}

impl<C: Component> ComponentPod<C> {
    fn update(&mut self, message: C::Message, ctx: &dyn WidgetContext) {
        self.component.lock().update(message, ctx);
    }

    fn view(&mut self, ctx: &dyn WidgetContext) -> WidgetPod<C::Event> {
        self.component.lock().view(ctx)
    }
}

// ----------------------------------------------------------------------------
// ComponentView
// ----------------------------------------------------------------------------

pub struct ComponentView<Event: Send + Sync + 'static, InnerEvent: Send + Sync + 'static> {
    label: Option<String>,
    component: Arc<Mutex<dyn AnyComponent<Event, InnerEvent>>>,
    inner_view: Box<dyn View<InnerEvent>>,
}

impl<Event: Send + Sync + 'static, InnerEvent: Send + Sync + 'static> View<Event>
    for ComponentView<Event, InnerEvent>
{
    fn build(&self) -> WidgetPod<Event> {
        WidgetPod::new(
            self.label.as_deref(),
            ComponentWidget {
                component: self.component.clone(),
                inner_widget: self.inner_view.build(),
            },
        )
    }
}

// ----------------------------------------------------------------------------
// ComponentWidget
// ----------------------------------------------------------------------------

struct ComponentWidget<Event: Send + Sync + 'static, InnerEvent: Send + Sync + 'static> {
    component: Arc<Mutex<dyn AnyComponent<Event, InnerEvent>>>,
    inner_widget: WidgetPod<InnerEvent>,
}

impl<Event: Send + Sync + 'static, InnerEvent: Send + Sync + 'static> Widget<Event>
    for ComponentWidget<Event, InnerEvent>
{
    type View = ComponentView<Event, InnerEvent>;

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
    ) -> (Option<Event>, WidgetInteractionResult) {
        self.component.lock().input(event, ctx);

        let (inner_event, interaction_result) = self.inner_widget.device_input(bounds, event, ctx);

        if let Some(inner_event) = inner_event {
            let event = self.component.lock().event(inner_event, ctx);
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
