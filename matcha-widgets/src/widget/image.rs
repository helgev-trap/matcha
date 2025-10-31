use crate::style::Style;
use matcha_core::context::WidgetContext;
use matcha_core::{
    device_input::DeviceInput,
    metrics::{Arrangement, Constraints},
    ui::{
        AnyWidgetFrame, Background, Dom, Widget, WidgetFrame,
        widget::{AnyWidget, InvalidationHandle},
    },
};
use renderer::render_node::RenderNode;

use crate::{style, types::size::Size};
use nalgebra::Matrix4;

// MARK: DOM

pub struct Image {
    label: Option<String>,
    image_style: style::image::Image,
}

impl Image {
    pub fn new(image: impl Into<style::image::ImageSource>) -> Self {
        Self {
            label: None,
            image_style: style::image::Image::new(image),
        }
    }

    pub fn label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    pub fn size(mut self, size: [Size; 2]) -> Self {
        self.image_style = self.image_style.size(size);
        self
    }
}

#[async_trait::async_trait]
impl<T: Send + Sync + 'static> Dom<T> for Image {
    fn build_widget_tree(&self) -> Box<dyn AnyWidgetFrame<T>> {
        Box::new(WidgetFrame::new(
            self.label.clone(),
            vec![],
            vec![],
            ImageNode {
                image_style: self.image_style.clone(),
            },
        ))
    }
}

// MARK: Widget

#[derive(Clone)]
pub struct ImageNode {
    image_style: style::image::Image,
}

impl<T: Send + Sync + 'static> Widget<Image, T, ()> for ImageNode {
    fn update_widget<'a>(
        &mut self,
        dom: &'a Image,
        cache_invalidator: Option<InvalidationHandle>,
    ) -> Vec<(&'a dyn Dom<T>, (), u128)> {
        // A proper implementation would require making the key public or implementing PartialEq.
        if self.image_style != dom.image_style {
            if let Some(handle) = cache_invalidator {
                handle.relayout_next_frame();
            }
        }
        self.image_style = dom.image_style.clone();
        vec![]
    }

    fn measure(
        &self,
        constraints: &Constraints,
        _children: &[(&dyn AnyWidget<T>, &())],
        ctx: &WidgetContext,
    ) -> [f32; 2] {
        let size = self
            .image_style
            .required_region(constraints, ctx)
            .unwrap_or_default();

        [size.max_x(), size.max_y()]
    }

    fn arrange(
        &self,
        _bounds: [f32; 2],
        _children: &[(&dyn AnyWidget<T>, &())],
        _ctx: &WidgetContext,
    ) -> Vec<Arrangement> {
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
            && position[0] <= bounds[0]
            && position[1] >= 0.0
            && position[1] <= bounds[1]
    }

    fn render(
        &self,
        _bounds: [f32; 2],
        _children: &[(&dyn AnyWidget<T>, &(), &Arrangement)],
        _background: Background,
        ctx: &WidgetContext,
    ) -> RenderNode {
        let mut render_node = RenderNode::new();
        let size = <Self as Widget<Image, T, ()>>::measure(
            self,
            &Constraints::new([0.0f32, f32::INFINITY], [0.0f32, f32::INFINITY]),
            &[],
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
                            label: Some("Image Render Encoder"),
                        });

                self.image_style
                    .draw(&mut encoder, &style_region, size, [0.0, 0.0], ctx);

                ctx.queue().submit(Some(encoder.finish()));
                render_node = render_node.with_texture(style_region, size, Matrix4::identity())
            }
        }

        render_node
    }
}
