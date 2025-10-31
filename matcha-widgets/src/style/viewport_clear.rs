use crate::style::Style;
use gpu_utils::texture_atlas::atlas_simple::atlas::AtlasRegion;
use matcha_core::{
    color::Color,
    context::WidgetContext,
    metrics::{Constraints, QRect},
};
use renderer::widgets_renderer::viewport_clear::ViewportClear as RendererViewportClear;

/// Clear the viewport region to a specified color.
///
/// This Style uses the renderer's `ViewportClear` pipeline and passes the clear color
/// via push constants.
pub struct ViewportClear {
    pub color: Color,
}

impl Style for ViewportClear {
    fn required_region(&self, constraints: &Constraints, _ctx: &WidgetContext) -> Option<QRect> {
        let max = constraints.max_size();
        if max[0] > 0.0 && max[1] > 0.0 {
            Some(QRect::new([0.0, 0.0], max))
        } else {
            None
        }
    }

    fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &AtlasRegion,
        _boundary_size: [f32; 2],
        _offset: [f32; 2],
        ctx: &WidgetContext,
    ) {
        let target_format = target.format();
        let renderer = ctx
            .any_resource()
            .get_or_insert_default::<RendererViewportClear>();

        let mut render_pass = match target.begin_render_pass(encoder) {
            Ok(rp) => rp,
            Err(_) => return,
        };

        renderer.render(
            &mut render_pass,
            target_format,
            &ctx.device(),
            self.color.to_rgba_f32(),
        );
    }
}
