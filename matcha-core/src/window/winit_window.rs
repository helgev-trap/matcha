use std::sync::Arc;

// Implement WindowControler for Winit EventLoop
impl super::WindowControler for winit::event_loop::ActiveEventLoop {
    fn create_native_window(
        &self,
        config: &super::WindowConfig,
        instance: &wgpu::Instance,
        device: &wgpu::Device,
    ) -> Result<std::sync::Arc<WindowSurface>, super::WindowError> {
        let surface = WindowSurface::new(self, config, instance, device)
            .map_err(|e| super::WindowError::BackendError(e.to_string()))?;
        Ok(std::sync::Arc::new(surface))
    }
}

pub struct WindowSurface {
    window: Arc<winit::window::Window>,
    surface: wgpu::Surface<'static>,
    /// texture size is ensured to be greater than 0
    // TODO: wgpu v28.0.0 can get current config from surface. Fix this in the future.
    current_config: parking_lot::Mutex<wgpu::SurfaceConfiguration>,
}

/// Constructor
impl WindowSurface {
    pub fn new(
        event_loop: &winit::event_loop::ActiveEventLoop,
        config: &super::window_config::WindowConfig,
        instance: &wgpu::Instance,
        device: &wgpu::Device,
    ) -> Result<Self, WindowSurfaceError> {
        let window = event_loop
            .create_window(config.to_winit_attributes())
            .map_err(WindowSurfaceError::CreateWindow)?;

        let window = Arc::new(window);
        let surface = instance
            .create_surface(Arc::clone(&window))
            .map_err(WindowSurfaceError::CreateWindowSurface)?;
        let (width, height) = window.inner_size().into();

        let surface_config = wgpu::SurfaceConfiguration {
            width,
            height,
            usage: config.surface_config.usage,
            format: config.surface_config.format,
            view_formats: Vec::new(),
            present_mode: config.surface_config.present_mode,
            desired_maximum_frame_latency: config.surface_config.desired_maximum_frame_latency,
            alpha_mode: config.surface_config.alpha_mode,
        };

        surface.configure(device, &surface_config);

        Ok(Self {
            window,
            surface,
            current_config: parking_lot::Mutex::new(surface_config),
        })
    }

    pub fn window(&self) -> &winit::window::Window {
        &self.window
    }

    pub fn window_id(&self) -> super::WindowId {
        let u64_id: u64 = self.window.id().into();
        super::WindowId {
            id: u64_id as usize,
        }
    }

    pub fn surface(&self) -> &wgpu::Surface<'_> {
        &self.surface
    }
}

/// Setters and Getters
impl WindowSurface {
    pub fn title(&self) -> String {
        self.window.title()
    }

    pub fn set_title(&self, title: &str) {
        self.window.set_title(title);
    }

    pub fn maximized(&self) -> bool {
        self.window.is_maximized()
    }

    pub fn set_maximized(&self, maximized: bool) {
        self.window.set_maximized(maximized);
    }

    pub fn fullscreen(&self) -> Option<winit::window::Fullscreen> {
        self.window.fullscreen()
    }

    pub fn set_fullscreen(&self, fullscreen: Option<winit::window::Fullscreen>) {
        self.window.set_fullscreen(fullscreen);
    }

    pub fn inner_size(&self) -> winit::dpi::PhysicalSize<u32> {
        self.window.inner_size()
    }

    // todo: handle request result
    pub fn request_inner_size(&self, size: winit::dpi::PhysicalSize<u32>) {
        let _ = self.window.request_inner_size(size);
    }

    pub fn outer_size(&self) -> winit::dpi::PhysicalSize<u32> {
        self.window.outer_size()
    }

    pub fn request_outer_size(&self, size: impl Into<winit::dpi::Position>) {
        self.window.set_outer_position(size);
    }

    pub fn inner_position(
        &self,
    ) -> Result<winit::dpi::PhysicalPosition<i32>, winit::error::NotSupportedError> {
        self.window.inner_position()
    }

    pub fn outer_position(
        &self,
    ) -> Result<winit::dpi::PhysicalPosition<i32>, winit::error::NotSupportedError> {
        self.window.outer_position()
    }

    pub fn request_outer_position_physical(&self, position: winit::dpi::PhysicalPosition<i32>) {
        self.window.set_outer_position(position);
    }

    pub fn request_outer_position_logical(&self, position: winit::dpi::LogicalPosition<f64>) {
        self.window.set_outer_position(position);
    }

    pub fn dpi(&self) -> f64 {
        self.window.scale_factor()
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.current_config.lock().format
    }

    pub fn change_format(&self, device: &wgpu::Device, format: wgpu::TextureFormat) {
        let mut config = self.current_config.lock();
        config.format = format;
        self.surface.configure(device, &config);
    }

    pub fn is_resizable(&self) -> bool {
        self.window.is_resizable()
    }

    pub fn set_resizable(&self, resizable: bool) {
        self.window.set_resizable(resizable);
    }

    pub fn enabled_buttons(&self) -> winit::window::WindowButtons {
        self.window.enabled_buttons()
    }

    pub fn set_enabled_buttons(&self, buttons: winit::window::WindowButtons) {
        self.window.set_enabled_buttons(buttons);
    }

    pub fn is_decorated(&self) -> bool {
        self.window.is_decorated()
    }

    pub fn set_decorations(&self, decorations: bool) {
        self.window.set_decorations(decorations);
    }

    pub fn theme(&self) -> Option<winit::window::Theme> {
        self.window.theme()
    }

    pub fn set_theme(&self, theme: Option<winit::window::Theme>) {
        self.window.set_theme(theme);
    }

    pub fn is_visible(&self) -> Option<bool> {
        self.window.is_visible()
    }

    pub fn set_visible(&self, visible: bool) {
        self.window.set_visible(visible);
    }

    pub fn resize_increments(&self) -> Option<winit::dpi::PhysicalSize<u32>> {
        self.window.resize_increments()
    }

    pub fn set_resize_increments(&self, increments: Option<winit::dpi::Size>) {
        self.window.set_resize_increments(increments);
    }

    pub fn set_min_inner_size(&self, min_size: Option<winit::dpi::Size>) {
        self.window.set_min_inner_size(min_size);
    }

    pub fn set_max_inner_size(&self, max_size: Option<winit::dpi::Size>) {
        self.window.set_max_inner_size(max_size);
    }
}

impl WindowSurface {
    pub fn get_config(&self) -> super::WindowConfig {
        let config = self.current_config.lock();
        use crate::window::window_config::{Position, Size, WindowButtons};

        let inner_size = self.window.inner_size();
        let position = self
            .window
            .outer_position()
            .ok()
            .map(|p| Position::Physical { x: p.x, y: p.y });

        super::WindowConfig {
            title: self.window.title(),
            inner_size: Some(Size::Physical {
                width: inner_size.width,
                height: inner_size.height,
            }),
            min_inner_size: None, // needs mapping if needed
            max_inner_size: None,
            position,
            resizable: self.window.is_resizable(),
            enabled_buttons: WindowButtons::ALL, // needs mapping if needed
            maximized: self.window.is_maximized(),
            fullscreen: None, // needs mapping if needed
            visible: self.window.is_visible().unwrap_or(true),
            transparent: false, // needs mapping
            decorations: self.window.is_decorated(),
            preferred_theme: None,
            resize_increments: None,
            active: self.window.has_focus(),
            surface_config: config.clone(),
        }
    }

    pub fn surface_config(&self) -> wgpu::SurfaceConfiguration {
        self.current_config.lock().clone()
    }
}

/// Operations
impl WindowSurface {
    pub fn resize(&self, size: [u32; 2], device: &wgpu::Device) {
        if size[0] != 0 && size[1] != 0 {
            let mut config = self.current_config.lock();
            config.width = size[0];
            config.height = size[1];
            self.surface.configure(device, &config);
        }
    }

    pub fn reconfigure(&self, device: &wgpu::Device) {
        let size = self.window.inner_size();

        if size.width != 0 && size.height != 0 {
            let mut config = self.current_config.lock();
            config.width = size.width;
            config.height = size.height;
            self.surface.configure(device, &config);
        }
    }

    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }
}

/// Rendering
impl WindowSurface {
    pub fn rendering_with_surface_texture<R>(
        &self,
        device: &wgpu::Device,
        f: impl FnOnce(&wgpu::TextureView, &wgpu::Texture) -> R,
    ) -> Result<Option<R>, wgpu::SurfaceError> {
        match self.get_surface_texture(device)? {
            Some(surface_texture) => {
                let view = surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let result = f(&view, &surface_texture.texture);
                surface_texture.present();
                Ok(Some(result))
            }
            None => Ok(None),
        }
    }

    pub fn try_rendering_with_surface_texture<R, E>(
        &self,
        device: &wgpu::Device,
        f: impl FnOnce(wgpu::TextureView) -> Result<R, E>,
    ) -> Result<Option<Result<R, E>>, wgpu::SurfaceError> {
        match self.get_surface_texture(device)? {
            Some(surface_texture) => {
                let view = surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                let result = f(view);
                surface_texture.present();
                Ok(Some(result))
            }
            None => Ok(None),
        }
    }
}

impl WindowSurface {
    /// Return Value:
    /// - `Ok(Some(texture))`: Success to acquire surface texture.
    /// - `Ok(None)`: Timeout or when texture size is zero. Frame will be skipped.
    /// - `Err(wgpu::SurfaceError)`: Other unrecoverable error.
    pub fn get_surface_texture(
        &self,
        device: &wgpu::Device,
    ) -> Result<Option<wgpu::SurfaceTexture>, wgpu::SurfaceError> {
        match self.surface.get_current_texture() {
            Ok(texture) => Ok(Some(texture)),
            Err(wgpu::SurfaceError::Timeout) => {
                log::warn!("Surface texture acquire timed out. Skipping frame.");
                Ok(None)
            }
            Err(wgpu::SurfaceError::Outdated) | Err(wgpu::SurfaceError::Lost) => {
                log::warn!("Surface is outdated or lost. Reconfiguring and retrying...");
                // Reconfigure surface
                let size = self.window.inner_size();
                if size.width == 0 || size.height == 0 {
                    // Could not recover valid size (e.g. still minimized or race condition)
                    Ok(None)
                } else {
                    self.reconfigure(device);
                    // Retry once
                    match self.surface.get_current_texture() {
                        Ok(texture) => Ok(Some(texture)),
                        Err(wgpu::SurfaceError::Timeout) => {
                            log::warn!(
                                "Surface texture acquire timed out on retry. Skipping frame."
                            );
                            Ok(None)
                        }
                        Err(e) => Err(e),
                    }
                }
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                log::error!("System is out of memory. Application should probably exit.");
                Err(wgpu::SurfaceError::OutOfMemory)
            }
            Err(wgpu::SurfaceError::Other) => {
                log::error!("Unknown surface error");
                Err(wgpu::SurfaceError::Other)
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WindowSurfaceError {
    #[error("Failed to create window")]
    CreateWindow(winit::error::OsError),
    #[error("Failed to create window surface")]
    CreateWindowSurface(wgpu::CreateSurfaceError),
    #[error("Failed to get default surface config")]
    GetDefaultSurfaceConfig,
}
