use crate::event::{
    EventStateConfig, device_event::DeviceEventState, window_event::WindowEventState,
};
use std::sync::Arc;

pub mod window_config;
pub use window_config::*;

// --- Backend Abstraction Trait ---

pub trait WindowControler {
    fn create_native_window(
        &self,
        config: &WindowConfig,
        instance: &wgpu::Instance,
        device: &wgpu::Device,
    ) -> Result<Arc<WindowSurface>, WindowError>;
}

// --- Common Types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId {
    id: usize,
}

pub struct Window {
    // window
    config: WindowConfig,
    window_surface: Option<Arc<WindowSurface>>,
    renderable: Option<Arc<dyn WindowRenderable>>,

    // render task
    render_task_handle: Option<tokio::task::JoinHandle<Result<Option<()>, wgpu::SurfaceError>>>,

    // event states
    // TODO: use methods instead of public fields
    pub(crate) device_event_state: DeviceEventState,
    pub(crate) window_event_state: WindowEventState,
}

impl Window {
    pub(crate) fn new(
        config: WindowConfig,
        window_surface: Arc<WindowSurface>,
        event_config: &EventStateConfig,
    ) -> Self {
        Self {
            config,
            window_surface: Some(window_surface),
            renderable: None,
            render_task_handle: None,
            device_event_state: DeviceEventState::new(event_config.mouse)
                .expect("EventStateConfig passed to Window::new must be valid"),
            window_event_state: WindowEventState::default(),
        }
    }

    pub(crate) fn config(&self) -> &WindowConfig {
        &self.config
    }

    pub(crate) fn set_surface(&mut self, surface: Option<Arc<WindowSurface>>) {
        self.window_surface = surface;
    }

    pub(crate) fn has_surface(&self) -> bool {
        self.window_surface.is_some()
    }

    pub(crate) fn set_renderable(&mut self, renderable: Option<Arc<dyn WindowRenderable>>) {
        self.renderable = renderable;
    }

    pub(crate) async fn render_or_skip(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        tokio_runtime: &tokio::runtime::Handle,
        if_previous_panicked: Option<impl FnOnce(Box<dyn std::any::Any + Send>) + Send + 'static>,
    ) -> Result<(), wgpu::SurfaceError> {
        if let Some(handle) = self.render_task_handle.take() {
            if !handle.is_finished() {
                self.render_task_handle = Some(handle);
                return Ok(());
            }

            match handle.await {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => {
                    log::error!("Rendering failed: {}", e);
                    return Err(e);
                }
                Err(join_error) => {
                    if join_error.is_panic() {
                        let panic = join_error.into_panic();

                        match if_previous_panicked {
                            Some(handler) => handler(panic),
                            None => std::panic::resume_unwind(panic),
                        }
                    }
                }
            }
        }

        if let Some(window_surface) = &self.window_surface {
            if let Some(renderable) = self.renderable.as_ref()
                && renderable.is_updated().await
            {
                let window_surface = window_surface.clone();
                let device = device.clone();
                let queue = queue.clone();
                let renderable = renderable.clone();

                let join_handle = tokio_runtime.spawn(async move {
                    window_surface.rendering_with_surface_texture(
                        &device,
                        |view, target_texture| {
                            let mut encoder =
                                device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                    label: Some("Window Render Encoder"),
                                });

                            let surface_config = window_surface.surface_config();
                            let width = surface_config.width;
                            let height = surface_config.height;

                            let scale_factor = 1.0;

                            let mut context = RenderContext {
                                device: &device,
                                queue: &queue,
                                encoder: &mut encoder,
                                view,
                                target_texture,
                                width,
                                height,
                                scale_factor,
                            };

                            renderable.render(&mut context);

                            queue.submit(std::iter::once(encoder.finish()));
                        },
                    )
                });
                self.render_task_handle = Some(join_handle);
            }
        }

        Ok(())
    }
}

pub struct RenderContext<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub view: &'a wgpu::TextureView,
    pub target_texture: &'a wgpu::Texture,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

#[async_trait::async_trait]
pub trait WindowRenderable: Send + Sync + 'static {
    async fn is_updated(&self) -> bool {
        true
    }

    fn render(&self, ctx: &mut RenderContext);
}

#[derive(Debug, thiserror::Error)]
pub enum WindowError {
    #[error("Failed to create window: {0}")]
    BackendError(String),
    #[error("Failed to create window surface: {0}")]
    CreateWindowSurface(#[from] wgpu::CreateSurfaceError),
}

#[cfg(feature = "winit")]
mod winit_window;
#[cfg(feature = "winit")]
pub(crate) use winit_window::*;

// --- Platform-agnostic Window API ---
//
// These methods are defined here (not in window.rs) to keep window.rs free of
// winit dependencies. All public signatures use types from window_config.

impl Window {
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
        self.window_surface.as_ref().map(|s| {
            let size = s.inner_size();
            [size.width, size.height]
        })
    }

    pub fn request_inner_size(&self, width: u32, height: u32) {
        if let Some(s) = &self.window_surface {
            s.request_inner_size(winit::dpi::PhysicalSize::new(width, height));
        }
    }

    /// Returns the outer (including decorations) size in physical pixels.
    pub fn outer_size(&self) -> Option<[u32; 2]> {
        self.window_surface.as_ref().map(|s| {
            let size = s.outer_size();
            [size.width, size.height]
        })
    }

    pub fn set_min_inner_size(&self, min_size: Option<window_config::Size>) {
        if let Some(s) = &self.window_surface {
            s.set_min_inner_size(min_size.map(Into::into));
        }
    }

    pub fn set_max_inner_size(&self, max_size: Option<window_config::Size>) {
        if let Some(s) = &self.window_surface {
            s.set_max_inner_size(max_size.map(Into::into));
        }
    }

    /// Returns the resize increment hint in physical pixels, if set.
    pub fn resize_increments(&self) -> Option<[u32; 2]> {
        self.window_surface
            .as_ref()
            .and_then(|s| s.resize_increments().map(|size| [size.width, size.height]))
    }

    pub fn set_resize_increments(&self, increments: Option<window_config::Size>) {
        if let Some(s) = &self.window_surface {
            s.set_resize_increments(increments.map(Into::into));
        }
    }

    // --- Position ---

    /// Returns the inner position (top-left of client area) in physical pixels,
    /// or `None` if unsupported on this platform or no surface is present.
    pub fn inner_position(&self) -> Option<[i32; 2]> {
        self.window_surface
            .as_ref()
            .and_then(|s| s.inner_position().ok().map(|p| [p.x, p.y]))
    }

    /// Returns the outer position (top-left including decorations) in physical
    /// pixels, or `None` if unsupported or no surface is present.
    pub fn outer_position(&self) -> Option<[i32; 2]> {
        self.window_surface
            .as_ref()
            .and_then(|s| s.outer_position().ok().map(|p| [p.x, p.y]))
    }

    pub fn set_outer_position(&self, position: window_config::Position) {
        if let Some(s) = &self.window_surface {
            match position {
                window_config::Position::Physical { x, y } => {
                    s.request_outer_position_physical(winit::dpi::PhysicalPosition::new(x, y));
                }
                window_config::Position::Logical { x, y } => {
                    s.request_outer_position_logical(winit::dpi::LogicalPosition::new(x, y));
                }
            }
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
        self.window_surface
            .as_ref()
            .and_then(|s| s.fullscreen().map(Into::into))
    }

    pub fn set_fullscreen(&self, fullscreen: Option<window_config::Fullscreen>) {
        if let Some(s) = &self.window_surface {
            s.set_fullscreen(fullscreen.map(Into::into));
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
        self.window_surface
            .as_ref()
            .and_then(|s| s.theme().map(Into::into))
    }

    pub fn set_theme(&self, theme: Option<window_config::Theme>) {
        if let Some(s) = &self.window_surface {
            s.set_theme(theme.map(Into::into));
        }
    }

    pub fn enabled_buttons(&self) -> Option<window_config::WindowButtons> {
        self.window_surface
            .as_ref()
            .map(|s| s.enabled_buttons().into())
    }

    pub fn set_enabled_buttons(&self, buttons: window_config::WindowButtons) {
        if let Some(s) = &self.window_surface {
            s.set_enabled_buttons(buttons.into());
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
}

#[cfg(feature = "baseview")]
mod baseview_window;
#[cfg(feature = "baseview")]
pub(crate) use baseview_window::*;
