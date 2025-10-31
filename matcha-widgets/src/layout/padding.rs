use nalgebra::Matrix4;

use matcha_core::context::WidgetContext;
use matcha_core::{
    device_input::DeviceInput,
    metrics::{Arrangement, Constraints},
    ui::{AnyWidget, AnyWidgetFrame, Background, Dom, InvalidationHandle, Widget, WidgetFrame},
};
use renderer::render_node::RenderNode;

pub struct Padding<T>
where
    T: Send + 'static,
{
    label: Option<String>,
    top: f32,
    right: f32,
    bottom: f32,
    left: f32,
    content: Option<Box<dyn Dom<T>>>,
}

impl<T> Padding<T>
where
    T: Send + 'static,
{
    pub fn new() -> Self {
        Self {
            label: None,
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
            content: None,
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn top(mut self, top: f32) -> Self {
        self.top = top;
        self
    }

    pub fn right(mut self, right: f32) -> Self {
        self.right = right;
        self
    }

    pub fn bottom(mut self, bottom: f32) -> Self {
        self.bottom = bottom;
        self
    }

    pub fn left(mut self, left: f32) -> Self {
        self.left = left;
        self
    }

    pub fn content(mut self, content: impl Dom<T>) -> Self {
        self.content = Some(Box::new(content));
        self
    }
}

#[async_trait::async_trait]
impl<T> Dom<T> for Padding<T>
where
    T: Send + 'static,
{
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
            PaddingNode {
                top: self.top,
                right: self.right,
                bottom: self.bottom,
                left: self.left,
            },
        ))
    }
}

pub struct PaddingNode {
    top: f32,
    right: f32,
    bottom: f32,
    left: f32,
}

impl<T> Widget<Padding<T>, T, ()> for PaddingNode
where
    T: Send + 'static,
{
    fn update_widget<'a>(
        &mut self,
        dom: &'a Padding<T>,
        cache_invalidator: Option<InvalidationHandle>,
    ) -> Vec<(&'a dyn Dom<T>, (), u128)> {
        if self.right != dom.right
            || self.top != dom.top
            || self.bottom != dom.bottom
            || self.left != dom.left
        {
            cache_invalidator.map(|h| h.relayout_next_frame());
        }
        self.top = dom.top;
        self.right = dom.right;
        self.bottom = dom.bottom;
        self.left = dom.left;

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
        } else {
            None
        }
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
        let content_size = if let Some((child, _)) = children.first() {
            let inner_constraints = Constraints::new(
                [
                    (constraints.min_width() - self.left - self.right).max(0.0),
                    (constraints.max_width() - self.left - self.right).max(0.0),
                ],
                [
                    (constraints.min_height() - self.top - self.bottom).max(0.0),
                    (constraints.max_height() - self.top - self.bottom).max(0.0),
                ],
            );
            child.measure(&inner_constraints, ctx)
        } else {
            [0.0, 0.0]
        };

        [
            content_size[0] + self.left + self.right,
            content_size[1] + self.top + self.bottom,
        ]
    }

    fn arrange(
        &self,
        bounds: [f32; 2],
        children: &[(&dyn AnyWidget<T>, &())],
        _ctx: &WidgetContext,
    ) -> Vec<Arrangement> {
        if children.is_empty() {
            return vec![];
        }

        let content_final_size = [
            (bounds[0] - self.left - self.right).max(0.0),
            (bounds[1] - self.top - self.bottom).max(0.0),
        ];

        let transform = Matrix4::new_translation(&nalgebra::Vector3::new(self.left, self.top, 0.0));

        vec![Arrangement::new(content_final_size, transform)]
    }

    fn render(
        &self,
        bounds: [f32; 2],
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
