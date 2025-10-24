use std::vec;

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

// MARK: DOM

pub struct Text {
    label: Option<String>,

    sentence: crate::style::text::Sentence,
    font_size: f32,
    line_height: f32,
}

impl Text {
    pub fn new(s: &str) -> Self {
        Self {
            label: None,
            sentence: crate::style::text::Sentence::new(s),
            font_size: 14.0,
            line_height: 20.0,
        }
    }

    pub fn label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    pub fn color(mut self, color: matcha_core::color::Color) -> Self {
        self.sentence = self.sentence.color(color);
        self
    }

    pub fn family(mut self, family: crate::style::text::TextFamily) -> Self {
        self.sentence = self.sentence.family(family);
        self
    }

    pub fn stretch(mut self, stretch: crate::style::text::TextStretch) -> Self {
        self.sentence = self.sentence.stretch(stretch);
        self
    }

    pub fn style(mut self, style: crate::style::text::TextStyle) -> Self {
        self.sentence = self.sentence.style(style);
        self
    }

    pub fn weight(mut self, weight: crate::style::text::TextWeight) -> Self {
        self.sentence = self.sentence.weight(weight);
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn line_height(mut self, height: f32) -> Self {
        self.line_height = height;
        self
    }
}

#[async_trait::async_trait]
impl<T: Send + Sync + 'static> Dom<T> for Text {
    fn build_widget_tree(&self) -> Box<dyn AnyWidgetFrame<T>> {
        let text_desc = crate::style::text::TextDesc::new(vec![self.sentence.clone()])
            .font_size(self.font_size)
            .line_height(self.line_height);

        Box::new(WidgetFrame::new(
            self.label.clone(),
            vec![],
            vec![],
            TextWidget {
                label: self.label.clone(),
                clear: crate::style::viewport_clear::ViewportClear {
                    color: matcha_core::color::Color::TRANSPARENT,
                },
                style: crate::style::text::Text::new(&text_desc),
            },
        ))
    }
}

// MARK: Widget

pub struct TextWidget {
    label: Option<String>,
    clear: crate::style::viewport_clear::ViewportClear,
    style: crate::style::text::Text,
}

impl<E: Send + Sync + 'static> Widget<Text, E, ()> for TextWidget {
    fn update_widget<'b>(
        &mut self,
        dom: &'b Text,
        cache_invalidator: Option<InvalidationHandle>,
    ) -> Vec<(&'b dyn Dom<E>, (), u128)> {
        // Build a TextDesc like Dom::build_widget_tree does and create a new style
        let text_desc = crate::style::text::TextDesc::new(vec![dom.sentence.clone()])
            .font_size(dom.font_size)
            .line_height(dom.line_height);

        let new_style = crate::style::text::Text::new(&text_desc);

        // If visible text metrics changed, request relayout
        if !self.style.eq_desc(&text_desc)
            && let Some(handle) = cache_invalidator
        {
            handle.relayout_next_frame();
        }

        self.label = dom.label.clone();
        self.style = new_style;

        // No children
        vec![]
    }

    fn measure(
        &self,
        constraints: &Constraints,
        _: &[(&dyn AnyWidget<E>, &())],
        ctx: &WidgetContext,
    ) -> [f32; 2] {
        let rect = self.style.required_region(constraints, ctx);
        if let Some(rect) = rect {
            [rect.width(), rect.height()]
        } else {
            [0.0, 0.0]
        }
    }

    fn arrange(
        &self,
        _bounds: [f32; 2],
        _children: &[(&dyn AnyWidget<E>, &())],
        _ctx: &WidgetContext,
    ) -> Vec<Arrangement> {
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
        None
    }

    fn is_inside(
        &self,
        bounds: [f32; 2],
        position: [f32; 2],
        _children: &[(&dyn AnyWidget<E>, &(), &Arrangement)],
        ctx: &WidgetContext,
    ) -> bool {
        self.style.is_inside(position, bounds, ctx)
    }

    fn render(
        &self,
        bounds: [f32; 2],
        _children: &[(&dyn AnyWidget<E>, &(), &Arrangement)],
        _background: Background,
        ctx: &WidgetContext,
    ) -> RenderNode {
        let mut render_node = RenderNode::new();
        let size = <Self as Widget<Text, E, ()>>::measure(
            self,
            &Constraints::from_boundary(bounds),
            &[],
            ctx,
        );

        if size[0] > 0.0 && size[1] > 0.0 {
            let texture_size = [size[0].ceil() as u32, size[1].ceil() as u32];

            if let Ok(style_region) =
                ctx.texture_atlas()
                    .lock()
                    .allocate(&ctx.device(), &ctx.queue(), texture_size)
            {
                let mut encoder =
                    ctx.device()
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Text Render Encoder"),
                        });

                // clear the texture to transparent
                self.clear
                    .draw(&mut encoder, &style_region, size, [0.0, 0.0], ctx);

                self.style
                    .draw(&mut encoder, &style_region, size, [0.0, 0.0], ctx);

                ctx.queue().submit(Some(encoder.finish()));
                render_node =
                    render_node.with_texture(style_region, size, nalgebra::Matrix4::identity());
            }
        }

        render_node
    }
}
