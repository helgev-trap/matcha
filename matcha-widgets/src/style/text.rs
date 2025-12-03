use std::num::NonZeroUsize;

use gpu_utils::texture_atlas::atlas_simple::atlas::AtlasRegion;
use matcha_core::metrics::{Constraints, QSize};
use matcha_core::{color::Color, context::WidgetContext};
use parking_lot::Mutex;

use utils::cache::RwCache;
use utils::rwoption::RwOption;

pub use wgfont::fontdb::{Family, Stretch, Style, Weight};

pub struct TextSpan<'a> {
    pub text: &'a str,
    pub size: f32,
    pub families: &'a [wgfont::fontdb::Family<'a>],
    pub weight: wgfont::fontdb::Weight,
    pub stretch: wgfont::fontdb::Stretch,
    pub style: wgfont::fontdb::Style,
    pub color: Color,
}

/// Internal use.
#[derive(Clone, PartialEq)]
enum OwnedFamily {
    Name(String),
    Serif,
    SansSerif,
    Cursive,
    Fantasy,
    Monospace,
}

/// Internal use.
#[derive(Clone, PartialEq)]
struct OwnedTextSpan {
    text: String,
    size: f32,
    families: Vec<OwnedFamily>,
    weight: wgfont::fontdb::Weight,
    stretch: wgfont::fontdb::Stretch,
    style: wgfont::fontdb::Style,
    color: matcha_core::color::Color,
}

impl From<TextSpan<'_>> for OwnedTextSpan {
    fn from(span: TextSpan<'_>) -> Self {
        Self {
            text: span.text.to_string(),
            size: span.size,
            families: span
                .families
                .iter()
                .map(|f| match f {
                    wgfont::fontdb::Family::Name(name) => OwnedFamily::Name(name.to_string()),
                    wgfont::fontdb::Family::Serif => OwnedFamily::Serif,
                    wgfont::fontdb::Family::SansSerif => OwnedFamily::SansSerif,
                    wgfont::fontdb::Family::Cursive => OwnedFamily::Cursive,
                    wgfont::fontdb::Family::Fantasy => OwnedFamily::Fantasy,
                    wgfont::fontdb::Family::Monospace => OwnedFamily::Monospace,
                })
                .collect(),
            weight: span.weight,
            stretch: span.stretch,
            style: span.style,
            color: span.color,
        }
    }
}

impl OwnedTextSpan {
    fn eq(&self, other: &TextSpan<'_>) -> bool {
        todo!()
    }
}

/// **To prevent deadlock, must lock in the order of font_system -> swash_cache -> cache -> text_atlas.**
struct TextShared {
    font_storage: Mutex<wgfont::font_storage::FontStorage>,
    cpu_renderer: Mutex<wgfont::renderer::CpuRenderer>,
}

impl TextShared {
    fn setup(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let _ = (device, queue);

        let mut font_storage = wgfont::font_storage::FontStorage::new();
        font_storage.load_system_fonts();

        #[allow(clippy::unwrap_used)]
        let cache = wgfont::renderer::cpu_renderer::GlyphCache::new(&[
            (
                NonZeroUsize::new(1024).unwrap(),
                NonZeroUsize::new(512).unwrap(),
            ),
            (
                NonZeroUsize::new(4096).unwrap(),
                NonZeroUsize::new(128).unwrap(),
            ),
            (
                NonZeroUsize::new(16_384).unwrap(),
                NonZeroUsize::new(64).unwrap(),
            ),
        ]);

        let cpu_renderer = wgfont::renderer::CpuRenderer::new(cache);

        Self {
            font_storage: Mutex::new(font_storage),
            cpu_renderer: Mutex::new(cpu_renderer),
        }
    }
}

pub struct TextRenderer {
    owned_texts: Vec<OwnedTextSpan>,
    text_data: RwOption<wgfont::text::TextData>,

    layout_cache: RwCache<Constraints, wgfont::text::TextLayout>,
    bitmap_cache: RwCache<QSize, wgfont::renderer::Bitmap>,

    texture_cache: RwCache<QSize, wgpu::Texture>,
}

impl PartialEq for TextRenderer {
    fn eq(&self, other: &Self) -> bool {
        self.owned_texts == other.owned_texts
    }
}

impl Clone for TextRenderer {
    fn clone(&self) -> Self {
        Self {
            owned_texts: self.owned_texts.clone(),
            text_data: RwOption::new(),
            layout_cache: RwCache::new(),
            bitmap_cache: RwCache::new(),
            texture_cache: RwCache::new(),
        }
    }
}

impl Default for TextRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl TextRenderer {
    pub fn new() -> Self {
        Self {
            owned_texts: Vec::new(),
            text_data: RwOption::new(),
            layout_cache: RwCache::new(),
            bitmap_cache: RwCache::new(),
            texture_cache: RwCache::new(),
        }
    }

    pub fn push_span(&mut self, span: TextSpan<'_>) {
        let owned_span = OwnedTextSpan::from(span);
        self.owned_texts.push(owned_span);
    }

    fn build_text_data(
        owned_texts: &[OwnedTextSpan],
        font_storage: &mut wgfont::font_storage::FontStorage,
    ) -> wgfont::text::TextData {
        let mut text_data = wgfont::text::TextData::new();

        for span in owned_texts {
            let OwnedTextSpan {
                text,
                size,
                families,
                weight,
                stretch,
                style,
                color: _,
            } = span;

            let family = families
                .iter()
                .map(|f| match f {
                    OwnedFamily::Name(name) => wgfont::fontdb::Family::Name(name.as_str()),
                    OwnedFamily::Serif => wgfont::fontdb::Family::Serif,
                    OwnedFamily::SansSerif => wgfont::fontdb::Family::SansSerif,
                    OwnedFamily::Cursive => wgfont::fontdb::Family::Cursive,
                    OwnedFamily::Fantasy => wgfont::fontdb::Family::Fantasy,
                    OwnedFamily::Monospace => wgfont::fontdb::Family::Monospace,
                })
                .collect::<smallvec::SmallVec<[_; 10]>>();

            let Some((font_id, _)) = font_storage.query(&wgfont::fontdb::Query {
                families: &family,
                weight: *weight,
                stretch: *stretch,
                style: *style,
            }) else {
                todo!();
            };

            text_data.append(wgfont::text::TextElement {
                font_id,
                font_size: *size,
                content: text.clone(),
            });
        }

        text_data
    }

    fn build_layout(
        constraints: &Constraints,
        text_data: &wgfont::text::TextData,
        font_storage: &mut wgfont::font_storage::FontStorage,
    ) -> wgfont::text::TextLayout {
        let max_size = constraints.max_size();

        let layout_config = wgfont::text::TextLayoutConfig {
            max_width: Some(max_size[0]),
            max_height: Some(max_size[1]),
            horizontal_align: wgfont::text::HorizontalAlign::Left,
            vertical_align: wgfont::text::VerticalAlign::Top,
            line_height_scale: 1.0,
            wrap_style: wgfont::text::WrapStyle::WordWrap,
            wrap_hard_break: false,
            word_separators: " ".chars().collect(),
            linebreak_char: ['\n'].into_iter().collect(),
        };

        text_data.layout(&layout_config, font_storage)
    }

    fn build_bitmap(
        text_layout: &wgfont::text::TextLayout,
        size: [f32; 2],
        renderer: &mut wgfont::renderer::CpuRenderer,
        font_storage: &mut wgfont::font_storage::FontStorage,
    ) -> wgfont::renderer::Bitmap {
        renderer.render(
            text_layout,
            [size[0].ceil() as usize, size[1].ceil() as usize],
            font_storage,
        )
    }

    fn build_texture(bitmap: &wgfont::renderer::Bitmap, ctx: &WidgetContext) -> wgpu::Texture {
        let texture = ctx.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("text_texture_cache"),
            size: wgpu::Extent3d {
                width: bitmap.width as u32,
                height: bitmap.height as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let queue = ctx.queue();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            &bitmap.pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bitmap.width as u32),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: bitmap.width as u32,
                height: bitmap.height as u32,
                depth_or_array_layers: 1,
            },
        );

        texture
    }
}

impl crate::style::Style for TextRenderer {
    fn required_region(
        &self,
        constraints: &matcha_core::metrics::Constraints,
        ctx: &WidgetContext,
    ) -> Option<matcha_core::metrics::QRect> {
        let shared = ctx
            .any_resource()
            .get_or_insert_with(|| TextShared::setup(&ctx.device(), &ctx.queue()));

        let TextShared {
            font_storage,
            cpu_renderer: _,
        } = &*shared;

        let text_data = self.text_data.get_or_insert_with(|| {
            let mut font_storage = font_storage.lock();
            Self::build_text_data(&self.owned_texts, &mut font_storage)
        });

        let text_layout = self.layout_cache.get_or_insert_with(constraints, || {
            let mut font_storage = shared.font_storage.lock();
            Self::build_layout(constraints, &text_data, &mut font_storage)
        });
        let (_, layout) = &*text_layout;

        let size = [layout.total_width, layout.total_height];

        // to avoid quantization and rounding errors
        // todo: find a better way
        let size = [
            (size[0] + 0.5).min(constraints.max_width()),
            (size[1] + 0.5).min(constraints.max_height()),
        ];

        Some(matcha_core::metrics::QRect::new([0.0, 0.0], size))
    }

    fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &AtlasRegion,
        boundary_size: [f32; 2],
        offset: [f32; 2],
        ctx: &WidgetContext,
    ) {
        // to avoid rounding errors
        let boundary_size = [boundary_size[0] + 0.5, boundary_size[1] + 0.5];

        // Reuse shaped buffer and renderer where possible. Observe lock order:
        // font_system -> swash_cache -> cache -> text_atlas
        let q_size = QSize::from(boundary_size);
        let size = q_size.size();

        let constraints = Constraints::from_max_q_size(q_size);

        let shared = ctx
            .any_resource()
            .get_or_insert_with(|| TextShared::setup(&ctx.device(), &ctx.queue()));

        let TextShared {
            font_storage,
            cpu_renderer: _,
        } = &*shared;

        let text_data = self.text_data.get_or_insert_with(|| {
            let mut font_storage = font_storage.lock();
            Self::build_text_data(&self.owned_texts, &mut font_storage)
        });

        let text_layout = self.layout_cache.get_or_insert_with(&constraints, || {
            let mut font_storage = shared.font_storage.lock();
            Self::build_layout(&constraints, &text_data, &mut font_storage)
        });
        let (_, text_layout) = &*text_layout;

        let bitmap = self.bitmap_cache.get_or_insert_with(&q_size, || {
            let mut font_storage = shared.font_storage.lock();
            let mut renderer = shared.cpu_renderer.lock();

            Self::build_bitmap(text_layout, size, &mut renderer, &mut font_storage)
        });
        let (_, bitmap) = &*bitmap;

        let texture = self
            .texture_cache
            .get_or_insert_with(&q_size, || Self::build_texture(bitmap, ctx));

        let (_, texture) = &*texture;

        let texture_copy = ctx
            .any_resource()
            .get_or_insert_default::<renderer::texture_copy::TextureCopy>();

        let Ok(mut render_pass) = target.begin_render_pass(encoder) else {
            todo!()
        };

        texture_copy.render(
            &mut render_pass,
            renderer::texture_copy::TargetData {
                target_size: target.texture_size(),
                target_format: target.format(),
            },
            renderer::texture_copy::RenderData {
                source_texture_view: &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                source_texture_position_min: offset,
                source_texture_position_max: [
                    offset[0] + texture.size().width as f32,
                    offset[1] + texture.size().height as f32,
                ],
                color_transformation: Some(COLOR_TRANSFORMATION),
                color_offset: None,
            },
            &ctx.device(),
        );
    }
}

#[rustfmt::skip]
const COLOR_TRANSFORMATION: nalgebra::Matrix4<f32> = nalgebra::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    1.0, 0.0, 0.0, 0.0,
    1.0, 0.0, 0.0, 0.0,
    1.0, 0.0, 0.0, 1.0,
);
