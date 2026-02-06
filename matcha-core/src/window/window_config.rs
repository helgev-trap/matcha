#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub window_attributes: winit::window::WindowAttributes,
    /// Note: Values of `width` and `height` in `surface_config` will be ignored.
    pub surface_config: wgpu::SurfaceConfiguration,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            window_attributes: Default::default(),
            surface_config: wgpu::SurfaceConfiguration {
                width: 100,
                height: 100,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                view_formats: Vec::new(),
                present_mode: wgpu::PresentMode::Fifo,
                desired_maximum_frame_latency: 1,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
            },
        }
    }
}

impl WindowConfig {
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.window_attributes.title = title.into();
        self
    }

    pub fn with_inner_size(mut self, size: impl Into<winit::dpi::Size>) -> Self {
        self.window_attributes.inner_size = Some(size.into());
        self
    }

    pub fn with_min_inner_size(mut self, size: impl Into<winit::dpi::Size>) -> Self {
        self.window_attributes.min_inner_size = Some(size.into());
        self
    }

    pub fn with_max_inner_size(mut self, size: impl Into<winit::dpi::Size>) -> Self {
        self.window_attributes.max_inner_size = Some(size.into());
        self
    }

    pub fn with_position(mut self, position: impl Into<winit::dpi::Position>) -> Self {
        self.window_attributes.position = Some(position.into());
        self
    }

    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.window_attributes.resizable = resizable;
        self
    }

    pub fn with_enabled_buttons(mut self, buttons: winit::window::WindowButtons) -> Self {
        self.window_attributes.enabled_buttons = buttons;
        self
    }

    pub fn with_maximized(mut self, maximized: bool) -> Self {
        self.window_attributes.maximized = maximized;
        self
    }

    pub fn with_fullscreen(mut self, fullscreen: Option<winit::window::Fullscreen>) -> Self {
        self.window_attributes.fullscreen = fullscreen;
        self
    }

    pub fn with_visible(mut self, visible: bool) -> Self {
        self.window_attributes.visible = visible;
        self
    }

    pub fn with_transparent(mut self, transparent: bool) -> Self {
        self.window_attributes.transparent = transparent;
        self
    }

    pub fn with_decorations(mut self, decorations: bool) -> Self {
        self.window_attributes.decorations = decorations;
        self
    }

    pub fn with_preferred_theme(mut self, theme: Option<winit::window::Theme>) -> Self {
        self.window_attributes.preferred_theme = theme;
        self
    }

    pub fn with_resize_increments(mut self, increments: impl Into<winit::dpi::Size>) -> Self {
        self.window_attributes.resize_increments = Some(increments.into());
        self
    }

    pub fn with_active(mut self, active: bool) -> Self {
        self.window_attributes.active = active;
        self
    }

    pub fn with_surface_usage(mut self, usage: wgpu::TextureUsages) -> Self {
        self.surface_config.usage = usage;
        self
    }

    pub fn with_surface_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.surface_config.format = format;
        self
    }

    pub fn with_surface_present_mode(mut self, present_mode: wgpu::PresentMode) -> Self {
        self.surface_config.present_mode = present_mode;
        self
    }

    pub fn with_surface_alpha_mode(mut self, alpha_mode: wgpu::CompositeAlphaMode) -> Self {
        self.surface_config.alpha_mode = alpha_mode;
        self
    }
}
