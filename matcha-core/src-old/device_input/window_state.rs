use super::DeviceInputData;

pub struct WindowState {
    inner_size: [f32; 2],
    outer_size: [f32; 2],
    inner_position: [f32; 2],
    outer_position: [f32; 2],
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            inner_size: [1.0, 1.0],
            outer_size: [1.0, 1.0],
            inner_position: [1.0, 1.0],
            outer_position: [1.0, 1.0],
        }
    }
}

impl WindowState {
    pub fn new(
        inner_size: [f32; 2],
        outer_size: [f32; 2],
        inner_position: [f32; 2],
        outer_position: [f32; 2],
    ) -> Self {
        Self {
            inner_size,
            outer_size,
            inner_position,
            outer_position,
        }
    }

    pub fn resized(&mut self, inner_size: [f32; 2], outer_size: [f32; 2]) -> DeviceInputData {
        self.inner_size = inner_size;
        self.outer_size = outer_size;

        DeviceInputData::WindowPositionSize {
            inner_position: self.inner_position,
            outer_position: self.outer_position,
            inner_size: self.inner_size,
            outer_size: self.outer_size,
        }
    }

    pub fn moved(&mut self, inner_position: [f32; 2], outer_position: [f32; 2]) -> DeviceInputData {
        self.inner_position = inner_position;
        self.outer_position = outer_position;

        DeviceInputData::WindowPositionSize {
            inner_position: self.inner_position,
            outer_position: self.outer_position,
            inner_size: self.inner_size,
            outer_size: self.outer_size,
        }
    }
}
