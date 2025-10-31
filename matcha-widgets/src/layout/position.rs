use nalgebra::Matrix4;

use matcha_core::context::WidgetContext;
use matcha_core::{
    device_input::DeviceInput,
    metrics::{Arrangement, Constraints},
    ui::{AnyWidget, AnyWidgetFrame, Background, Dom, InvalidationHandle, Widget, WidgetFrame},
};
use renderer::render_node::RenderNode;

// MARK: DOM

pub struct Position<T: Send + 'static> {
    label: Option<String>,
    left: Option<f32>,
    top: Option<f32>,
    right: Option<f32>,
    bottom: Option<f32>,
    content: Option<Box<dyn Dom<T>>>,
}

impl<T: Send + 'static> Position<T> {
    pub fn new() -> Self {
        Self {
            label: None,
            left: None,
            top: None,
            right: None,
            bottom: None,
            content: None,
        }
    }

    pub fn left(mut self, left: f32) -> Self {
        self.left = Some(left);
        self
    }

    pub fn top(mut self, top: f32) -> Self {
        self.top = Some(top);
        self
    }

    pub fn right(mut self, right: f32) -> Self {
        self.right = Some(right);
        self
    }

    pub fn bottom(mut self, bottom: f32) -> Self {
        self.bottom = Some(bottom);
        self
    }

    pub fn content(mut self, content: impl Dom<T>) -> Self {
        self.content = Some(Box::new(content));
        self
    }
}

#[async_trait::async_trait]
impl<T: Send + 'static> Dom<T> for Position<T> {
    fn build_widget_tree(&self) -> Box<dyn AnyWidgetFrame<T>> {
        let mut children_and_settings = Vec::new();
        let mut child_ids = Vec::new();

        if let Some(content_widget) = self.content.as_ref().map(|c| c.build_widget_tree()) {
            children_and_settings.push((content_widget, ()));
            child_ids.push(0);
        }

        Box::new(WidgetFrame::new(
            self.label.clone(),
            children_and_settings,
            child_ids,
            PositionNode {
                left: self.left,
                top: self.top,
                right: self.right,
                bottom: self.bottom,
            },
        ))
    }
}

// MARK: Widget

pub struct PositionNode {
    left: Option<f32>,
    top: Option<f32>,
    right: Option<f32>,
    bottom: Option<f32>,
}

impl<T: Send + 'static> Widget<Position<T>, T, ()> for PositionNode {
    fn update_widget<'a>(
        &mut self,
        dom: &'a Position<T>,
        _cache_invalidator: Option<InvalidationHandle>,
    ) -> Vec<(&'a dyn Dom<T>, (), u128)> {
        self.left = dom.left;
        self.top = dom.top;
        self.right = dom.right;
        self.bottom = dom.bottom;
        dom.content
            .as_ref()
            .map(|c| (c.as_ref(), (), 0))
            .into_iter()
            .collect()
    }

    fn device_input(
        &mut self,
        _bounds: [f32; 2],
        event: &DeviceInput,
        children: &mut [(&mut dyn AnyWidget<T>, &mut (), &Arrangement)],
        _cache_invalidator: InvalidationHandle,
        ctx: &WidgetContext,
    ) -> Option<T> {
        if let Some((child, _, arrangement)) = children.first_mut() {
            let child_event = event.transform(arrangement.affine);
            return child.device_input(&child_event, ctx);
        }
        None
    }

    fn is_inside(
        &self,
        bounds: [f32; 2],
        position: [f32; 2],
        _children: &[(&dyn AnyWidget<T>, &(), &Arrangement)],
        _ctx: &WidgetContext,
    ) -> bool {
        0.0 <= position[0]
            && position[0] <= bounds[0]
            && 0.0 <= position[1]
            && position[1] <= bounds[1]
    }

    fn measure(
        &self,
        constraints: &Constraints,
        children: &[(&dyn AnyWidget<T>, &())],
        ctx: &WidgetContext,
    ) -> [f32; 2] {
        let mut width = constraints.width();
        let mut height = constraints.height();

        // left
        if let Some(left) = self.left {
            width[0] = (width[0] - left).max(0.0);
            width[1] = (width[1] - left).max(0.0);
        }

        // top
        if let Some(top) = self.top {
            height[0] = (height[0] - top).max(0.0);
            height[1] = (height[1] - top).max(0.0);
        }

        // right
        if let Some(right) = self.right {
            width[0] = (width[0] - right).max(0.0);
            width[1] = (width[1] - right).max(0.0);
        }

        // bottom
        if let Some(bottom) = self.bottom {
            height[0] = (height[0] - bottom).max(0.0);
            height[1] = (height[1] - bottom).max(0.0);
        }

        let child_constraints = Constraints::new(width, height);

        let child_measured_size = if let Some((child, _)) = children.first() {
            child.measure(&child_constraints, ctx)
        } else {
            [0.0, 0.0]
        };

        let measured_width =
            child_measured_size[0] + self.left.unwrap_or(0.0) + self.right.unwrap_or(0.0);
        let measured_height =
            child_measured_size[1] + self.top.unwrap_or(0.0) + self.bottom.unwrap_or(0.0);

        [measured_width, measured_height]
    }

    fn arrange(
        &self,
        bounds: [f32; 2],
        children: &[(&dyn AnyWidget<T>, &())],
        ctx: &WidgetContext,
    ) -> Vec<Arrangement> {
        let Some((content, _)) = children.first() else {
            return vec![];
        };

        // available space for child (parent size minus margins)
        let available = [
            (bounds[0] - self.left.unwrap_or(0.0) - self.right.unwrap_or(0.0)).max(0.0),
            (bounds[1] - self.top.unwrap_or(0.0) - self.bottom.unwrap_or(0.0)).max(0.0),
        ];

        // give child a flexible constraint up to available space
        let content_constraints = Constraints::new([0.0, available[0]], [0.0, available[1]]);
        let content_measured_size = content.measure(&content_constraints, ctx);

        // final child size: clamp to available (defensive)
        let final_child_size = [
            content_measured_size[0].clamp(0.0, available[0]),
            content_measured_size[1].clamp(0.0, available[1]),
        ];

        let offset_x = match (self.left, self.right) {
            (Some(left), _) => left,
            (None, Some(right)) => bounds[0] - right - final_child_size[0],
            (None, None) => 0.0,
        };
        let offset_y = match (self.top, self.bottom) {
            (Some(top), _) => top,
            (None, Some(bottom)) => bounds[1] - bottom - final_child_size[1],
            (None, None) => 0.0,
        };

        vec![Arrangement::new(
            final_child_size,
            Matrix4::new_translation(&nalgebra::Vector3::new(offset_x, offset_y, 0.0)),
        )]
    }

    fn render(
        &self,
        _bounds: [f32; 2],
        children: &[(&dyn AnyWidget<T>, &(), &Arrangement)],
        background: Background,
        ctx: &WidgetContext,
    ) -> RenderNode {
        if let Some((child, _, arrangement)) = children.first() {
            let affine = arrangement.affine;

            let child_node = child.render(background, ctx);

            return RenderNode::new().add_child(child_node, affine);
        }
        RenderNode::default()
    }
}
