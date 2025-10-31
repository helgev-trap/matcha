use matcha_core::context::WidgetContext;
use matcha_core::{
    device_input::DeviceInput,
    metrics::{Arrangement, Constraints},
    ui::{AnyWidget, AnyWidgetFrame, Background, Dom, InvalidationHandle, Widget, WidgetFrame},
};
use renderer::render_node::RenderNode;

use crate::types::size::{ChildSize, Size};

/// DOM node: Space
pub struct Space {
    label: Option<String>,
    width: Size,
    height: Size,
}

impl Space {
    pub fn new(label: Option<&str>) -> Box<Self> {
        Box::new(Self {
            label: label.map(|s| s.to_string()),
            width: Size::px(0.0),
            height: Size::px(0.0),
        })
    }

    pub fn width(mut self, width: Size) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: Size) -> Self {
        self.height = height;
        self
    }
}

#[async_trait::async_trait]
impl<T: Send + 'static> Dom<T> for Space {
    fn build_widget_tree(&self) -> Box<dyn AnyWidgetFrame<T>> {
        Box::new(WidgetFrame::new(
            self.label.clone(),
            vec![],
            vec![],
            SpaceNode {
                label: self.label.clone(),
                width: self.width.clone(),
                height: self.height.clone(),
            },
        ))
    }
}

/// Widget implementation for Space
pub struct SpaceNode {
    label: Option<String>,
    width: Size,
    height: Size,
}

impl<T: Send + 'static> Widget<Space, T, ()> for SpaceNode {
    fn update_widget<'a>(
        &mut self,
        dom: &'a Space,
        cache_invalidator: Option<InvalidationHandle>,
    ) -> Vec<(&'a dyn Dom<T>, (), u128)> {
        if self.width != dom.width || self.height != dom.height {
            if let Some(handle) = cache_invalidator {
                handle.relayout_next_frame();
            }
        }
        self.label = dom.label.clone();
        self.width = dom.width.clone();
        self.height = dom.height.clone();
        // no children
        vec![]
    }

    fn device_input(
        &mut self,
        _bounds: [f32; 2],
        _event: &DeviceInput,
        _children: &mut [(&mut dyn AnyWidget<T>, &mut (), &Arrangement)],
        _cache_invalidator: InvalidationHandle,
        _ctx: &WidgetContext,
    ) -> Option<T> {
        None
    }

    fn is_inside(
        &self,
        bounds: [f32; 2],
        position: [f32; 2],
        _children: &[(&dyn AnyWidget<T>, &(), &Arrangement)],
        _ctx: &WidgetContext,
    ) -> bool {
        position[0] >= 0.0
            && position[0] < bounds[0]
            && position[1] >= 0.0
            && position[1] < bounds[1]
    }

    fn measure(
        &self,
        constraints: &Constraints,
        _children: &[(&dyn AnyWidget<T>, &())],
        ctx: &WidgetContext,
    ) -> [f32; 2] {
        let parent_size = [constraints.max_width(), constraints.max_height()];

        let mut child_size = ChildSize::with_size([0.0, 0.0]);

        let w = self.width.size(parent_size, &mut child_size, ctx);
        let h = self.height.size(parent_size, &mut child_size, ctx);

        let w = w.clamp(constraints.min_width(), constraints.max_width());
        let h = h.clamp(constraints.min_height(), constraints.max_height());

        [w, h]
    }

    fn arrange(
        &self,
        _bounds: [f32; 2],
        _children: &[(&dyn AnyWidget<T>, &())],
        _ctx: &WidgetContext,
    ) -> Vec<Arrangement> {
        // No children to arrange
        vec![]
    }

    fn render(
        &self,
        _bounds: [f32; 2],
        _children: &[(&dyn AnyWidget<T>, &(), &Arrangement)],
        _background: Background,
        _ctx: &WidgetContext,
    ) -> RenderNode {
        RenderNode::default()
    }
}
