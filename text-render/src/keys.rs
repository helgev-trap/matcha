#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey<const N: u32> {
    char: char,
    // store size multiplied by N to make it able to derive Eq and Hash
    multiplied_font_size: u32,
    font_hash: usize,
}

impl<const N: u32> GlyphKey<N> {
    pub const fn new(char: char, font_size: f32, font_hash: usize) -> Self {
        let multiplied_font_size = ((font_size * N as f32) + 0.5) as u32;

        GlyphKey {
            char,
            multiplied_font_size,
            font_hash,
        }
    }

    pub const fn get_font_size(&self) -> f32 {
        self.multiplied_font_size as f32 / N as f32
    }

    pub const fn get_char(&self) -> char {
        self.char
    }

    pub const fn get_font_hash(&self) -> usize {
        self.font_hash
    }
}

impl<const N: u32> std::fmt::Debug for GlyphKey<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlyphKey")
            .field("char", &self.char)
            .field("size", &self.get_font_size())
            .field("font_hash", &self.font_hash)
            .finish()
    }
}
