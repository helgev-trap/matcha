use std::{collections::HashMap, sync::Arc};

use crate::{
    application::Application,
    event::{
        EventStateConfig,
        device_event::{DeviceEvent, DeviceEventState},
        raw_device_event::{RawDeviceEvent, RawDeviceId},
        window_event::{WindowEvent, WindowEventState},
    },
    window::WindowId,
};

// ---------------------------------------------------------------------------
// Per-window state machines
// ---------------------------------------------------------------------------

pub(crate) struct PerWindowState {
    pub device: DeviceEventState,
    pub window: WindowEventState,
}

impl PerWindowState {
    fn new(config: &EventStateConfig) -> Self {
        Self {
            device: DeviceEventState::new(config.mouse)
                .expect("EventStateConfig passed to PerWindowState::new must be valid"),
            window: WindowEventState::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Adapter
// ---------------------------------------------------------------------------

pub struct Adapter<App: Application> {
    tokio_runtime: tokio::runtime::Runtime,

    rendering_window: HashMap<WindowId, tokio::task::JoinHandle<()>>,

    /// Per-window event state machines, keyed by WindowId.
    /// Created lazily on the first event for a window;
    /// removed when `WindowEvent::Destroyed` is received.
    window_states: HashMap<WindowId, PerWindowState>,

    /// Configuration applied to every new per-window state machine.
    event_config: EventStateConfig,

    app: Arc<App>,
}

/// Construction and running
impl<App: Application> Adapter<App> {
    /// Run the application on the winit event loop.
    #[cfg(feature = "winit")]
    pub fn run_on_winit(self) -> Result<(), winit::error::EventLoopError> {
        crate::winit_interface::run_on_winit(self)
    }

    pub fn new(app: App) -> Self {
        Self::with_tokio_runtime(app, tokio::runtime::Runtime::new().unwrap())
    }

    pub fn with_event_config(app: App, event_config: EventStateConfig) -> Self {
        Self::with_tokio_runtime_and_event_config(
            app,
            tokio::runtime::Runtime::new().unwrap(),
            event_config,
        )
    }

    pub fn with_tokio_runtime(app: App, runtime: tokio::runtime::Runtime) -> Self {
        Self::with_tokio_runtime_and_event_config(app, runtime, EventStateConfig::default())
    }

    pub fn with_tokio_runtime_and_event_config(
        app: App,
        runtime: tokio::runtime::Runtime,
        event_config: EventStateConfig,
    ) -> Self {
        Self {
            tokio_runtime: runtime,
            rendering_window: HashMap::new(),
            window_states: HashMap::new(),
            event_config,
            app: Arc::new(app),
        }
    }
}

/// Lifecycle events
impl<App: Application> Adapter<App> {
    pub fn init(&mut self, event_loop: &impl EventLoop) {
        let _guard = self.tokio_runtime.enter();
        self.app.init(self.tokio_runtime.handle(), event_loop);
    }

    pub fn resumed(&mut self, event_loop: &impl EventLoop) {
        let _guard = self.tokio_runtime.enter();
        self.app.resumed(self.tokio_runtime.handle(), event_loop);
    }

    pub fn create_surface(&mut self, event_loop: &impl EventLoop) {
        let _guard = self.tokio_runtime.enter();
        self.app
            .create_surface(self.tokio_runtime.handle(), event_loop);
    }

    pub fn destroy_surface(&mut self, event_loop: &impl EventLoop) {
        // ensure all rendering tasks are finished
        self.abort_all_rendering_tasks();

        let _guard = self.tokio_runtime.enter();
        self.app
            .destroy_surface(self.tokio_runtime.handle(), event_loop);
    }

    pub fn suspended(&mut self, event_loop: &impl EventLoop) {
        let _guard = self.tokio_runtime.enter();
        self.app.suspended(self.tokio_runtime.handle(), event_loop);
    }

    pub fn exiting(&mut self, event_loop: &impl EventLoop) {
        let _guard = self.tokio_runtime.enter();
        self.app.exiting(self.tokio_runtime.handle(), event_loop);
    }
}

/// Events
impl<App: Application> Adapter<App> {
    pub fn render(&mut self, event_loop: &impl EventLoop, window_id: WindowId) {
        if let Some(handle) = self.rendering_window.get(&window_id) {
            if handle.is_finished() {
                self.rendering_window.remove(&window_id);
            } else {
                return;
            }
        }

        let app = self.app.clone();

        let handle = self.tokio_runtime.spawn(async move {
            app.render(window_id).await;
        });

        self.rendering_window.insert(window_id, handle);
    }

    pub fn window_event(
        &mut self,
        event_loop: &impl EventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        let _guard = self.tokio_runtime.enter();
        let event = self.window_state_mut(window_id).window.process(event);
        self.app
            .window_event(self.tokio_runtime.handle(), event_loop, window_id, event);
    }

    pub fn window_destroyed(&mut self, event_loop: &impl EventLoop, window_id: WindowId) {
        // Clean up the per-window state machine so it doesn't outlive the window.
        self.remove_window_state(window_id);
        // Clean up the rendering task for the window.
        self.remove_rendering_task(window_id);
        // Notify the Application that the window is gone.
        let _guard = self.tokio_runtime.enter();
        self.app
            .window_destroyed(self.tokio_runtime.handle(), event_loop, window_id);
    }

    pub fn device_event(
        &mut self,
        event_loop: &impl EventLoop,
        window_id: WindowId,
        event: DeviceEvent,
    ) {
        if let Some(processed) = self.window_state_mut(window_id).device.process(event) {
            let _guard = self.tokio_runtime.enter();
            self.app.device_event(
                self.tokio_runtime.handle(),
                event_loop,
                window_id,
                processed,
            );
        }
    }

    pub fn raw_device_event(
        &mut self,
        event_loop: &impl EventLoop,
        raw_device_id: RawDeviceId,
        raw_event: RawDeviceEvent,
    ) {
        let _guard = self.tokio_runtime.enter();
        self.app.raw_device_event(
            self.tokio_runtime.handle(),
            event_loop,
            raw_device_id,
            raw_event,
        );
    }
}

/// User event
impl<App: Application> Adapter<App> {
    /// Called when a `BufferUpdated` event is received from the bridge thread.
    pub fn buffer_updated(&mut self, event_loop: &impl EventLoop) {
        let _guard = self.tokio_runtime.enter();
        self.app
            .buffer_updated(self.tokio_runtime.handle(), event_loop);
    }

    pub fn backend_message(&mut self, event_loop: &impl EventLoop, msg: App::Msg) {
        let _guard = self.tokio_runtime.enter();
        self.app
            .backend_message(self.tokio_runtime.handle(), event_loop, msg);
    }
}

/// Event Loop Commands
impl<App: Application> Adapter<App> {
    pub fn event_loop_commands(&self, _cmd: ApplicationCommand) {
        todo!()
    }
}

/// Polling
impl<App: Application> Adapter<App> {
    pub fn poll(&mut self, event_loop: &impl EventLoop) {
        let _guard = self.tokio_runtime.enter();
        self.app.poll(self.tokio_runtime.handle(), event_loop);
    }

    pub fn resume_time_reached(
        &mut self,
        event_loop: &impl EventLoop,
        start: std::time::Instant,
        requested_resume: std::time::Instant,
    ) {
        let _guard = self.tokio_runtime.enter();
        self.app.resume_time_reached(
            self.tokio_runtime.handle(),
            event_loop,
            start,
            requested_resume,
        );
    }

    pub fn wait_cancelled(
        &mut self,
        event_loop: &impl EventLoop,
        start: std::time::Instant,
        requested_resume: Option<std::time::Instant>,
    ) {
        let _guard = self.tokio_runtime.enter();
        self.app.wait_cancelled(
            self.tokio_runtime.handle(),
            event_loop,
            start,
            requested_resume,
        );
    }

    pub fn about_to_wait(&self, event_loop: &impl EventLoop) {
        let _guard = self.tokio_runtime.enter();
        self.app
            .about_to_wait(self.tokio_runtime.handle(), event_loop);
    }
}

impl<App: Application> Adapter<App> {
    pub fn memory_warning(&mut self, event_loop: &impl EventLoop) {
        let _guard = self.tokio_runtime.enter();
        self.app
            .memory_warning(self.tokio_runtime.handle(), event_loop);
    }
}

// -------------------
// Helpers
// -------------------

impl<App: Application> Adapter<App> {
    fn abort_all_rendering_tasks(&mut self) {
        self.tokio_runtime.block_on(async {
            for handle in self.rendering_window.values() {
                handle.abort();
            }
            for (_, handle) in self.rendering_window.drain() {
                let _ = handle.await;
            }
        });
    }

    fn remove_rendering_task(&mut self, window_id: WindowId) {
        if let Some(handle) = self.rendering_window.get(&window_id) {
            handle.abort();
            self.rendering_window.remove(&window_id);
        }
    }
}

/// Per-window state machine access
impl<App: Application> Adapter<App> {
    /// Returns a mutable reference to the state machine for `id`,
    /// creating it with the stored `event_config` if it doesn't exist yet.
    fn window_state_mut(&mut self, id: WindowId) -> &mut PerWindowState {
        let config = self.event_config; // EventStateConfig is Copy
        self.window_states
            .entry(id)
            .or_insert_with(|| PerWindowState::new(&config))
    }

    /// Removes the state machine for `id`.
    /// Called when winit fires `WindowEvent::Destroyed`.
    fn remove_window_state(&mut self, id: WindowId) {
        self.window_states.remove(&id);
    }
}

// -------------------
// API type definition
// -------------------

pub trait EventLoop: crate::window::WindowControler {}

pub enum ApplicationCommand {
    Exit,
}

pub trait EventLoopProxy {}
