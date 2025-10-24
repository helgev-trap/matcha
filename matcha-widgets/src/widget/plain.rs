use std::sync::Arc;

use crate::style::Style;
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

use crate::{buffer::Buffer, types::size::Size};

// MARK: DOM

pub struct Plain<T> {
    label: Option<String>,
    style: Vec<Arc<dyn Style>>,
    content: Option<Box<dyn Dom<T>>>,
    size: [Size; 2],
}

impl<T> Plain<T> {
    pub fn new(label: Option<&str>) -> Box<Self> {
        Box::new(Self {
            label: label.map(|s| s.to_string()),
            style: Vec::new(),
            content: None,
            size: [Size::child_w(1.0), Size::child_h(1.0)],
        })
    }

    pub fn style(mut self, style: impl Style + 'static) -> Self {
        self.style.push(Arc::new(style));
        self
    }

    pub fn content(mut self, content: impl Dom<T>) -> Self {
        self.content = Some(Box::new(content));
        self
    }

    pub fn size(mut self, size: [Size; 2]) -> Self {
        self.size = size;
        self
    }
}

#[async_trait::async_trait]
impl<T: Send + Sync + 'static> Dom<T> for Plain<T> {
    fn build_widget_tree(&self) -> Box<dyn AnyWidgetFrame<T>> {
        let children = self
            .content
            .as_ref()
            .map(|c| (c.build_widget_tree(), ()))
            .into_iter()
            .collect();
        let child_ids = self.content.as_ref().map(|_| 0).into_iter().collect();

        Box::new(WidgetFrame::new(
            self.label.clone(),
            children,
            child_ids,
            PlainNode {
                style: self.style.clone(),
                size: self.size.clone(),
                buffer: Buffer::new(self.style.clone()),
                _phantom: std::marker::PhantomData,
            },
        ))
    }
}

// MARK: Widget

pub struct PlainNode<T> {
    style: Vec<Arc<dyn Style>>,
    size: [Size; 2],
    buffer: Buffer,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Send + Sync + 'static> Widget<Plain<T>, T, ()> for PlainNode<T> {
    fn update_widget<'a>(
        &mut self,
        dom: &'a Plain<T>,
        cache_invalidator: Option<InvalidationHandle>,
    ) -> Vec<(&'a dyn Dom<T>, (), u128)> {
        if self.size != dom.size {
            if let Some(handle) = cache_invalidator {
                handle.relayout_next_frame();
            }
        }
        self.style = dom.style.clone();
        self.size = dom.size.clone();
        self.buffer = Buffer::new(self.style.clone());

        dom.content
            .as_ref()
            .map(|c| (&**c, (), 0))
            .into_iter()
            .collect()
    }

    fn measure(
        &self,
        constraints: &Constraints,
        children: &[(&dyn AnyWidget<T>, &())],
        ctx: &WidgetContext,
    ) -> [f32; 2] {
        let child_size = if let Some((child, _)) = children.first() {
            child.measure(constraints, ctx)
        } else {
            [0.0, 0.0]
        };

        let mut child_size_provider = crate::types::size::ChildSize::new(|| child_size);
        let parent_size = [constraints.max_width(), constraints.max_height()];

        let w = self.size[0].size(parent_size, &mut child_size_provider, ctx);
        let h = self.size[1].size(parent_size, &mut child_size_provider, ctx);

        [w, h]
    }

    fn arrange(
        &self,
        bounds: [f32; 2],
        _children: &[(&dyn AnyWidget<T>, &())],
        _ctx: &WidgetContext,
    ) -> Vec<Arrangement> {
        vec![Arrangement::new(bounds, nalgebra::Matrix4::identity())]
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
        position[0] >= 0.0
            && position[0] <= bounds[0]
            && position[1] >= 0.0
            && position[1] <= bounds[1]
    }

    fn render(
        &self,
        _bounds: [f32; 2],
        children: &[(&dyn AnyWidget<T>, &(), &Arrangement)],
        background: Background,
        ctx: &WidgetContext,
    ) -> RenderNode {
        let mut render_node = RenderNode::new();
        let children_for_measure: Vec<(&dyn AnyWidget<T>, &())> =
            children.iter().map(|(c, _, _)| (*c, &())).collect();
        let size = self.measure(
            &Constraints::new([0.0f32, f32::INFINITY], [0.0f32, f32::INFINITY]),
            &children_for_measure,
            ctx,
        );

        if size[0] > 0.0 && size[1] > 0.0 {
            let texture_size = [size[0].ceil() as u32, size[1].ceil() as u32];
            if let Ok(style_region) =
                ctx.texture_atlas()
                    .allocate(&ctx.device(), &ctx.queue(), texture_size)
            {
                let mut encoder =
                    ctx.device()
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Plain Render Encoder"),
                        });

                for style in &self.style {
                    style.draw(&mut encoder, &style_region, size, [0.0, 0.0], ctx);
                }

                ctx.queue().submit(Some(encoder.finish()));
                render_node =
                    render_node.with_texture(style_region, size, nalgebra::Matrix4::identity());
            }
        }

        if let Some((child, _, arrangement)) = children.first() {
            let child_node = child.render(background, ctx);
            render_node.push_child(child_node, arrangement.affine);
        }

        render_node
    }
}
