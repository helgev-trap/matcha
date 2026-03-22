use crate::event::{device_event::DeviceEventState, window_event::WindowEventState};
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
    pub(crate) fn new(config: WindowConfig, window_surface: Arc<WindowSurface>) -> Self {
        Self {
            config,
            window_surface: Some(window_surface),
            renderable: None,
            render_task_handle: None,
            device_event_state: DeviceEventState::new(),
            window_event_state: WindowEventState::new(),
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

#[cfg(feature = "baseview")]
mod baseview_window;
#[cfg(feature = "baseview")]
pub(crate) use baseview_window::*;
