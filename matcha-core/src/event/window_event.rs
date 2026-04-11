// ----------------------------------------------------------------------------
// WindowEventState
// ----------------------------------------------------------------------------

/// Tracks the current state of a window (size, position, focus, etc.).
#[derive(Debug)]
pub struct WindowEventState {
    inner_size: [f32; 2],
    outer_size: [f32; 2],
    inner_position: [f32; 2],
    outer_position: [f32; 2],
}

impl Default for WindowEventState {
    fn default() -> Self {
        Self {
            inner_size: [1.0, 1.0],
            outer_size: [1.0, 1.0],
            inner_position: [1.0, 1.0],
            outer_position: [1.0, 1.0],
        }
    }
}

impl WindowEventState {
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

    /// Handle `winit::event::WindowEvent::Resized`.
    pub fn resized(&mut self, inner_size: [f32; 2], outer_size: [f32; 2]) -> WindowEvent {
        self.inner_size = inner_size;
        self.outer_size = outer_size;

        WindowEvent {
            data: WindowEventData::PositionSize {
                inner_position: self.inner_position,
                outer_position: self.outer_position,
                inner_size: self.inner_size,
                outer_size: self.outer_size,
            },
        }
    }

    /// Handle `winit::event::WindowEvent::Moved`.
    pub fn moved(&mut self, inner_position: [f32; 2], outer_position: [f32; 2]) -> WindowEvent {
        self.inner_position = inner_position;
        self.outer_position = outer_position;

        WindowEvent {
            data: WindowEventData::PositionSize {
                inner_position: self.inner_position,
                outer_position: self.outer_position,
                inner_size: self.inner_size,
                outer_size: self.outer_size,
            },
        }
    }

    /// Handle `winit::event::WindowEvent::CloseRequested`.
    pub fn close_requested(&self) -> WindowEvent {
        WindowEvent {
            data: WindowEventData::CloseRequested,
        }
    }

    /// Handle `winit::event::WindowEvent::Focused`.
    pub fn focused(&self, focused: bool) -> WindowEvent {
        WindowEvent {
            data: WindowEventData::Focus(focused),
        }
    }

    /// Handle `winit::event::WindowEvent::ThemeChanged`.
    pub fn theme_changed(&self, theme: winit::window::Theme) -> WindowEvent {
        WindowEvent {
            data: WindowEventData::Theme(theme),
        }
    }

    /// Handle `winit::event::WindowEvent::ScaleFactorChanged`.
    pub fn scale_factor_changed(&self, scale_factor: f64) -> WindowEvent {
        WindowEvent {
            data: WindowEventData::ScaleFactorChanged { scale_factor },
        }
    }

    /// Handle `winit::event::WindowEvent::Occluded`.
    pub fn occluded(&self, occluded: bool) -> WindowEvent {
        WindowEvent {
            data: WindowEventData::Occluded(occluded),
        }
    }
}

// ----------------------------------------------------------------------------
// WindowEvent
// ----------------------------------------------------------------------------

/// A high-level window lifecycle / state event.
///
/// These events are distinct from device input events (mouse, keyboard, etc.)
/// and describe changes to the window itself.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowEvent {
    data: WindowEventData,
}

impl WindowEvent {
    pub fn data(&self) -> &WindowEventData {
        &self.data
    }
}

// Convenience accessors
impl WindowEvent {
    pub fn on_close_requested<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce() -> R,
    {
        match &self.data {
            WindowEventData::CloseRequested => Some(f()),
            _ => None,
        }
    }

    pub fn on_focus<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(bool) -> R,
    {
        match &self.data {
            WindowEventData::Focus(focused) => Some(f(*focused)),
            _ => None,
        }
    }

    pub fn on_position_size<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce([f32; 2], [f32; 2], [f32; 2], [f32; 2]) -> R,
    {
        match &self.data {
            WindowEventData::PositionSize {
                inner_position,
                outer_position,
                inner_size,
                outer_size,
            } => Some(f(*inner_position, *outer_position, *inner_size, *outer_size)),
            _ => None,
        }
    }

    pub fn on_theme<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(winit::window::Theme) -> R,
    {
        match &self.data {
            WindowEventData::Theme(theme) => Some(f(*theme)),
            _ => None,
        }
    }

    pub fn on_scale_factor_changed<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(f64) -> R,
    {
        match &self.data {
            WindowEventData::ScaleFactorChanged { scale_factor } => Some(f(*scale_factor)),
            _ => None,
        }
    }

    pub fn on_occluded<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(bool) -> R,
    {
        match &self.data {
            WindowEventData::Occluded(occluded) => Some(f(*occluded)),
            _ => None,
        }
    }
}

// ----------------------------------------------------------------------------
// WindowEventData
// ----------------------------------------------------------------------------

/// The concrete payload of a window event.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowEventData {
    CloseRequested,
    /// Combined position and size change (either a resize or a move was fired).
    PositionSize {
        inner_position: [f32; 2],
        outer_position: [f32; 2],
        inner_size: [f32; 2],
        outer_size: [f32; 2],
    },
    Focus(bool),
    Theme(winit::window::Theme),
    ScaleFactorChanged { scale_factor: f64 },
    Occluded(bool),
}
