use dashmap::{DashMap, DashSet};
use fxhash::FxBuildHasher;
use std::sync::{Arc, Mutex as StdMutex, Weak};
use tokio::sync::Mutex;

mod window_config;
pub use window_config::WindowConfig;
mod window_surface;
use window_surface::WindowSurface;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId {
    id: usize,
}

impl WindowId {
    pub(crate) fn from_winit(id: winit::window::WindowId) -> Self {
        let u64_id: u64 = id.into();
        Self {
            id: u64_id as usize,
        }
    }

    pub(crate) fn to_winit(&self) -> winit::window::WindowId {
        winit::window::WindowId::from(self.id as u64)
    }
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
        winit_event_loop: &winit::event_loop::ActiveEventLoop,
        config: &WindowConfig,
        instance: &wgpu::Instance,
        device: &wgpu::Device,
    ) -> Result<Window, WindowError> {
        let window_surface = Arc::new(WindowSurface::new(
            winit_event_loop,
            config,
            instance,
            device,
        )?);
        let id = window_surface.window_id();
        let id = WindowId::from_winit(id);

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
        winit_event_loop: &winit::event_loop::ActiveEventLoop,
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

        let window_surface = Arc::new(WindowSurface::new(
            winit_event_loop,
            &inner.config,
            instance,
            device,
        )?);

        // We update the window_surface and keep the original WindowId to not break UiArch.
        inner.window_surface = Some(window_surface);
        self.disabled_windows.remove(&id);

        Ok(())
    }

    pub async fn enable_all_windows(
        &self,
        winit_event_loop: &winit::event_loop::ActiveEventLoop,
        instance: &wgpu::Instance,
        device: &wgpu::Device,
    ) -> Result<(), WindowError> {
        let disabled_ids: Vec<WindowId> = self.disabled_windows.iter().map(|id| *id).collect();
        for id in disabled_ids {
            self.enable_window(id, winit_event_loop, instance, device)
                .await?;
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
        // Clone the Arc to hold the lock independently of the DashMap ref
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

struct WindowInner {
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
            // skip frame if previous frame is not finished
            if !handle.is_finished() {
                self.render_task_handle = Some(handle);
                return Ok(());
            }

            match handle.await {
                Ok(Ok(_)) => {
                    // frame rendered successfully
                }
                Ok(Err(e)) => {
                    log::error!("Rendering failed: {}", e);
                    return Err(e);
                }
                Err(join_error) => {
                    if join_error.is_panic() {
                        let panic = join_error.into_panic();
                        if let Some(s) = panic.downcast_ref::<&str>() {
                            log::error!("Render task panicked: {:#?}", s);
                        } else if let Some(s) = panic.downcast_ref::<String>() {
                            log::error!("Render task panicked: {:#?}", s);
                        } else {
                            log::error!("Render task panicked with unknown error type");
                        }

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
                            let physical_size = winit::dpi::PhysicalSize::new(
                                surface_config.width,
                                surface_config.height,
                            );

                            let scale_factor = 1.0; // TODO: pipe scale factor from somewhere if needed, or use physical_size

                            let mut context = RenderContext {
                                device: &device,
                                queue: &queue,
                                encoder: &mut encoder,
                                view,
                                target_texture,
                                physical_size,
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
    pub physical_size: winit::dpi::PhysicalSize<u32>,
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
    #[error("Failed to create window")]
    CreateWindow(#[from] winit::error::OsError),
    #[error("Failed to create window surface")]
    CreateWindowSurface(#[from] window_surface::WindowSurfaceError),
}
