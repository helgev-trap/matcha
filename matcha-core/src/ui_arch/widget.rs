use std::any::Any;

use renderer::render_node::RenderNode;

use crate::event::device_event::DeviceEvent;
use super::metrics;

// ----------------------------------------------------------------------------
// Types
// ----------------------------------------------------------------------------

/// Represents an error that can occur when updating a `Widget` tree.
pub enum WidgetUpdateError {
    /// Occurs when the type of the new `Dom` node does not match the existing `Widget`.
    TypeMismatch,
}

pub enum WidgetInteractionResult {
    // Need to rearrange layout (and also redraw).
    LayoutNeeded,
    // Need to redraw.
    RedrawNeeded,
    // No change.
    NoChange,
}

// ----------------------------------------------------------------------------
// Key Structs and Traits
// ----------------------------------------------------------------------------

pub trait WidgetContext {}

pub trait View<T: 'static>: Send + Sync + Any {
    fn build(&self) -> WidgetPod<T>;
}

pub trait Widget<T: 'static>: Send + Sync + Any {
    type View: View<T>;

    fn update(&mut self, view: &Self::View) -> WidgetInteractionResult;

    fn device_input(&mut self, bounds: [f32; 2], event: &DeviceEvent, ctx: &dyn WidgetContext) -> (Option<T>, WidgetInteractionResult);

    fn is_inside(&self, bounds: [f32; 2], position: [f32; 2], ctx: &dyn WidgetContext) -> bool {
        let _ = ctx;

        0.0 <= position[0]
            && position[0] <= bounds[0]
            && 0.0 <= position[1]
            && position[1] <= bounds[1]
    }

    fn measure(&self, constraints: &metrics::Constraints, ctx: &dyn WidgetContext) -> [f32; 2];

    fn render(&mut self, bounds: [f32; 2], ctx: &dyn WidgetContext) -> RenderNode;
}

/// Wrapper trait to erase generic type T from Widget trait.
pub(super) trait AnyWidget<T>: Send + Sync + Any {
    fn try_update(&mut self, view: &dyn View<T>) -> Result<WidgetInteractionResult, WidgetUpdateError>;

    fn device_input(&mut self, bounds: [f32; 2], event: &DeviceEvent, ctx: &dyn WidgetContext) -> (Option<T>, WidgetInteractionResult);

    fn is_inside(&self, bounds: [f32; 2], position: [f32; 2], ctx: &dyn WidgetContext) -> bool;

    fn measure(&self, constraints: &metrics::Constraints, ctx: &dyn WidgetContext) -> [f32; 2];

    fn render(&mut self, bounds: [f32; 2], /* TODO: background: Background, */ ctx: &dyn WidgetContext) -> RenderNode;
}

impl<W, V, T: 'static> AnyWidget<T> for W
where
    W: Widget<T, View = V>,
    V: View<T>,
{
    fn try_update(&mut self, view: &dyn View<T>) -> Result<WidgetInteractionResult, WidgetUpdateError> {
        let Some(view) = (view as &dyn Any).downcast_ref::<V>() else {
            return Err(WidgetUpdateError::TypeMismatch);
        };

        Ok(self.update(view))
    }

    fn device_input(&mut self, bounds: [f32; 2], event: &DeviceEvent, ctx: &dyn WidgetContext) -> (Option<T>, WidgetInteractionResult) {
        Widget::device_input(self, bounds, event, ctx)
    }

    fn is_inside(&self, bounds: [f32; 2], position: [f32; 2], ctx: &dyn WidgetContext) -> bool {
        Widget::is_inside(self, bounds, position, ctx)
    }

    fn measure(&self, constraints: &metrics::Constraints, ctx: &dyn WidgetContext) -> [f32; 2] {
        Widget::measure(self, constraints, ctx)
    }

    fn render(&mut self, bounds: [f32; 2], ctx: &dyn WidgetContext) -> RenderNode {
        Widget::render(self, bounds, ctx)
    }
}

pub struct WidgetPod<T: 'static> {
    label: Option<String>,
    id_hash: usize,

    widget: Box<dyn AnyWidget<T>>,

    // cache
    // NOTE: Use cache existency as redraw flag.
    render_cache: Option<RenderNode>,
}

impl<T> WidgetPod<T> {
    pub fn new(id: impl std::hash::Hash, widget: impl Widget<T>) -> Self {
        let mut hasher = fxhash::FxHasher::default();
        id.hash(&mut hasher);
        let id_hash = fxhash::hash(&id);

        Self {
            label: None,
            id_hash,
            widget: Box::new(widget),
            render_cache: None,
        }
    }

    pub fn id_hash(&self) -> usize {
        self.id_hash
    }
}

impl<T> WidgetPod<T> {
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    fn need_redraw(&self) -> bool {
        self.render_cache.is_none()
    }

    fn invalidate_render_cache(&mut self) {
        self.render_cache = None;
    }
}

impl<T: 'static> WidgetPod<T> {
    pub fn try_update(&mut self, view: &dyn View<T>) -> Result<WidgetInteractionResult, WidgetUpdateError> {
        self.widget.try_update(view)
    }

    pub fn device_input(&mut self, bounds: [f32; 2], event: &DeviceEvent, ctx: &dyn WidgetContext) -> (Option<T>, WidgetInteractionResult) {
        let (event, interaction_result) = self.widget.device_input(bounds, event, ctx);

        match interaction_result {
            WidgetInteractionResult::LayoutNeeded => {
                self.invalidate_render_cache();
            }
            WidgetInteractionResult::RedrawNeeded => {
                self.invalidate_render_cache();
            }
            WidgetInteractionResult::NoChange => {}
        }

        (event, interaction_result)
    }

    pub fn is_inside(&self, bounds: [f32; 2], position: [f32; 2], ctx: &dyn WidgetContext) -> bool {
        self.widget.is_inside(bounds, position, ctx)
    }

    pub fn measure(&self, constraints: &metrics::Constraints, ctx: &dyn WidgetContext) -> [f32; 2] {
        self.widget.measure(constraints, ctx)
    }

    pub fn render(&mut self, bounds: [f32; 2], ctx: &dyn WidgetContext) -> RenderNode {
        if let Some(render_node) = &self.render_cache {
            return render_node.clone();
        }

        let render_node = self.widget.render(bounds, ctx);
        self.render_cache = Some(render_node.clone());
        render_node
    }
}
