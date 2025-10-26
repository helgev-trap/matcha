use std::sync::Arc;

use log::{debug, error, trace};
use renderer::{CoreRenderer, core_renderer};
use thiserror::Error;

use crate::{
    backend::Backend,
    color::Color,
    context::{ApplicationCommand, GlobalResources},
    window_ui::WindowUi,
};

pub struct ApplicationInstance<
    Message: Send + 'static,
    Event: Send + 'static,
    B: Backend<Event> + Send + Sync + 'static,
> {
    tokio_runtime: tokio::runtime::Runtime,

    global_resources: GlobalResources,

    windows: tokio::sync::RwLock<Vec<WindowUi<Message, Event>>>,
    base_color: Color,

    renderer: CoreRenderer,

    backend: Arc<B>,

    benchmarker: tokio::sync::Mutex<utils::benchmark::Benchmark>,

    frame_count: u128,
}

impl<Message: Send + 'static, Event: Send + 'static, B: Backend<Event> + Send + Sync + 'static>
    ApplicationInstance<Message, Event, B>
{
    pub(crate) fn new(
        tokio_runtime: tokio::runtime::Runtime,
        global_resources: GlobalResources,
        windows: Vec<WindowUi<Message, Event>>,
        base_color: Color,
        renderer: CoreRenderer,
        backend: Arc<B>,
    ) -> Arc<Self> {
        Arc::new(Self {
            tokio_runtime,
            global_resources,
            windows: tokio::sync::RwLock::new(windows),
            base_color,
            renderer,
            backend,
            benchmarker: tokio::sync::Mutex::new(utils::benchmark::Benchmark::new(120)),
            frame_count: 0,
        })
    }
}

/// Syncronous winit event handling.
impl<Message: Send + 'static, Event: Send + 'static, B: Backend<Event> + Send + Sync + 'static>
    ApplicationInstance<Message, Event, B>
{
    pub fn start_all_windows(&self, winit_event_loop: &winit::event_loop::ActiveEventLoop) {
        trace!("ApplicationInstance::start_all_windows: starting all windows");
        self.tokio_runtime.block_on(async {
            let windows = self.windows.read().await;
            trace!(
                "ApplicationInstance::start_all_windows: {} windows to start",
                windows.len()
            );
            for window in &*windows {
                trace!("ApplicationInstance::start_all_windows: starting a window");
                let res = window
                    .start_window(winit_event_loop, self.global_resources.gpu())
                    .await;
                if let Err(e) = res {
                    log::error!(
                        "ApplicationInstance::start_all_windows: failed to start window: {e:?}"
                    );
                }
            }
        });
    }

    pub fn call_all_setups(&self) {
        trace!("ApplicationInstance::call_all_setups: calling setup on all windows");
        self.tokio_runtime.block_on(async {
            let windows = self.windows.read().await;
            for window in &*windows {
                trace!("ApplicationInstance::call_all_setups: calling setup for one window");
                window
                    .setup(&self.tokio_runtime.handle(), &self.global_resources)
                    .await;
            }
        });
    }

    pub fn window_event(
        &self,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        trace!("ApplicationInstance::window_event: window_id={window_id:?} event={event:?}");
        self.tokio_runtime.block_on(async {
            let windows = self.windows.read().await;

            let Some(window) = windows.iter().find(|w| w.window_id() == Some(window_id)) else {
                trace!("ApplicationInstance::window_event: no matching window for id={window_id:?}");
                return;
            };

            trace!("ApplicationInstance::window_event: delivering event to window");

            if let winit::event::WindowEvent::Resized(physical_size) = event {
                trace!("ApplicationInstance::window_event: resize detected {}x{}", physical_size.width, physical_size.height);
                window
                    .resize_window(physical_size, &self.global_resources.gpu().device())
                    .await;
            }

            let event = window
                .window_event(event, self.tokio_runtime.handle(), &self.global_resources)
                .await;

            if let Some(event) = event {
                trace!("ApplicationInstance::window_event: widget produced event, forwarding to backend");
                self.backend.send_event(event).await;
            }
        });
    }

    pub fn user_event(self: &Arc<Self>, message: Message) {
        trace!("ApplicationInstance::user_event: received user event");
        let app_instance = self.clone();
        self.tokio_runtime.spawn(async move {
            let app_instance = app_instance;
            let message = message;
            for window in &*app_instance.windows.read().await {
                trace!("ApplicationInstance::user_event: forwarding to window");
                window.user_event(
                    &message,
                    app_instance.tokio_runtime.handle(),
                    &app_instance.global_resources,
                );
            }
        });
    }

    pub fn try_recv_command(
        &self,
    ) -> Result<ApplicationCommand, tokio::sync::mpsc::error::TryRecvError> {
        self.global_resources.try_recv_command()
    }
}

/// Async rendering loop.
impl<Message: Send + 'static, Event: Send + 'static, B: Backend<Event> + Send + Sync + 'static>
    ApplicationInstance<Message, Event, B>
{
    pub fn start_rendering_loop(self: &Arc<Self>) -> tokio::sync::oneshot::Sender<()> {
        let (exit_signal_sender, exit_signal_receiver) = tokio::sync::oneshot::channel();
        let app_instance = self.clone();

        self.tokio_runtime.spawn(async move {
            app_instance.rendering_loop(exit_signal_receiver).await;
        });

        exit_signal_sender
    }

    pub async fn rendering_loop(
        self: Arc<Self>,
        mut exit_signal: tokio::sync::oneshot::Receiver<()>,
    ) {
        loop {
            // receive exit signal.
            if exit_signal.try_recv().is_ok() {
                break;
            }

            {
                let windows = self.windows.read().await;
                for window in &*windows {
                    if !window.needs_render().await {
                        continue;
                    }

                    window
                        .render(
                            self.tokio_runtime.handle(),
                            &self.global_resources,
                            &self.base_color,
                            &self.renderer,
                            &mut *self.benchmarker.lock().await,
                        )
                        .await;
                }
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum RenderError {
    #[error("Window not found")]
    WindowNotFound,
    // #[error("Window surface error: {0}")]
    // WindowSurface(&'static str),
    #[error(transparent)]
    Surface(#[from] wgpu::SurfaceError),
    #[error(transparent)]
    Render(#[from] core_renderer::TextureValidationError),
}
