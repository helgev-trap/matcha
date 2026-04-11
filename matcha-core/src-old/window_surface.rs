use gpu_utils::gpu::Gpu;
use log::{debug, trace};
use std::sync::Arc;
use thiserror::Error;
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event_loop::ActiveEventLoop,
    window::{Fullscreen, Window},
};

#[derive(Debug, Clone)]
pub struct WindowSurfaceConfig {
    title: String,
    size: PhysicalSize<u32>,
    maximized: bool,
    fullscreen: bool,
}

impl Default for WindowSurfaceConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowSurfaceConfig {
    pub fn new() -> Self {
        trace!("WindowSurfaceConfig::new: initializing config");
        Self {
            title: "Matcha App".to_string(),
            size: PhysicalSize::new(800, 600),
            maximized: false,
            fullscreen: false,
        }
    }

    pub fn set_title(&mut self, title: &str) {
        trace!("WindowSurfaceConfig::set_title: title={title}");
        self.title = title.to_string();
    }

    pub fn request_inner_size(&mut self, size: PhysicalSize<u32>) {
        trace!(
            "WindowSurfaceConfig::request_inner_size: requested size={}x{}",
            size.width, size.height
        );
        self.size = size;
    }

    pub fn set_maximized(&mut self, maximized: bool) {
        trace!("WindowSurfaceConfig::set_maximized: maximized={maximized}");
        self.maximized = maximized;
    }

    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        trace!("WindowSurfaceConfig::set_fullscreen: fullscreen={fullscreen}");
        self.fullscreen = fullscreen;
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }

    pub fn maximized(&self) -> bool {
        self.maximized
    }

    pub fn fullscreen(&self) -> bool {
        self.fullscreen
    }

    pub fn start_window(
        &self,
        event_loop: &ActiveEventLoop,
        gpu: &Gpu,
    ) -> Result<WindowSurface, WindowSurfaceError> {
        debug!("WindowSurfaceConfig::start_window: starting window lifecycle");

        let window_attributes = Window::default_attributes()
            .with_title(&self.title)
            .with_inner_size(self.size)
            .with_maximized(self.maximized);

        let window = Arc::new(event_loop.create_window(window_attributes)?);
        trace!(
            "WindowSurfaceConfig::start_window: window created ({}x{})",
            self.size.width, self.size.height
        );

        if self.fullscreen {
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        }

        let surface = gpu.instance().create_surface(window.clone())?;
        trace!("WindowSurfaceConfig::start_window: surface created");

        let if_preferred_format_supported = surface
            .get_capabilities(gpu.adapter())
            .formats
            .contains(&gpu.preferred_surface_format());
        trace!(
            "WindowSurfaceConfig::start_window: preferred_format_supported={if_preferred_format_supported}"
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
            "WindowSurfaceConfig::start_window: default config width={} height={} format={:?}",
            surface_config.width, surface_config.height, surface_config.format
        );

        if if_preferred_format_supported {
            surface_config.format = gpu.preferred_surface_format();
            trace!(
                "WindowSurfaceConfig::start_window: applying preferred format {:?}",
                surface_config.format
            );
        }

        surface.configure(&gpu.device(), &surface_config);
        trace!("WindowSurfaceConfig::start_window: surface configured");

        Ok(WindowSurface {
            window,
            surface,
            surface_config,
        })
    }
}

pub struct WindowSurface {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
}

impl WindowSurface {
    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }

    pub fn window_id(&self) -> winit::window::WindowId {
        self.window.id()
    }

    pub fn set_title(&self, title: &str) {
        trace!("WindowSurface::set_title: title={title}");
        self.window.set_title(title);
    }

    pub fn request_inner_size(&self, size: PhysicalSize<u32>) {
        trace!(
            "WindowSurface::request_inner_size: requested size={}x{}",
            size.width, size.height
        );
        let _ = self.window.request_inner_size(size);
    }

    pub fn set_surface_size(&mut self, size: PhysicalSize<u32>, device: &wgpu::Device) {
        if size.width == 0 || size.height == 0 {
            trace!("WindowSurface::set_surface_size: ignoring zero size update");
            return;
        }

        self.surface_config.width = size.width;
        self.surface_config.height = size.height;
        trace!(
            "WindowSurface::set_surface_size: configuring surface to {}x{}",
            size.width, size.height
        );
        self.surface.configure(device, &self.surface_config);
    }

    pub fn set_maximized(&self, maximized: bool) {
        trace!("WindowSurface::set_maximized: maximized={maximized}");
        self.window.set_maximized(maximized);
    }

    pub fn set_fullscreen(&self, fullscreen: bool) {
        trace!("WindowSurface::set_fullscreen: fullscreen={fullscreen}");
        if fullscreen {
            self.window
                .set_fullscreen(Some(Fullscreen::Borderless(None)));
        } else {
            self.window.set_fullscreen(None);
        }
    }

    pub fn reconfigure_surface(&mut self, device: &wgpu::Device) {
        if self.window.inner_size().width == 0 || self.window.inner_size().height == 0 {
            trace!("WindowSurface::reconfigure_surface: skipping due to zero-sized window");
            return;
        }

        self.surface_config.width = self.window.inner_size().width;
        self.surface_config.height = self.window.inner_size().height;
        trace!(
            "WindowSurface::reconfigure_surface: new size {}x{}",
            self.surface_config.width, self.surface_config.height
        );
        self.surface.configure(device, &self.surface_config);
    }

    pub fn request_redraw(&self) {
        trace!("WindowSurface::request_redraw: requested");
        self.window.request_redraw();
    }

    pub fn current_texture(&self) -> Result<wgpu::SurfaceTexture, wgpu::SurfaceError> {
        self.surface.get_current_texture()
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.surface_config.format
    }

    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.window.inner_size()
    }

    pub fn outer_size(&self) -> PhysicalSize<u32> {
        self.window.outer_size()
    }

    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, winit::error::NotSupportedError> {
        self.window.inner_position()
    }

    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, winit::error::NotSupportedError> {
        self.window.outer_position()
    }

    pub fn dpi(&self) -> f64 {
        self.window.scale_factor()
    }

    pub fn into_config(self) -> WindowSurfaceConfig {
        WindowSurfaceConfig {
            title: self.window.title(),
            size: self.window.inner_size(),
            maximized: self.window.is_maximized(),
            fullscreen: self.window.fullscreen().is_some(),
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
