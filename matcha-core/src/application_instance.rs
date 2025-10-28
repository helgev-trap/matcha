use std::{
    collections::HashMap,
    sync::Arc,
};

use gpu_utils::device_loss_recoverable::DeviceLossRecoverable;
use renderer::CoreRenderer;

use crate::{
    backend::Backend,
    color::Color,
    context::{ApplicationCommand, GlobalResources},
    window_ui::{WindowUi, WindowUiConfig},
};

pub struct ApplicationInstance<
    Message: Send + 'static,
    Event: Send + 'static,
    B: Backend<Event> + Send + Sync + 'static,
> {
    tokio_runtime: tokio::runtime::Runtime,

    global_resources: GlobalResources,

    windows: tokio::sync::RwLock<
        HashMap<winit::window::WindowId, WindowUi<Message, Event>, fxhash::FxBuildHasher>,
    >,
    not_started_uis: tokio::sync::Mutex<Vec<WindowUiConfig<Message, Event>>>,

    // todo: make this per-window?
    base_color: Color,
    renderer: CoreRenderer,

    backend: Arc<B>,

    benchmarker: tokio::sync::Mutex<utils::benchmark::Benchmark>,

    frame_count: std::sync::atomic::AtomicU64,

    device_lost_callback_id: parking_lot::Mutex<Option<gpu_utils::gpu::CallbackId>>,
    device_recover_callback_id: parking_lot::Mutex<Option<gpu_utils::gpu::CallbackId>>,

    // exit signal is used to stop the rendering loop gracefully.
    // this task handle is used to kill the rendering loop task when needed.
    render_loop_task_handle: tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl<Message: Send + 'static, Event: Send + 'static, B: Backend<Event> + Send + Sync + 'static>
    ApplicationInstance<Message, Event, B>
{
    pub(crate) fn new(
        tokio_runtime: tokio::runtime::Runtime,
        global_resources: GlobalResources,
        windows: Vec<WindowUiConfig<Message, Event>>,
        base_color: Color,
        renderer: CoreRenderer,
        backend: Arc<B>,
    ) -> Arc<Self> {
        let app = Arc::new(Self {
            tokio_runtime,
            global_resources,
            windows: tokio::sync::RwLock::new(HashMap::with_hasher(
                fxhash::FxBuildHasher::default(),
            )),
            not_started_uis: tokio::sync::Mutex::new(windows),
            base_color,
            renderer,
            backend,
            benchmarker: tokio::sync::Mutex::new(utils::benchmark::Benchmark::new(120)),
            frame_count: std::sync::atomic::AtomicU64::new(0),
            device_lost_callback_id: parking_lot::Mutex::new(None),
            device_recover_callback_id: parking_lot::Mutex::new(None),
            render_loop_task_handle: tokio::sync::Mutex::new(None),
        });

        // register gpu device lost callback
        {
            app.global_resources.gpu().enable_auto_recover(true);

            let app_weak = Arc::downgrade(&app);
            let device_lost_cbid =
                app.global_resources
                    .gpu()
                    .add_device_lost_callback(move |e, s| {
                        log::warn!("GPU device lost: reason={e:?} info={s}");

                        if let Some(app_clone) = app_weak.upgrade() {
                            // invalidate all caches to avoid using invalid resources
                            for window in app_clone.windows.blocking_read().values() {
                                window.invalidate_widget_render_cache();
                            }
                        }
                    });

            let app_weak = Arc::downgrade(&app);
            let devoce_lost_recover_cbid =
                app.global_resources
                    .gpu()
                    .add_device_recover_callback(move |device, queue| {
                        log::info!("GPU device recovered");

                        if let Some(app) = app_weak.upgrade() {
                            // texture atlas
                            app.global_resources
                                .texture_atlas()
                                .recover(&device, &queue);

                            // stencil atlas
                            app.global_resources
                                .stencil_atlas()
                                .recover(&device, &queue);

                            // gpu resources
                            app.global_resources.gpu_resource().recover(&device, &queue);

                            // core renderer
                            app.renderer.recover(&device, &queue);
                        }
                    });

            *app.device_lost_callback_id.lock() = Some(device_lost_cbid);
            *app.device_recover_callback_id.lock() = Some(devoce_lost_recover_cbid);
        }

        app
    }
}

/// Syncronous winit event handling.
impl<Message: Send + 'static, Event: Send + 'static, B: Backend<Event> + Send + Sync + 'static>
    ApplicationInstance<Message, Event, B>
{
    pub fn start_all_windows(&self, winit_event_loop: &winit::event_loop::ActiveEventLoop) {
        log::trace!("ApplicationInstance::start_all_windows: starting all windows");
        self.tokio_runtime.block_on(async {
            let not_started_uis_guard = &mut *self.not_started_uis.lock().await;
            let not_started_uis = std::mem::take(not_started_uis_guard);
            let windows = &mut *self.windows.write().await;
            log::trace!(
                "ApplicationInstance::start_all_windows: {} windows to start",
                not_started_uis.len()
            );
            for window_config in not_started_uis {
                log::trace!("ApplicationInstance::start_all_windows: starting a window");
                match window_config
                    .start_window(winit_event_loop, self.global_resources.gpu())
                    .await
                {
                    Ok(window) => {
                        let window_id = window.window_id();
                        windows.insert(window_id, window);
                        log::info!(
                            "ApplicationInstance::start_all_windows: window id={window_id:?} started"
                        );
                    }
                    Err((window_config, err)) => {
                        not_started_uis_guard.push(window_config);
                        log::error!(
                            "ApplicationInstance::start_all_windows: failed to start window: {err}"
                        );
                    }
                }
            }
        });
    }

    pub fn call_all_setups(&self) {
        log::trace!("ApplicationInstance::call_all_setups: calling setup on all windows");
        self.tokio_runtime.block_on(async {
            let windows = self.windows.read().await;
            for window in windows.values() {
                log::trace!("ApplicationInstance::call_all_setups: calling setup for one window");
                window
                    .setup(self.tokio_runtime.handle(), &self.global_resources)
                    .await;
            }
        });
    }

    pub fn window_event(
        &self,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        log::trace!("ApplicationInstance::window_event: window_id={window_id:?} event={event:?}");
        self.tokio_runtime.block_on(async {
            let windows = self.windows.read().await;

            let Some(window) = windows.get(&window_id) else {
                log::trace!("ApplicationInstance::window_event: no matching window for id={window_id:?}");
                return;
            };

            log::trace!("ApplicationInstance::window_event: delivering event to window");

            if let winit::event::WindowEvent::Resized(physical_size) = event {
                log::trace!("ApplicationInstance::window_event: resize detected {}x{}", physical_size.width, physical_size.height);
                window
                    .resize_window(physical_size, &self.global_resources.gpu().device())
                    .await;
            }

            let event = window
                .window_event(event, self.tokio_runtime.handle(), &self.global_resources)
                .await;

            if let Some(event) = event {
                log::trace!("ApplicationInstance::window_event: widget produced event, forwarding to backend");
                self.backend.send_event(event).await;
            }
        });
    }

    pub fn poll_mouse_state(&self) {
        log::trace!("ApplicationInstance::poll_mouse_state: polling mouse state");
        self.tokio_runtime.block_on(async {
            let windows = self.windows.read().await;

            for window in windows.values() {
                let events = window
                    .poll_mouse_state(self.tokio_runtime.handle(), &self.global_resources)
                    .await;

                for event in events {
                    self.backend.send_event(event).await;
                }
            }
        });
    }

    pub fn user_event(self: &Arc<Self>, message: Message) {
        log::trace!("ApplicationInstance::user_event: received user event");
        let app_instance = self.clone();
        self.tokio_runtime.spawn(async move {
            let app_instance = app_instance;
            let message = message;
            for window in app_instance.windows.read().await.values() {
                log::trace!("ApplicationInstance::user_event: forwarding to window");
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

    pub fn close_window(&self, window_id: winit::window::WindowId) {
        log::info!("ApplicationInstance::close_window: closing window id={window_id:?}");
        self.tokio_runtime.block_on(async {
            let mut windows = self.windows.write().await;
            if let Some(window) = windows.remove(&window_id) {
                drop(window);
                log::info!("ApplicationInstance::close_window: window id={window_id:?} closed");
            } else {
                log::warn!(
                    "ApplicationInstance::close_window: no window found for id={window_id:?}"
                );
            }
        });
    }
}

/// Async rendering loop.
impl<Message: Send + 'static, Event: Send + 'static, B: Backend<Event> + Send + Sync + 'static>
    ApplicationInstance<Message, Event, B>
{
    pub fn start_rendering_loop(self: &Arc<Self>) -> Option<tokio::sync::oneshot::Sender<()>> {
        let self_instance = self.clone();
        self.tokio_runtime.block_on(async {
            let render_loop_task_handle = &mut *self_instance.render_loop_task_handle.lock().await;
            if render_loop_task_handle.is_none() {
                let (exit_signal_sender, exit_signal_receiver) = tokio::sync::oneshot::channel();

                let self_instance = self_instance.clone();
                let handle = self.tokio_runtime.spawn(async move {
                    self_instance.rendering_loop(exit_signal_receiver).await;
                });

                *render_loop_task_handle = Some(handle);

                Some(exit_signal_sender)
            } else {
                None
            }
        })
    }

    pub async fn rendering_loop(
        self: Arc<Self>,
        mut exit_signal: tokio::sync::oneshot::Receiver<()>,
    ) {
        loop {
            // receive exit signal.
            match exit_signal.try_recv() {
                Ok(_) => {
                    log::info!("ApplicationInstance::rendering_loop: exit signal received, stopping rendering loop");
                    break;
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                    log::error!("ApplicationInstance::rendering_loop: exit signal channel closed, stopping rendering loop");
                    break;
                }
                Err(tokio::sync::oneshot::error::TryRecvError::Empty) => (),
            }

            {
                let windows = self.windows.read().await;
                for window in windows.values() {
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

            self.frame_count
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);

            tokio::task::yield_now().await;
        }

        {
            let mut render_loop_task_handle = self.render_loop_task_handle.lock().await;
            *render_loop_task_handle = None;
        }
    }
}
