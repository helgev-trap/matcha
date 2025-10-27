use crate::style::Style;
use gpu_utils::texture_atlas::atlas_simple::atlas::AtlasRegion;
use matcha_core::metrics::QSize;
use matcha_core::{color::Color, context::WidgetContext};
use parking_lot::Mutex;

pub use glyphon::cosmic_text::Stretch as TextStretch;
pub use glyphon::cosmic_text::Style as TextStyle;
pub use glyphon::cosmic_text::Weight as TextWeight;
/// Same as `cosmic_text::Family` but without lifetime parameter.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum TextFamily {
    Name(String),
    Serif,
    SansSerif,
    Cursive,
    Fantasy,
    Monospace,
}

impl From<glyphon::cosmic_text::Family<'_>> for TextFamily {
    fn from(family: glyphon::cosmic_text::Family) -> Self {
        match family {
            glyphon::cosmic_text::Family::Name(name) => TextFamily::Name(name.to_string()),
            glyphon::cosmic_text::Family::Serif => TextFamily::Serif,
            glyphon::cosmic_text::Family::SansSerif => TextFamily::SansSerif,
            glyphon::cosmic_text::Family::Cursive => TextFamily::Cursive,
            glyphon::cosmic_text::Family::Fantasy => TextFamily::Fantasy,
            glyphon::cosmic_text::Family::Monospace => TextFamily::Monospace,
        }
    }
}

impl<'a> From<&'a TextFamily> for glyphon::cosmic_text::Family<'a> {
    fn from(family: &'a TextFamily) -> Self {
        match family {
            TextFamily::Name(name) => glyphon::cosmic_text::Family::Name(name.as_str()),
            TextFamily::Serif => glyphon::cosmic_text::Family::Serif,
            TextFamily::SansSerif => glyphon::cosmic_text::Family::SansSerif,
            TextFamily::Cursive => glyphon::cosmic_text::Family::Cursive,
            TextFamily::Fantasy => glyphon::cosmic_text::Family::Fantasy,
            TextFamily::Monospace => glyphon::cosmic_text::Family::Monospace,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sentence {
    pub text: String,
    pub color: Color,
    pub family: TextFamily,
    pub stretch: TextStretch,
    pub style: TextStyle,
    pub weight: TextWeight,
}

impl Default for Sentence {
    fn default() -> Self {
        Self {
            text: String::new(),
            color: Color::rgb(0, 0, 0),
            family: TextFamily::SansSerif,
            stretch: glyphon::cosmic_text::Stretch::Normal,
            style: glyphon::cosmic_text::Style::Normal,
            weight: glyphon::cosmic_text::Weight::NORMAL,
        }
    }
}

impl Sentence {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            ..Default::default()
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn family(mut self, family: TextFamily) -> Self {
        self.family = family;
        self
    }

    pub fn stretch(mut self, stretch: TextStretch) -> Self {
        self.stretch = stretch;
        self
    }

    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = style;
        self
    }

    pub fn weight(mut self, weight: TextWeight) -> Self {
        self.weight = weight;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextDesc {
    pub texts: Vec<Sentence>,
    pub font_size: f32,
    pub line_height: f32,
}

impl TextDesc {
    pub fn new(texts: Vec<Sentence>) -> Self {
        Self {
            texts,
            font_size: 14.0,
            line_height: 20.0,
        }
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn line_height(mut self, height: f32) -> Self {
        self.line_height = height;
        self
    }

    pub fn add_element(mut self, text: Sentence) -> Self {
        self.texts.push(text);
        self
    }

    pub fn push_element(&mut self, text: Sentence) {
        self.texts.push(text);
    }
}

/// **To prevent deadlock, must lock in the order of font_system -> swash_cache -> cache -> text_atlas.**
struct TextShared {
    font_system: Mutex<glyphon::FontSystem>,
    swash_cache: Mutex<glyphon::SwashCache>,
    cache: Mutex<glyphon::Cache>,
    text_atlas: Mutex<glyphon::TextAtlas>,
}

impl TextShared {
    fn setup(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let font_system = glyphon::FontSystem::new();
        let swash_cache = glyphon::SwashCache::new();
        let cache = glyphon::Cache::new(device);
        let text_atlas =
            glyphon::TextAtlas::new(device, queue, &cache, wgpu::TextureFormat::Rgba8UnormSrgb);

        Self {
            font_system: Mutex::new(font_system),
            swash_cache: Mutex::new(swash_cache),
            cache: Mutex::new(cache),
            text_atlas: Mutex::new(text_atlas),
        }
    }
}

pub struct Text {
    // text info
    pub texts: Vec<Sentence>,
    pub font_size: f32,
    pub line_height: f32,

    // rendering context (needs `wgpu::Device` or `GlyphonShared` so cannot be created in `new()`)
    buffer: utils::cache::RwCache<QSize, glyphon::Buffer>,
    text_area_size: utils::cache::RwCache<QSize, [f32; 2]>,
    viewport: utils::cache::RwCache<QSize, glyphon::Viewport>,
    text_renderer: utils::cache::RwCache<QSize, glyphon::TextRenderer>,
}

impl Text {
    pub fn new(desc: &TextDesc) -> Self {
        Self {
            texts: desc.texts.clone(),
            font_size: desc.font_size,
            line_height: desc.line_height,
            buffer: utils::cache::RwCache::new(),
            text_area_size: utils::cache::RwCache::new(),
            viewport: utils::cache::RwCache::new(),
            text_renderer: utils::cache::RwCache::new(),
        }
    }

    pub fn eq_desc(&self, desc: &TextDesc) -> bool {
        self.texts == desc.texts
            && (self.font_size - desc.font_size).abs() < f32::EPSILON
            && (self.line_height - desc.line_height).abs() < f32::EPSILON
    }
}

impl Style for Text {
    fn required_region(
        &self,
        constraints: &matcha_core::metrics::Constraints,
        ctx: &WidgetContext,
    ) -> Option<matcha_core::metrics::QRect> {
        let q_size = QSize::from(constraints.max_size());

        let (_, buffer) = &*self.buffer.get_or_insert_with(&q_size, || {
            let size = constraints.max_size();

            let glyphon_shared = ctx
                .any_resource()
                .get_or_insert_with(|| TextShared::setup(&ctx.device(), &ctx.queue()));

            let mut font_system = glyphon_shared.font_system.lock();

            let mut buffer = glyphon::Buffer::new(
                &mut font_system,
                glyphon::Metrics::new(self.font_size, self.line_height),
            );
            buffer.set_size(&mut font_system, Some(size[0]), Some(size[1]));

            buffer.set_rich_text(
                &mut font_system,
                self.texts.iter().map(|e| {
                    (
                        e.text.as_str(),
                        glyphon::Attrs {
                            family: (&e.family).into(),
                            stretch: e.stretch,
                            style: e.style,
                            weight: e.weight,
                            color_opt: Some({
                                let c = e.color.to_rgba_u8();
                                glyphon::Color::rgba(c[0], c[1], c[2], c[3])
                            }),
                            // defaults
                            metadata: 0,
                            cache_key_flags: glyphon::cosmic_text::CacheKeyFlags::empty(),
                            metrics_opt: None,
                        },
                    )
                }),
                glyphon::Attrs::new(),
                glyphon::cosmic_text::Shaping::Advanced,
            );

            buffer.shape_until_scroll(&mut font_system, false);

            buffer
        });

        let (_, text_area_size) = &*self.text_area_size.get_or_insert_with(&q_size, || {
            let (w, h) = get_shaped_buffer_size(buffer);
            [w, h]
        });

        Some(matcha_core::metrics::QRect::new(
            [0.0, 0.0],
            [text_area_size[0], text_area_size[1]],
        ))
    }

    fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &AtlasRegion,
        boundary_size: [f32; 2],
        offset: [f32; 2],
        ctx: &WidgetContext,
    ) {
        // Reuse shaped buffer and renderer where possible. Observe lock order:
        // font_system -> swash_cache -> cache -> text_atlas
        let size = boundary_size;
        let q_size = QSize::from(size);

        let glyphon_shared = ctx
            .any_resource()
            .get_or_insert_with(|| TextShared::setup(&ctx.device(), &ctx.queue()));

        // 1) Acquire locks in required order
        let mut font_system = glyphon_shared.font_system.lock();
        let mut swash_cache = glyphon_shared.swash_cache.lock();
        let cache = glyphon_shared.cache.lock();
        let mut text_atlas = glyphon_shared.text_atlas.lock();

        // 2) Obtain or create the buffer (mutable)
        let (_, buffer) = &mut *self.buffer.get_or_insert_with(&q_size, || {
            let mut b = glyphon::Buffer::new(
                &mut font_system,
                glyphon::Metrics::new(self.font_size, self.line_height),
            );
            b.set_size(&mut font_system, Some(size[0]), Some(size[1]));
            b
        });

        // Ensure buffer size and content reflect the current boundary
        buffer.set_size(&mut font_system, Some(size[0]), Some(size[1]));
        buffer.set_rich_text(
            &mut font_system,
            self.texts.iter().map(|e| {
                (
                    e.text.as_str(),
                    glyphon::Attrs {
                        family: (&e.family).into(),
                        stretch: e.stretch,
                        style: e.style,
                        weight: e.weight,
                        color_opt: Some({
                            let c = e.color.to_rgba_u8();
                            glyphon::Color::rgba(c[0], c[1], c[2], c[3])
                        }),
                        // defaults
                        metadata: 0,
                        cache_key_flags: glyphon::cosmic_text::CacheKeyFlags::empty(),
                        metrics_opt: None,
                    },
                )
            }),
            glyphon::Attrs::new(),
            glyphon::cosmic_text::Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut font_system, false);

        // 3) Prepare viewport and text_renderer, caching them in RwOption to avoid recreation
        let target_size = target.size();
        // viewport resolution should match the render target (region) size so shader NDC math maps correctly
        let (_, viewport) = &mut *self
            .viewport
            .get_or_insert_with(&q_size, || glyphon::Viewport::new(&ctx.device(), &cache));
        viewport.update(
            &ctx.queue(),
            glyphon::Resolution {
                width: target_size[0],
                height: target_size[1],
            },
        );

        let (_, text_renderer) = &mut *self.text_renderer.get_or_insert_with(&q_size, || {
            glyphon::TextRenderer::new(
                &mut text_atlas,
                &ctx.device(),
                wgpu::MultisampleState::default(),
                None,
            )
        });

        // 4) Build TextArea mapped into the target region.
        // Use offset as the top-left position within the target region.
        let text_area = glyphon::TextArea {
            buffer,
            left: offset[0],
            top: offset[1],
            scale: 1.0,
            bounds: glyphon::TextBounds {
                left: 0,
                top: 0,
                right: target_size[0] as i32,
                bottom: target_size[1] as i32,
            },
            default_color: glyphon::Color::rgba(128, 128, 128, 255),
            custom_glyphs: &[],
        };

        // 5) Call prepare to ensure glyphs are rasterized into glyphon's atlas and vertex buffer is populated.
        if text_renderer
            .prepare(
                &ctx.device(),
                &ctx.queue(),
                &mut font_system,
                &mut text_atlas,
                viewport,
                [text_area],
                &mut swash_cache,
            )
            .is_err()
        {
            // On failure (e.g., atlas full) bail out gracefully.
            return;
        }

        // 6) Begin a render pass targeting the atlas region and render glyphon content into it.
        let mut render_pass = match target.begin_render_pass(encoder) {
            Ok(rp) => rp,
            Err(_) => return,
        };

        if text_renderer
            .render(&text_atlas, viewport, &mut render_pass)
            .is_err()
        {
            // rendering failed, abort
            return;
        }

        // 7) Trim atlas usage flags so glyphon can evict unused glyphs later.
        text_atlas.trim();
    }
}

fn get_shaped_buffer_size(buffer: &glyphon::Buffer) -> (f32, f32) {
    let mut max_width = 0.0f32;
    let mut lines = 0usize;

    // バッファ内のすべての行をループ
    for line in buffer.lines.iter() {
        // この行がシェイピング（レイアウト計算）されているか確認
        if let Some(layout) = line.layout_opt() {
            for layout_line in layout.iter() {
                // 各行の幅を取得して最大幅を更新
                max_width = max_width.max(layout_line.w);
                lines += 1;
            }
        }
    }

    (max_width, buffer.metrics().line_height * (lines as f32))
}
