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

    /// Process an input WindowEvent from the windowing backend mapping it to a stateful event.
    pub fn process(&mut self, event: WindowEvent) -> WindowEvent {
        let mut out_data = event.data.clone();
        match event.data {
            WindowEventData::Resized {
                inner_size,
                outer_size,
            } => {
                self.inner_size = inner_size;
                self.outer_size = outer_size;
                out_data = WindowEventData::PositionSize {
                    inner_position: self.inner_position,
                    outer_position: self.outer_position,
                    inner_size: self.inner_size,
                    outer_size: self.outer_size,
                };
            }
            WindowEventData::Moved {
                inner_position,
                outer_position,
            } => {
                self.inner_position = inner_position;
                self.outer_position = outer_position;
                out_data = WindowEventData::PositionSize {
                    inner_position: self.inner_position,
                    outer_position: self.outer_position,
                    inner_size: self.inner_size,
                    outer_size: self.outer_size,
                };
            }
            _ => {}
        }
        WindowEvent { data: out_data }
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
    pub fn stateless(data: WindowEventData) -> Self {
        Self { data }
    }

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
            } => Some(f(
                *inner_position,
                *outer_position,
                *inner_size,
                *outer_size,
            )),
            _ => None,
        }
    }

    pub fn on_theme<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(crate::window::window_config::Theme) -> R,
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
    /// Stateless input from windowing backend.
    Resized {
        inner_size: [f32; 2],
        outer_size: [f32; 2],
    },
    /// Stateless input from windowing backend.
    Moved {
        inner_position: [f32; 2],
        outer_position: [f32; 2],
    },
    /// Combined position and size change (either a resize or a move was fired).
    PositionSize {
        inner_position: [f32; 2],
        outer_position: [f32; 2],
        inner_size: [f32; 2],
        outer_size: [f32; 2],
    },
    Focus(bool),
    Theme(crate::window::window_config::Theme),
    ScaleFactorChanged {
        scale_factor: f64,
    },
    Occluded(bool),
}
