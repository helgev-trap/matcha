#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TextRenderConfig<'a> {
    pub font: fontdb::Query<'a>,
    pub font_size: f32,
    pub kerning: Kerning,
    pub line_height: LineHeight,
    pub line_length: f32,
    pub horizontal_layout: TextLayout,
}

pub struct TextRasterizeConfig<'a> {
    pub font: fontdb::Query<'a>,
    pub font_size: f32,
    pub color: [f32; 4],
    pub kerning: Kerning,
    pub line_height: LineHeight,
    pub line_length: f32,
    pub bitmap_size: [usize; 2],
    pub vertical_layout: TextLayout,
    pub baseline_standardized: bool,
    pub horizontal_layout: TextLayout,
    pub transform: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kerning {
    Kern(f32),
    Monospace(f32),
}

impl Default for Kerning {
    fn default() -> Self {
        Kerning::Kern(0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LineHeight {
    Fixed(f32),
    Relative(f32),
}

impl Default for LineHeight {
    fn default() -> Self {
        LineHeight::Relative(0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextLayout {
    Start(f32),
    Center(f32),
    End(f32),
}

impl Default for TextLayout {
    fn default() -> Self {
        TextLayout::Start(0.0)
    }
}
