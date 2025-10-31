use matcha_core::{
    context::WidgetContext,
    device_input::DeviceInput,
    metrics::{Arrangement, Constraints},
    ui::{
        AnyWidgetFrame, Background, Dom, Widget, WidgetFrame,
        widget::{AnyWidget, InvalidationHandle},
    },
};
use renderer::render_node::RenderNode;

// todo: more documentation

// MARK: DOM

pub struct Template {
    label: Option<String>,
}

impl Template {
    pub fn new(label: Option<&str>) -> Box<Self> {
        Box::new(Self {
            label: label.map(|s| s.to_string()),
        })
    }
}

#[async_trait::async_trait]
impl<E: Send + 'static> Dom<E> for Template {
    fn build_widget_tree(&self) -> Box<dyn AnyWidgetFrame<E>> {
        Box::new(WidgetFrame::new(
            self.label.clone(),
            vec![],
            vec![],
            TemplateNode {
                label: self.label.clone(),
            },
        ))
    }
}

// MARK: Widget

pub struct TemplateNode {
    label: Option<String>,
}

// MARK: Widget trait

impl<E: Send + 'static> Widget<Template, E, ()> for TemplateNode {
    fn update_widget<'a>(
        &mut self,
        dom: &'a Template,
        _cache_invalidator: Option<InvalidationHandle>,
    ) -> Vec<(&'a dyn Dom<E>, (), u128)> {
        // In a real widget, you would compare properties here.
        // If properties are different, call handle.relayout_next_frame() or handle.redraw_next_frame().
        self.label = dom.label.clone();
        // This widget has no children, so return an empty vec.
        vec![]
    }

    fn device_input(
        &mut self,
        _bounds: [f32; 2],
        _event: &DeviceInput,
        _children: &mut [(&mut dyn AnyWidget<E>, &mut (), &Arrangement)],
        _cache_invalidator: InvalidationHandle,
        _ctx: &WidgetContext,
    ) -> Option<E> {
        // Handle device events here.
        None
    }

    fn is_inside(
        &self,
        _bounds: [f32; 2],
        _position: [f32; 2],
        _children: &[(&dyn AnyWidget<E>, &(), &Arrangement)],
        _ctx: &WidgetContext,
    ) -> bool {
        // Implement this if your widget has a non-rectangular shape or transparent areas.
        // For a simple template, we can assume it's always inside its bounds.
        true
    }

    fn measure(
        &self,
        _constraints: &Constraints,
        _children: &[(&dyn AnyWidget<E>, &())],
        _ctx: &WidgetContext,
    ) -> [f32; 2] {
        // This widget has no content, so it takes up no space.
        [0.0, 0.0]
    }

    fn arrange(
        &self,
        _bounds: [f32; 2],
        _children: &[(&dyn AnyWidget<E>, &())],
        _ctx: &WidgetContext,
    ) -> Vec<Arrangement> {
        // This widget has no children to arrange.
        vec![]
    }

    fn render(
        &self,
        _bounds: [f32; 2],
        _children: &[(&dyn AnyWidget<E>, &(), &Arrangement)],
        _background: Background,
        _ctx: &WidgetContext,
    ) -> RenderNode {
        // This widget doesn't draw anything.
        RenderNode::new()
    }
}
