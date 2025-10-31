use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::Arc,
};

use crate::{
    cache_atlas::CacheAtlas,
    error::TextError,
    keys::GlyphKey,
    text::{Kerning, LineHeight},
};

use super::TextRenderConfig;

// MARK: TextContext

pub struct TextContext<const N: u32 = 256> {
    // config
    cache_panel_size: u32,
    index_width: u32,
    index_height: u32,

    // font management
    font_database: fontdb::Database,
    fonts: HashMap<usize, fontdue::Font>,

    // glyph cache
    cache: Option<CacheAtlas<N>>,
    // gpu rendering state
    // todo
}

// MARK: impl

impl TextContext {
    pub fn new(panel_size: u32, index_width: u32, index_height: u32) -> Self {
        let mut font_database = fontdb::Database::new();
        font_database.load_system_fonts();

        TextContext {
            cache_panel_size: panel_size,
            index_width,
            index_height,
            font_database,
            fonts: HashMap::new(),
            cache: None,
        }
    }
}

impl<const N: u32> TextContext<N> {
    pub fn set_cache(&mut self, device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) {
        self.cache = Some(CacheAtlas::<N>::new(
            device,
            queue,
            self.cache_panel_size,
            self.index_width,
            self.index_height,
        ));
    }

    pub fn use_font(&mut self, query: fontdb::Query) {
        // load font if not loaded
        let font_hash = utils::query_hash(&query);

        self.fonts
            .entry(font_hash)
            .or_insert_with(|| utils::load_font(&mut self.font_database, &query));
    }
}

// MARK: layout

pub struct GlyphLayout<const N: u32 = 256> {
    pub bounds: [f32; 2],
    pub glyphs: HashMap<GlyphKey<N>, Vec<GlyphPosition>>,
}

impl<const N: u32> TextContext<N> {
    pub fn layout(
        &mut self,
        text: &str,
        config: TextRenderConfig,
    ) -> Result<GlyphLayout<N>, TextError> {
        // load font if not loaded
        let font_hash = utils::query_hash(&config.font);

        let font = self
            .fonts
            .entry(font_hash)
            .or_insert_with(|| utils::load_font(&mut self.font_database, &config.font));

        // prepare for rendering

        let line_metrics = font.horizontal_line_metrics(config.font_size).unwrap();

        let new_line_size = match config.line_height {
            LineHeight::Fixed(height) => height,
            LineHeight::Relative(ratio) => line_metrics.new_line_size + ratio,
        };

        // render

        // step 1
        let mut line_buffer = Vec::new(); // glyph position base on baseline
        // step 2
        let mut text_buffer = Vec::new(); // (line_width, line_buffer)
        // step 3
        let mut glyph_layouts = HashMap::new(); // position of each glyph

        let mut max_width = 0.0f32;

        // layout glyphs

        {
            let mut previous_char = None;
            let mut line_width = 0.0f32;
            let mut accumulated_width = 0.0f32;

            // make text buffer

            for c in text.chars() {
                // skip control characters
                if c.is_control() {
                    // todo
                    continue;
                }

                // apply kerning
                if let (Kerning::Kern(_), Some(prev)) = (config.kerning, previous_char) {
                    accumulated_width += font
                        .horizontal_kern(prev, c, config.font_size)
                        .unwrap_or(0.0);
                }

                // get glyph metrics
                let metrics = font.metrics(c, config.font_size);

                // check overflow and line break
                if accumulated_width + metrics.bounds.xmin + metrics.bounds.width
                    > config.line_length
                    || c == '\n'
                {
                    // line break

                    // store line buffer
                    let mut swap_line_buffer = Vec::new();
                    std::mem::swap(&mut line_buffer, &mut swap_line_buffer);

                    text_buffer.push((line_width, swap_line_buffer));
                    accumulated_width = 0.0f32;
                    // previous_char = None;
                }

                let glyph_position = GlyphPosition::from_metrics(metrics, [accumulated_width, 0.0]);
                line_buffer.push((c, glyph_position));
                line_width = accumulated_width + metrics.bounds.xmin + metrics.bounds.width;

                // update accumulated width
                accumulated_width += match config.kerning {
                    Kerning::Kern(kern_fix) => metrics.advance_width + kern_fix,
                    Kerning::Monospace(space) => space,
                };

                // update previous char
                previous_char = Some(c);
            }

            // store last line buffer
            if !line_buffer.is_empty() {
                text_buffer.push((line_width, line_buffer));
                max_width = max_width.max(line_width);
            }
        }

        // data

        let lines = text_buffer.len() as f32;
        let all_lines_height =
            new_line_size * (lines - 1.0f32) + line_metrics.ascent + line_metrics.descent;
        let max_line_width = max_width;

        let text_bounds = [max_line_width, all_lines_height];

        // store glyphs to hashmap

        {
            let mut vertical_offset = line_metrics.ascent;
            for (line_width, line_buffer) in text_buffer {
                let horizontal_offset = match config.horizontal_layout {
                    crate::text::TextLayout::Start(_) => 0.0f32,
                    crate::text::TextLayout::Center(_) => (max_line_width - line_width) / 2.0f32,
                    crate::text::TextLayout::End(_) => max_line_width - line_width,
                };

                for (c, position) in line_buffer.iter() {
                    let glyph_position = position.transform([horizontal_offset, vertical_offset]);
                    let key = GlyphKey::<N>::new(*c, config.font_size, font_hash);

                    // store glyph position
                    let glyph_positions = glyph_layouts.entry(key).or_insert_with(Vec::new);
                    glyph_positions.push(glyph_position);
                }

                // update vertical offset
                vertical_offset += new_line_size;
            }
        }

        Ok(GlyphLayout {
            bounds: text_bounds,
            glyphs: glyph_layouts,
        })
    }
}

// MARK: render
impl<const N: u32> TextContext<N> {
    pub fn render_wgpu(
        &mut self,
        layout: HashMap<GlyphKey<N>, Vec<GlyphPosition>>,
        texture: &wgpu::Texture,
        transform: &nalgebra::Matrix4<f32>,
    ) -> Result<(), TextError> {
        // check if cache is set
        let Some(cache) = self.cache.as_mut() else {
            return Err(TextError::CacheNotSet);
        };

        let mut meshes_buffer = Vec::new();
        for (key, positions) in layout {
            // get glyph cache
            if let Some(glyph_cache) = cache.get(key) {
                // cache hit
                // add glyph mesh to the buffer
                for position in positions {
                    let mesh = utils::GlyphMesh::new(&position, glyph_cache);
                    meshes_buffer.push(mesh);
                }
            } else {
                //check if cache have extra space
                if !cache.is_full() {
                    // rasterize new glyph and store
                    let font = self
                        .fonts
                        .get(&key.get_font_hash())
                        .expect("Font not found in the font database.");

                    let (metrics, bitmap) =
                        font.rasterize(key.get_char(), self.cache_panel_size as f32);

                    cache.store_data(key, metrics, &bitmap);

                    // add glyph mesh to the buffer
                    let glyph_cache = cache.get(key).unwrap();
                    for position in positions {
                        let mesh = utils::GlyphMesh::new(&position, glyph_cache);
                        meshes_buffer.push(mesh);
                    }
                } else {
                    // render glyphs in the buffer
                    // make integrated mesh
                    let mut swap_meshes_buffer = Vec::new();
                    std::mem::swap(&mut meshes_buffer, &mut swap_meshes_buffer);
                    let (vertices, indices) = utils::mesh_integral(swap_meshes_buffer);

                    // render info
                    let target_width = texture.width() as f32;
                    let target_height = texture.height() as f32;
                    let cache_texture = cache.get_texture();

                    todo!()
                }
            }
        }

        Ok(())
    }
}

// MARK: glyph position

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphPosition {
    pub upper_left: [f32; 2],
    pub size: [f32; 2],
}

impl GlyphPosition {
    pub const fn from_metrics(metrics: fontdue::Metrics, offset: [f32; 2]) -> Self {
        // construct global position from metrics, use offset as the origin of metrics.
        // note that the direction of y-axis is inverted in the font metrics.
        let upper_left = [
            offset[0] + metrics.bounds.xmin,
            offset[1] - metrics.bounds.ymin - metrics.bounds.height,
        ];

        let size = [metrics.bounds.width, metrics.bounds.height];

        GlyphPosition { upper_left, size }
    }

    pub const fn transform(&self, transform: [f32; 2]) -> Self {
        GlyphPosition {
            upper_left: [
                self.upper_left[0] + transform[0],
                self.upper_left[1] + transform[1],
            ],
            size: self.size,
        }
    }
}

// MARK: utils

mod utils {
    use crate::cache_atlas::GlyphCache;

    use super::*;

    pub fn load_font(database: &mut fontdb::Database, query: &fontdb::Query) -> fontdue::Font {
        let face_id = database.query(query).unwrap();
        let face_info = database.face(face_id).unwrap();

        // load binary

        let mut vec_anchor: Vec<u8> = Vec::new();
        let binary_slice = match &face_info.source {
            fontdb::Source::Binary(binary) => binary.as_ref().as_ref(),
            fontdb::Source::File(path_buf) => {
                let path = path_buf.as_path();
                let mut binary = std::fs::read(path).unwrap();
                std::mem::swap(&mut vec_anchor, &mut binary);
                vec_anchor.as_slice()
            }
            fontdb::Source::SharedFile(_, as_ref) => as_ref.as_ref().as_ref(),
        };

        // load font

        let font_settings = fontdue::FontSettings {
            collection_index: face_info.index,
            scale: 1.0,
            load_substitutions: true,
        };

        fontdue::Font::from_bytes(binary_slice, font_settings).unwrap()
    }

    pub fn query_hash(query: &fontdb::Query) -> usize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        query.hash(&mut hasher);
        hasher.finish() as usize
    }

    pub struct GlyphMesh {
        pub uv_offset: [f32; 2],
        pub uv_size: [f32; 2],
        pub vertex_offset: [f32; 2],
        pub vertex_size: [f32; 2],
    }

    impl GlyphMesh {
        pub const fn new(position: &GlyphPosition, atlas: &GlyphCache) -> Self {
            GlyphMesh {
                vertex_offset: position.upper_left,
                vertex_size: position.size,
                uv_offset: atlas.bound_offset,
                uv_size: atlas.bound_size,
            }
        }
    }

    pub fn mesh_integral(meshes: Vec<GlyphMesh>) -> (Vec<GlyphVertex>, Vec<u32>) {
        // polygon division
        // 0----3
        // | \  |
        // |  \ |
        // 1----2

        // indices
        let mut indices = Vec::with_capacity(meshes.len() * 6);
        for i in 0..meshes.len() as u32 {
            let offset = i * 4;
            indices.push(offset);
            indices.push(offset + 1);
            indices.push(offset + 2);

            indices.push(offset + 2);
            indices.push(offset + 3);
            indices.push(offset);
        }

        // vertices
        let vertices = meshes
            .into_iter()
            .flat_map(|mesh| {
                vec![
                    // 0
                    GlyphVertex {
                        pos: [mesh.vertex_offset[0], mesh.vertex_offset[1]],
                        uv: [mesh.uv_offset[0], mesh.uv_offset[1]],
                    },
                    // 1
                    GlyphVertex {
                        pos: [
                            mesh.vertex_offset[0] + mesh.vertex_size[0],
                            mesh.vertex_offset[1],
                        ],
                        uv: [mesh.uv_offset[0] + mesh.uv_size[0], mesh.uv_offset[1]],
                    },
                    // 2
                    GlyphVertex {
                        pos: [
                            mesh.vertex_offset[0] + mesh.vertex_size[0],
                            mesh.vertex_offset[1] + mesh.vertex_size[1],
                        ],
                        uv: [
                            mesh.uv_offset[0] + mesh.uv_size[0],
                            mesh.uv_offset[1] + mesh.uv_size[1],
                        ],
                    },
                    // 3
                    GlyphVertex {
                        pos: [
                            mesh.vertex_offset[0],
                            mesh.vertex_offset[1] + mesh.vertex_size[1],
                        ],
                        uv: [mesh.uv_offset[0], mesh.uv_offset[1] + mesh.uv_size[1]],
                    },
                ]
            })
            .collect::<Vec<_>>();

        (vertices, indices)
    }

    // MARK: wgpu structs

    #[repr(C)]
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct GlyphVertex {
        pub pos: [f32; 2],
        pub uv: [f32; 2],
    }
}
