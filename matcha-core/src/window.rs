use dashmap::{DashMap, DashSet};
use fxhash::FxBuildHasher;
use std::sync::{Arc, Weak};
use tokio::sync::Mutex;

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

pub struct WindowManager {
    windows: DashMap<WindowId, Arc<Mutex<WindowInner>>, FxBuildHasher>,
    disabled_windows: DashSet<WindowId, FxBuildHasher>,
    weak_self: Weak<Self>,
}

impl WindowManager {
    pub fn new() -> Arc<Self> {
        Arc::new_cyclic(|weak_self| Self {
            windows: DashMap::with_hasher(FxBuildHasher::default()),
            disabled_windows: DashSet::with_hasher(FxBuildHasher::default()),
            weak_self: weak_self.clone(),
        })
    }
}

impl WindowManager {
    pub fn create_window(
        &self,
        ctrl: &impl WindowControler,
        config: &WindowConfig,
        instance: &wgpu::Instance,
        device: &wgpu::Device,
    ) -> Result<Window, WindowError> {
        let window_surface = ctrl.create_native_window(config, instance, device)?;
        let id = window_surface.window_id();

        let window_inner = WindowInner {
            config: config.clone(),
            window_surface: Some(window_surface),
            renderable: None,
            render_task_handle: None,
        };

        self.windows.insert(id, Arc::new(Mutex::new(window_inner)));
        self.disabled_windows.remove(&id);

        Ok(Window {
            id,
            weak_to_manager: self.weak_self.clone(),
        })
    }

    fn remove_window(&self, id: WindowId) {
        self.windows.remove(&id);
        self.disabled_windows.remove(&id);
    }

    pub async fn disable_window(&self, id: WindowId) {
        if let Some(inner) = self.windows.get(&id) {
            let mut inner = inner.lock().await;
            inner.window_surface = None;
            self.disabled_windows.insert(id);
        }
    }

    pub async fn disable_all_windows(&self) {
        for iter in self.windows.iter() {
            let id = *iter.key();
            let mut inner = iter.lock().await;
            inner.window_surface = None;
            self.disabled_windows.insert(id);
        }
    }

    pub async fn enable_window(
        &self,
        id: WindowId,
        ctrl: &impl WindowControler,
        instance: &wgpu::Instance,
        device: &wgpu::Device,
    ) -> Result<(), WindowError> {
        let inner_arc = if let Some(inner) = self.windows.get(&id) {
            inner.clone()
        } else {
            return Ok(());
        };

        let mut inner = inner_arc.lock().await;
        if inner.window_surface.is_some() {
            return Ok(()); // Already enabled
        }

        let window_surface = ctrl.create_native_window(&inner.config, instance, device)?;

        // We update the window_surface and keep the original WindowId to not break UiArch.
        inner.window_surface = Some(window_surface);
        self.disabled_windows.remove(&id);

        Ok(())
    }

    pub async fn enable_all_windows(
        &self,
        ctrl: &impl WindowControler,
        instance: &wgpu::Instance,
        device: &wgpu::Device,
    ) -> Result<(), WindowError> {
        let disabled_ids: Vec<WindowId> = self.disabled_windows.iter().map(|id| *id).collect();
        for id in disabled_ids {
            self.enable_window(id, ctrl, instance, device).await?;
        }
        Ok(())
    }

    async fn set_renderable(&self, id: WindowId, renderable: impl Into<Arc<dyn WindowRenderable>>) {
        if let Some(inner) = self.windows.get(&id) {
            let mut inner = inner.lock().await;
            inner.renderable = Some(renderable.into());
        }
    }

    pub(crate) async fn render_window(
        &self,
        id: WindowId,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        tokio_runtime: &tokio::runtime::Handle,
        if_previous_panicked: Option<impl FnOnce(Box<dyn std::any::Any + Send>) + Send + 'static>,
    ) -> Result<(), wgpu::SurfaceError> {
        let inner_mutex = if let Some(inner) = self.windows.get(&id) {
            inner.clone()
        } else {
            return Ok(());
        };

        let panic_handler = if_previous_panicked.map(|f| Box::new(f) as Box<dyn FnOnce(_) + Send>);

        let mut inner = inner_mutex.lock().await;
        inner
            .render_or_skip(device, queue, tokio_runtime, panic_handler)
            .await
    }
}

pub struct WindowInner {
    config: WindowConfig,
    window_surface: Option<Arc<WindowSurface>>,
    renderable: Option<Arc<dyn WindowRenderable>>,
    render_task_handle: Option<tokio::task::JoinHandle<Result<Option<()>, wgpu::SurfaceError>>>,
}

impl WindowInner {
    async fn render_or_skip(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        tokio_runtime: &tokio::runtime::Handle,
        if_previous_panicked: Option<
            Box<dyn FnOnce(Box<dyn std::any::Any + Send>) + Send + 'static>,
        >,
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
                        if let Some(handler) = if_previous_panicked {
                            handler(panic);
                        } else {
                            std::panic::resume_unwind(panic);
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

pub struct Window {
    id: WindowId,
    weak_to_manager: Weak<WindowManager>,
}

impl Drop for Window {
    fn drop(&mut self) {
        if let Some(manager) = self.weak_to_manager.upgrade() {
            manager.remove_window(self.id);
        }
    }
}

impl Window {
    pub async fn set_renderable(&self, renderable: impl Into<Arc<dyn WindowRenderable>>) {
        if let Some(manager) = self.weak_to_manager.upgrade() {
            manager.set_renderable(self.id, renderable).await;
        }
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
use winit_window::*;

#[cfg(feature = "baseview")]
mod baseview_window;
#[cfg(feature = "baseview")]
use baseview_window::*;
