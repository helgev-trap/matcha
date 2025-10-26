use gpu_utils::gpu::Gpu;
use log::{debug, trace};
use std::sync::Arc;
use thiserror::Error;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event_loop::ActiveEventLoop,
    window::{Fullscreen, Window},
};

pub enum WindowSurface {
    Window {
        window: Arc<Window>,
        surface: wgpu::Surface<'static>,
        surface_config: wgpu::SurfaceConfiguration,
    },
    Config {
        title: String,
        size: PhysicalSize<u32>,
        maximized: bool,
        fullscreen: bool,
    },
}

impl Default for WindowSurface {
    fn default() -> Self {
        Self::new()
    }
}

/// Creation and configuration
impl WindowSurface {
    pub fn new() -> Self {
        trace!("WindowSurface::new: initializing config");
        Self::Config {
            title: "Matcha App".to_string(),
            size: PhysicalSize::new(800, 600),
            maximized: false,
            fullscreen: false,
        }
    }

    pub fn set_title(&mut self, title: &str) {
        trace!("WindowSurface::set_title: title={title}");
        match self {
            WindowSurface::Window { window, .. } => {
                window.set_title(title);
            }
            WindowSurface::Config { title: t, .. } => {
                *t = title.to_string();
            }
        }
    }

    /// Request a resize of the window. In Config mode, this just updates the size configuration.
    /// Note that this method does not change surface configuration.
    /// This method will cause a `Resized` event to be emitted.
    /// After that, you should call `set_surface_size` to update the surface configuration.
    pub fn request_inner_size(&mut self, size: PhysicalSize<u32>) {
        trace!(
            "WindowSurface::request_inner_size: requested size={}x{}",
            size.width, size.height
        );
        match self {
            WindowSurface::Window { window, .. } => {
                let _ = window.request_inner_size(size);
            }
            WindowSurface::Config { size: s, .. } => {
                *s = size;
            }
        }
    }

    /// Update the surface configuration size. No-op if in Config mode.
    pub fn set_surface_size(&mut self, size: PhysicalSize<u32>, device: &wgpu::Device) {
        if size.width == 0 || size.height == 0 {
            trace!("WindowSurface::set_surface_size: ignoring zero size update");
            return;
        }

        match self {
            WindowSurface::Window {
                surface,
                surface_config,
                ..
            } => {
                surface_config.width = size.width;
                surface_config.height = size.height;
                trace!(
                    "WindowSurface::set_surface_size: configuring surface to {}x{}",
                    size.width, size.height
                );
                surface.configure(device, surface_config);
            }
            _ => {
                trace!("WindowSurface::set_surface_size: in Config mode, no surface to configure");
            }
        }
    }

    pub fn set_maximized(&mut self, maximized: bool) {
        trace!("WindowSurface::set_maximized: maximized={maximized}");
        match self {
            WindowSurface::Window { window, .. } => {
                window.set_maximized(maximized);
            }
            WindowSurface::Config { maximized: m, .. } => {
                *m = maximized;
            }
        }
    }

    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        trace!("WindowSurface::set_fullscreen: fullscreen={fullscreen}");
        match self {
            WindowSurface::Window { window, .. } => {
                if fullscreen {
                    window.set_fullscreen(Some(Fullscreen::Borderless(None)));
                } else {
                    window.set_fullscreen(None);
                }
            }
            WindowSurface::Config { fullscreen: f, .. } => {
                *f = fullscreen;
            }
        }
    }

    // --- Winit Integration ---

    pub fn start_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        gpu: &Gpu,
    ) -> Result<(), WindowSurfaceError> {
        debug!("WindowSurface::start_window: starting window lifecycle");
        // If already Window, nothing to do
        if let WindowSurface::Window { .. } = self {
            trace!("WindowSurface::start_window: already initialized");
            return Ok(());
        }

        // Extract configuration values (clone as needed)
        let (title, init_size, maximized, fullscreen) = match self {
            WindowSurface::Config {
                title,
                size: init_size,
                maximized,
                fullscreen,
            } => (title.clone(), *init_size, *maximized, *fullscreen),
            _ => unreachable!(),
        };

        let window_attributes = Window::default_attributes()
            .with_title(&title)
            .with_inner_size(init_size)
            .with_maximized(maximized);

        let window = Arc::new(event_loop.create_window(window_attributes)?);
        trace!(
            "WindowSurface::start_window: window created ({}x{})",
            init_size.width, init_size.height
        );

        if fullscreen {
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        }

        let surface = gpu.instance().create_surface(window.clone())?;
        trace!("WindowSurface::start_window: surface created");

        let if_preferred_format_supported = surface
            .get_capabilities(gpu.adapter())
            .formats
            .contains(&gpu.preferred_surface_format());
        trace!(
            "WindowSurface::start_window: preferred_format_supported={if_preferred_format_supported}"
        );

        let mut surface_config = surface
            .get_default_config(
                gpu.adapter(),
                window.inner_size().width,
                window.inner_size().height,
            )
            .map(|mut config| {
                config.usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
                config.present_mode = wgpu::PresentMode::AutoVsync;
                config.desired_maximum_frame_latency = 1;
                config.alpha_mode = wgpu::CompositeAlphaMode::Auto;
                config
            })
            .ok_or(WindowSurfaceError::SurfaceConfiguration)?;
        trace!(
            "WindowSurface::start_window: default config width={} height={} format={:?}",
            surface_config.width, surface_config.height, surface_config.format
        );

        if if_preferred_format_supported {
            surface_config.format = gpu.preferred_surface_format();
            trace!(
                "WindowSurface::start_window: applying preferred format {:?}",
                surface_config.format
            );
        }

        surface.configure(&gpu.device(), &surface_config);
        trace!("WindowSurface::start_window: surface configured");

        // Replace self with Window variant, preserving settings
        *self = WindowSurface::Window {
            window,
            surface,
            surface_config,
        };

        debug!("WindowSurface::start_window: window initialization complete");

        Ok(())
    }

    pub fn reconfigure_surface(&mut self, device: &wgpu::Device) {
        if let WindowSurface::Window {
            surface,
            surface_config,
            window,
        } = self
        {
            if window.inner_size().width == 0 || window.inner_size().height == 0 {
                trace!("WindowSurface::reconfigure_surface: skipping due to zero-sized window");
                return;
            }

            surface_config.width = window.inner_size().width;
            surface_config.height = window.inner_size().height;
            trace!(
                "WindowSurface::reconfigure_surface: new size {}x{}",
                surface_config.width, surface_config.height
            );
            surface.configure(device, surface_config);
        }
    }

    pub fn end_window(&mut self) {
        // If already Config, nothing to do
        if let WindowSurface::Config { .. } = self {
            trace!("WindowSurface::end_window: already in config mode");
            return;
        }

        // Extract configuration values (clone as needed)
        let (title, size, maximized, fullscreen) = match self {
            WindowSurface::Window { window, .. } => (
                window.title().to_string(),
                window.inner_size(),
                window.is_maximized(),
                window.fullscreen().is_some(),
            ),
            _ => unreachable!(),
        };

        // Replace self with Config variant, preserving settings
        *self = WindowSurface::Config {
            title,
            size,
            maximized,
            fullscreen,
        };
        debug!("WindowSurface::end_window: window torn down");
    }
}

/// window operations
impl WindowSurface {
    pub fn request_redraw(&self) {
        if let WindowSurface::Window { window, .. } = self {
            trace!("WindowSurface::request_redraw: requested");
            window.request_redraw();
        }
    }
}

/// getters
impl WindowSurface {
    pub fn window(&self) -> Option<&Arc<Window>> {
        match self {
            WindowSurface::Window { window, .. } => Some(window),
            WindowSurface::Config { .. } => None,
        }
    }

    pub fn window_id(&self) -> Option<winit::window::WindowId> {
        match self {
            WindowSurface::Window { window, .. } => Some(window.id()),
            WindowSurface::Config { .. } => None,
        }
    }

    pub fn current_texture(&self) -> Option<Result<wgpu::SurfaceTexture, wgpu::SurfaceError>> {
        match self {
            WindowSurface::Window { surface, .. } => Some(surface.get_current_texture()),
            WindowSurface::Config { .. } => None,
        }
    }

    pub fn format(&self) -> Option<wgpu::TextureFormat> {
        match self {
            WindowSurface::Window { surface_config, .. } => Some(surface_config.format),
            WindowSurface::Config { .. } => None,
        }
    }

    pub fn inner_size(&self) -> Option<PhysicalSize<u32>> {
        match self {
            WindowSurface::Window { window, .. } => Some(window.inner_size()),
            WindowSurface::Config {
                size: init_size, ..
            } => Some(*init_size),
        }
    }

    pub fn outer_size(&self) -> Option<PhysicalSize<u32>> {
        match self {
            WindowSurface::Window { window, .. } => Some(window.outer_size()),
            WindowSurface::Config {
                size: init_size, ..
            } => Some(*init_size),
        }
    }

    pub fn inner_position(
        &self,
    ) -> Option<Result<PhysicalPosition<i32>, winit::error::NotSupportedError>> {
        match self {
            WindowSurface::Window { window, .. } => Some(window.inner_position()),
            WindowSurface::Config { .. } => None,
        }
    }

    pub fn outer_position(
        &self,
    ) -> Option<Result<PhysicalPosition<i32>, winit::error::NotSupportedError>> {
        match self {
            WindowSurface::Window { window, .. } => Some(window.outer_position()),
            WindowSurface::Config { .. } => None,
        }
    }

    pub fn dpi(&self) -> Option<f64> {
        match self {
            WindowSurface::Window { window, .. } => Some(window.scale_factor()),
            WindowSurface::Config { .. } => None,
        }
    }
}

#[derive(Debug, Error)]
pub enum WindowSurfaceError {
    #[error(transparent)]
    Os(#[from] winit::error::OsError),
    #[error(transparent)]
    CreateSurface(#[from] wgpu::CreateSurfaceError),
    #[error("Failed to get surface configuration")]
    SurfaceConfiguration,
}
