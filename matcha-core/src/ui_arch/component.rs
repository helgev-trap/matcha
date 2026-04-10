use std::sync::Arc;

use renderer::RenderNode;

use super::widget::{View, Widget, WidgetInteractionResult, WidgetPod};
use crate::{
    event::device_event::DeviceEvent,
    ui_arch::{metrics, ui_context::UiContext, widget::WidgetUpdateError},
};

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
///
/// # Spawning tasks
///
/// Use `ctx.runtime_handle().spawn(...)` to launch background tasks.
pub trait Component: Send + Sync + 'static {
    /// Discrete commands delivered from the application layer.
    type Message: Send + Sync + 'static;
    /// Event propagated upward to the parent component.
    type Event: Send + Sync + 'static;
    /// Event emitted by child widgets, consumed by [`event()`](Component::event).
    type InnerEvent: Send + Sync + 'static;

    /// Called once when the component is first attached to the widget tree.
    fn setup(&self, ctx: &dyn UiContext);

    /// Called when a discrete [`Message`](Component::Message) is received.
    ///
    /// Typically used to trigger background tasks or write to [`SharedValue`](shared_buffer::SharedValue)
    /// fields. A redraw occurs only if `store()` is called.
    fn update(&self, message: Self::Message, ctx: &dyn UiContext);

    /// Builds the view tree from the current state.
    fn view(&self, ctx: &dyn UiContext) -> Box<dyn View<Self::InnerEvent>>;

    /// Translates a child widget's `InnerEvent` into an optional outward `Event`.
    fn event(&self, event: Self::InnerEvent, ctx: &dyn UiContext) -> Option<Self::Event>;

    /// Handles a raw device event (keyboard, mouse, etc.). Default implementation is a no-op.
    fn input(&self, device_event: &DeviceEvent, ctx: &dyn UiContext) {
        let _ = (device_event, ctx);
    }
}

// ----------------------------------------------------------------------------
// ComponentPod
// ----------------------------------------------------------------------------

/// Owns a [`Component`].
///
/// Held by the UI framework. Use [`ComponentPod::arc()`] to share the component
/// state with the backend.
pub struct ComponentPod<C: Component> {
    label: Option<String>,
    component: Arc<C>,
}

impl<C: Component> ComponentPod<C> {
    pub fn new(label: Option<&str>, component: C) -> Self {
        Self {
            label: label.map(|s| s.to_string()),
            component: Arc::new(component),
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
    pub fn setup(&self, ctx: &dyn UiContext) {
        self.component.setup(ctx);
    }

    /// Delivers a discrete [`Message`](Component::Message) to the component.
    pub fn update(&self, message: C::Message, ctx: &dyn UiContext) {
        self.component.update(message, ctx);
    }

    /// Builds a [`ComponentView`] from the current state.
    pub fn view(&self, ctx: &dyn UiContext) -> ComponentView<C> {
        ComponentView {
            label: self.label.clone(),
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
    component: Arc<C>,
    inner_view: Box<dyn View<C::InnerEvent>>,
}

impl<C: Component> View<C::Event> for ComponentView<C> {
    fn build(&self, ctx: &dyn UiContext) -> WidgetPod<C::Event> {
        WidgetPod::new(
            self.label.as_deref(),
            ComponentWidget {
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
    component: Arc<C>,
    inner_widget: WidgetPod<C::InnerEvent>,
}

impl<C: Component> Widget<C::Event> for ComponentWidget<C> {
    type View = ComponentView<C>;

    fn update(&mut self, view: &Self::View, ctx: &dyn UiContext) -> WidgetInteractionResult {
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
        ctx: &dyn UiContext,
    ) -> (Option<C::Event>, WidgetInteractionResult) {
        self.component.input(event, ctx);

        let (inner_event, interaction_result) = self.inner_widget.device_input(bounds, event, ctx);

        if let Some(inner_event) = inner_event {
            let event = self.component.event(inner_event, ctx);
            (event, interaction_result)
        } else {
            (None, interaction_result)
        }
    }

    fn is_inside(&self, bounds: [f32; 2], position: [f32; 2], ctx: &dyn UiContext) -> bool {
        self.inner_widget.is_inside(bounds, position, ctx)
    }

    fn measure(&self, constraints: &metrics::Constraints, ctx: &dyn UiContext) -> [f32; 2] {
        self.inner_widget.measure(constraints, ctx)
    }

    fn render(&mut self, bounds: [f32; 2], ctx: &dyn UiContext) -> RenderNode {
        self.inner_widget.render(bounds, ctx)
    }
}
