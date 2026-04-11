/// Represents the background onto which a widget is rendered.
#[derive(Clone, Copy)]
pub struct Background<'a> {
    view: &'a wgpu::TextureView,
    position: [f32; 2],
}

impl<'a> Background<'a> {
    /// Creates a new `Background`.
    pub fn new(view: &'a wgpu::TextureView, position: [f32; 2]) -> Self {
        Self { view, position }
    }

    /// Returns the texture view of the background.
    pub fn view(&self) -> &wgpu::TextureView {
        self.view
    }

    /// Returns the current top-left position of the background.
    pub fn position(&self) -> [f32; 2] {
        self.position
    }

    /// Translates the background by a given position, returning a new `Background`.
    pub fn translate(mut self, position: [f32; 2]) -> Self {
        self.position = [
            self.position[0] + position[0],
            self.position[1] + position[1],
        ];
        self
    }
}
