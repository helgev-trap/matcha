use crate::style::Style as _;
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
use std::vec;

pub use crate::style::text::TextSpan;
pub use crate::style::text::{Family, Stretch, Style, Weight};

// MARK: DOM

pub struct Text {
    label: Option<String>,

    text: crate::style::text::TextRenderer,
}

impl Text {
    pub fn new(label: Option<&str>) -> Self {
        Self {
            label: label.map(|s| s.to_string()),
            text: crate::style::text::TextRenderer::new(),
        }
    }

    pub fn label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    pub fn push_span(mut self, span: TextSpan) -> Self {
        self.text.push_span(span);
        self
    }
}

#[async_trait::async_trait]
impl<T: Send + Sync + 'static> Dom<T> for Text {
    fn build_widget_tree(&self) -> Box<dyn AnyWidgetFrame<T>> {
        Box::new(WidgetFrame::new(
            self.label.clone(),
            vec![],
            vec![],
            TextWidget {
                clear: crate::style::viewport_clear::ViewportClear {
                    color: matcha_core::color::Color::TRANSPARENT,
                },
                text: self.text.clone(),
            },
        ))
    }
}

// MARK: Widget

pub struct TextWidget {
    clear: crate::style::viewport_clear::ViewportClear,
    text: crate::style::text::TextRenderer,
}

impl<E: Send + Sync + 'static> Widget<Text, E, ()> for TextWidget {
    fn update_widget<'a>(
        &mut self,
        dom: &'a Text,
        cache_invalidator: Option<InvalidationHandle>,
    ) -> Vec<(&'a dyn Dom<E>, (), u128)> {
        // If visible text metrics changed, request relayout
        if self.text != dom.text
            && let Some(handle) = cache_invalidator
        {
            handle.relayout_next_frame();
            self.text = dom.text.clone();
        }

        // No children
        vec![]
    }

    fn measure(
        &self,
        constraints: &Constraints,
        _: &[(&dyn AnyWidget<E>, &())],
        ctx: &WidgetContext,
    ) -> [f32; 2] {
        let rect = self.text.required_region(constraints, ctx);

        match rect {
            Some(r) => [r.width(), r.height()],
            None => unreachable!("Text style always provides required region."),
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
        self.text.is_inside(position, bounds, ctx)
    }

    fn render(
        &self,
        bounds: [f32; 2],
        _children: &[(&dyn AnyWidget<E>, &(), &Arrangement)],
        _background: Background,
        ctx: &WidgetContext,
    ) -> RenderNode {
        let mut render_node = RenderNode::new();

        // NOTE: This did not work as expected:
        // // NOTE:
        // // It was observed that using the required_region computed during measure as the render
        // // boundary does not always produce the same layout from cosmic-text; adding a few pixels
        // // sometimes has no effect. To try to ensure the render produces the same layout as the
        // // measure pass, the widget currently increases the boundary/texture allocation by a margin
        // // equal to font_size before allocating the texture. The root cause appears to be internal
        // // to the library and is unknown; this comment only records the observed behavior and the
        // // pragmatic workaround.
        // let bounds = [
        //     bounds[0] + self.style.font_size / 2.0,
        //     bounds[1] + self.style.font_size / 2.0,
        // ];

        // // 上の問題に対処するために、引数として渡された `bounds` に収まる最大のmeasure結果を提供するようなConstraintsを探してcosmic-textへの入力サイズとする

        // let mut current_text_size = bounds;
        // let size_increment = self.style.font_size / 4.0;
        // for _ in 0..100 {
        //     let constraints = Constraints::from_boundary([
        //         current_text_size[0] + size_increment,
        //         current_text_size[1] + size_increment,
        //     ]);
        //     let Some(measured_size) = self.style.required_region(&constraints, ctx) else {
        //         unreachable!("Text style always provides required region.");
        //     };

        //     // 現状は横書きのみに対応
        //     // 横幅がboundsを超えたらループ終了し、横幅がboundsを超える直前のサイズをrender用に使う
        //     if measured_size.width() > bounds[0] + 1.0 / SUB_PIXEL_QUANTIZE {
        //         break;
        //     }

        //     current_text_size = [
        //         current_text_size[0] + size_increment,
        //         current_text_size[1] + size_increment,
        //     ];
        // }
        // let bounds = current_text_size;

        // // 上で決定したboundsに対してレンダリングを行う

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
                    .allocate(&ctx.device(), &ctx.queue(), texture_size)
            {
                let mut encoder =
                    ctx.device()
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("Text Render Encoder"),
                        });

                self.clear
                    .draw(&mut encoder, &style_region, size, [0.0, 0.0], ctx);

                self.text
                    .draw(&mut encoder, &style_region, size, [0.0, 0.0], ctx);

                ctx.queue().submit(Some(encoder.finish()));
                render_node =
                    render_node.with_texture(style_region, size, nalgebra::Matrix4::identity());
            }
        }

        render_node
    }
}
