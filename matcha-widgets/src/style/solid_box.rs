use crate::style::Style;
use gpu_utils::texture_atlas::atlas_simple::atlas::AtlasRegion;
use matcha_core::{
    color::Color,
    context::WidgetContext,
    metrics::{Constraints, QRect},
};
use renderer::{
    vertex::colored_vertex::ColorVertex,
    widgets_renderer::vertex_color::{RenderData, TargetData, VertexColor},
};

// todo: more documentation

// MARK: Style

pub struct SolidBox {
    pub color: Color,
}

impl Style for SolidBox {
    fn required_region(&self, constraints: &Constraints, _ctx: &WidgetContext) -> Option<QRect> {
        let max = constraints.max_size();
        if max[0] > 0.0 && max[1] > 0.0 {
            Some(QRect::new([0.0, 0.0], max))
        } else {
            None
        }
    }

    fn is_inside(&self, position: [f32; 2], boundary_size: [f32; 2], _ctx: &WidgetContext) -> bool {
        position[0] >= 0.0
            && position[0] <= boundary_size[0]
            && position[1] >= 0.0
            && position[1] <= boundary_size[1]
    }

    fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &AtlasRegion,
        boundary_size: [f32; 2],
        offset: [f32; 2],
        ctx: &WidgetContext,
    ) {
        let target_size = target.texture_size();
        let target_format = target.format();
        let renderer = ctx.any_resource().get_or_insert_default::<VertexColor>();

        // create a render pass targeting the atlas region so implementations can use multiple passes if needed
        let mut render_pass = match target.begin_render_pass(encoder) {
            Ok(rp) => rp,
            Err(_) => return,
        };

        let vertices = [
            ColorVertex {
                position: nalgebra::Point3::new(0.0, 0.0, 0.0),
                color: self.color.to_rgba_f32(),
            },
            ColorVertex {
                position: nalgebra::Point3::new(boundary_size[0], 0.0, 0.0),
                color: self.color.to_rgba_f32(),
            },
            ColorVertex {
                position: nalgebra::Point3::new(boundary_size[0], boundary_size[1], 0.0),
                color: self.color.to_rgba_f32(),
            },
            ColorVertex {
                position: nalgebra::Point3::new(0.0, boundary_size[1], 0.0),
                color: self.color.to_rgba_f32(),
            },
        ];

        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

        renderer.render(
            &mut render_pass,
            TargetData {
                target_size,
                target_format,
            },
            RenderData {
                vertices: &vertices,
                indices: &indices,
                transform: nalgebra::Matrix4::new_translation(&nalgebra::Vector3::new(
                    offset[0], offset[1], 0.0,
                )),
            },
            &ctx.device(),
        );
    }
}
