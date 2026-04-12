pub mod window_config;
pub use window_config::*;

// --- Backend Abstraction Trait ---

pub trait WindowControler {
    fn create_native_window(
        &self,
        config: &WindowConfig,
        instance: &wgpu::Instance,
        device: &wgpu::Device,
    ) -> Result<WindowSurface, WindowError>;
}

// --- Common Types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId {
    id: usize,
}

pub struct Window {
    config: WindowConfig,
    window_id: WindowId,
    window_surface: Option<WindowSurface>,
}

impl Window {
    pub fn new(
        config: &WindowConfig,
        ctrl: &dyn WindowControler,
        instance: &wgpu::Instance,
        device: &wgpu::Device,
    ) -> Result<Self, WindowError> {
        let window_surface = ctrl.create_native_window(&config, instance, device)?;
        Ok(Self {
            config: config.clone(),
            window_id: window_surface.id(),
            window_surface: Some(window_surface),
        })
    }

    pub fn config(&self) -> &WindowConfig {
        &self.config
    }

    pub fn has_surface(&self) -> bool {
        self.window_surface.is_some()
    }

    /// Discard the native window surface (e.g. on `Suspended`).
    /// The window config is retained so `enable` can recreate the surface later.
    pub fn disable(&mut self) {
        self.window_surface = None;
    }

    /// Recreate the native window surface (e.g. on `Resumed`).
    /// Does nothing if the surface already exists.
    pub fn enable(
        &mut self,
        ctrl: &dyn WindowControler,
        instance: &wgpu::Instance,
        device: &wgpu::Device
    ) -> Result<(), WindowError> {
        if self.window_surface.is_some() {
            return Ok(());
        }
        let surface = ctrl.create_native_window(&self.config, instance, device)?;
        self.window_surface = Some(surface);
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WindowError {
    #[error("Failed to create window: {0}")]
    BackendError(String),
    #[error("Failed to create window surface: {0}")]
    CreateWindowSurface(#[from] wgpu::CreateSurfaceError),
}

// --- Platform-agnostic Window API ---
//
// All type conversions between window_config types and backend-specific types
// are performed inside WindowSurface (winit_window.rs). These methods are
// pure delegations with no backend imports required.
impl Window {
    pub fn id(&self) -> WindowId {
        self.window_id
    }

    // --- Title ---

    pub fn title(&self) -> Option<String> {
        self.window_surface.as_ref().map(|s| s.title())
    }

    pub fn set_title(&self, title: &str) {
        if let Some(s) = &self.window_surface {
            s.set_title(title);
        }
    }

    // --- Size ---

    /// Returns the inner (client area) size in physical pixels, or `None` if
    /// the window has no surface (e.g. minimised / disabled).
    pub fn inner_size(&self) -> Option<[u32; 2]> {
        self.window_surface.as_ref().map(|s| s.inner_size())
    }

    pub fn request_inner_size(&self, width: u32, height: u32) {
        if let Some(s) = &self.window_surface {
            s.request_inner_size(width, height);
        }
    }

    /// Returns the outer (including decorations) size in physical pixels.
    pub fn outer_size(&self) -> Option<[u32; 2]> {
        self.window_surface.as_ref().map(|s| s.outer_size())
    }

    pub fn set_min_inner_size(&self, min_size: Option<window_config::Size>) {
        if let Some(s) = &self.window_surface {
            s.set_min_inner_size(min_size);
        }
    }

    pub fn set_max_inner_size(&self, max_size: Option<window_config::Size>) {
        if let Some(s) = &self.window_surface {
            s.set_max_inner_size(max_size);
        }
    }

    /// Returns the resize increment hint in physical pixels, if set.
    pub fn resize_increments(&self) -> Option<[u32; 2]> {
        self.window_surface
            .as_ref()
            .and_then(|s| s.resize_increments())
    }

    pub fn set_resize_increments(&self, increments: Option<window_config::Size>) {
        if let Some(s) = &self.window_surface {
            s.set_resize_increments(increments);
        }
    }

    // --- Position ---

    /// Returns the inner position (top-left of client area) in physical pixels,
    /// or `None` if unsupported on this platform or no surface is present.
    pub fn inner_position(&self) -> Option<[i32; 2]> {
        self.window_surface
            .as_ref()
            .and_then(|s| s.inner_position())
    }

    /// Returns the outer position (top-left including decorations) in physical
    /// pixels, or `None` if unsupported or no surface is present.
    pub fn outer_position(&self) -> Option<[i32; 2]> {
        self.window_surface
            .as_ref()
            .and_then(|s| s.outer_position())
    }

    pub fn set_outer_position(&self, position: window_config::Position) {
        if let Some(s) = &self.window_surface {
            s.set_outer_position(position);
        }
    }

    // --- Window state ---

    pub fn is_maximized(&self) -> Option<bool> {
        self.window_surface.as_ref().map(|s| s.maximized())
    }

    pub fn set_maximized(&self, maximized: bool) {
        if let Some(s) = &self.window_surface {
            s.set_maximized(maximized);
        }
    }

    pub fn fullscreen(&self) -> Option<window_config::Fullscreen> {
        self.window_surface.as_ref().and_then(|s| s.fullscreen())
    }

    pub fn set_fullscreen(&self, fullscreen: Option<window_config::Fullscreen>) {
        if let Some(s) = &self.window_surface {
            s.set_fullscreen(fullscreen);
        }
    }

    pub fn is_resizable(&self) -> Option<bool> {
        self.window_surface.as_ref().map(|s| s.is_resizable())
    }

    pub fn set_resizable(&self, resizable: bool) {
        if let Some(s) = &self.window_surface {
            s.set_resizable(resizable);
        }
    }

    pub fn is_decorated(&self) -> Option<bool> {
        self.window_surface.as_ref().map(|s| s.is_decorated())
    }

    pub fn set_decorations(&self, decorations: bool) {
        if let Some(s) = &self.window_surface {
            s.set_decorations(decorations);
        }
    }

    pub fn is_visible(&self) -> Option<bool> {
        self.window_surface.as_ref().and_then(|s| s.is_visible())
    }

    pub fn set_visible(&self, visible: bool) {
        if let Some(s) = &self.window_surface {
            s.set_visible(visible);
        }
    }

    // --- Appearance ---

    pub fn theme(&self) -> Option<window_config::Theme> {
        self.window_surface.as_ref().and_then(|s| s.theme())
    }

    pub fn set_theme(&self, theme: Option<window_config::Theme>) {
        if let Some(s) = &self.window_surface {
            s.set_theme(theme);
        }
    }

    pub fn enabled_buttons(&self) -> Option<window_config::WindowButtons> {
        self.window_surface.as_ref().map(|s| s.enabled_buttons())
    }

    pub fn set_enabled_buttons(&self, buttons: window_config::WindowButtons) {
        if let Some(s) = &self.window_surface {
            s.set_enabled_buttons(buttons);
        }
    }

    // --- DPI / surface format ---

    pub fn dpi(&self) -> Option<f64> {
        self.window_surface.as_ref().map(|s| s.dpi())
    }

    pub fn format(&self) -> Option<wgpu::TextureFormat> {
        self.window_surface.as_ref().map(|s| s.format())
    }

    pub fn change_format(&self, device: &wgpu::Device, format: wgpu::TextureFormat) {
        if let Some(s) = &self.window_surface {
            s.change_format(device, format);
        }
    }

    // --- Surface access ---

    pub fn surface(&self) -> Option<&WindowSurface> {
        self.window_surface.as_ref()
    }

    pub fn surface_mut(&self) -> Option<&WindowSurface> {
        self.window_surface.as_ref()
    }

    pub fn window_id(&self) -> Option<WindowId> {
        self.window_surface.as_ref().map(|s| s.window_id())
    }

    pub fn request_redraw(&self) {
        if let Some(s) = &self.window_surface {
            s.request_redraw();
        }
    }
}

#[cfg(feature = "winit")]
mod winit_window;
#[cfg(feature = "winit")]
pub(crate) use winit_window::*;

#[cfg(feature = "baseview")]
mod baseview_window;
#[cfg(feature = "baseview")]
pub(crate) use baseview_window::*;
