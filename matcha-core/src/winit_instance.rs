use std::{fmt::Debug, sync::Arc};

use crate::{
    application_instance::ApplicationInstance,
    backend::Backend,
    context::ApplicationCommand,
    window_surface::{self},
    window_ui::WindowUiError,
};

// MARK: modules

mod builder;

pub(crate) use builder::WinitInstanceBuilder;

// MARK: Winit

pub struct WinitInstance<
    Message: Send + 'static,
    Event: Send + 'static,
    B: Backend<Event> + Send + Sync + 'static,
> {
    application_instance: Arc<ApplicationInstance<Message, Event, B>>,
    render_loop_exit_signal: Option<tokio::sync::oneshot::Sender<()>>,
}

// MARK: render

impl<Message: Send + 'static, Event: Send + 'static, B: Backend<Event> + Send + Sync + 'static>
    WinitInstance<Message, Event, B>
{
    fn handle_commands(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        log::trace!("WinitInstance::handle_commands: draining command queue");
        while let Ok(command) = self.application_instance.try_recv_command() {
            match command {
                ApplicationCommand::Exit => {
                    log::info!("WinitInstance::handle_commands: received quit command");
                    self.render_loop_exit_signal
                        .take()
                        .map(|sender| sender.send(()).ok());
                    event_loop.exit();
                }
                ApplicationCommand::CloseWindow { id } => {
                    log::info!(
                        "WinitInstance::handle_commands: received close window command for window id={id:?}"
                    );
                    self.application_instance.close_window(id);
                }
            }
        }
    }
}

// MARK: Winit Event Loop

// TODO: Use TokioRuntime::spawn() instead of blocking on as much as possible.

// winit event handler
impl<Message: Send + 'static, Event: Send + 'static, B: Backend<Event> + Send + Sync + 'static>
    winit::application::ApplicationHandler<Message> for WinitInstance<Message, Event, B>
{
    // MARK: resumed

    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        // start window
        self.application_instance.start_all_windows(event_loop);

        // call setup function
        self.application_instance.call_all_setups();

        // start rendering loop
        if let Some(render_loop_exit_signal) = self.application_instance.start_rendering_loop(){
            self.render_loop_exit_signal = Some(render_loop_exit_signal);
        }
    }

    // MARK: window_event

    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        self.application_instance.window_event(window_id, event);
    }

    // MARK: new_events

    fn new_events(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _cause: winit::event::StartCause,
    ) {
        // handle some device event which needs continuous polling to detect (e.g. long press)

        self.application_instance.poll_mouse_state();

        // handle winit instance commands
        self.handle_commands(event_loop);
    }

    // MARK: user_event

    fn user_event(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop, event: Message) {
        self.application_instance.user_event(event);
    }

    // MARK: other

    fn device_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        log::trace!("WinitInstance::device_event: device_id={device_id:?} event={event:?}",);
        let _ = (event_loop, device_id, event);
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        log::trace!("WinitInstance::about_to_wait");
        let _ = event_loop;
    }

    fn suspended(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        log::trace!("WinitInstance::suspended");
        let _ = event_loop;
    }

    fn exiting(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        log::debug!("WinitInstance::exiting");
        let _ = event_loop;
    }

    fn memory_warning(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        log::trace!("WinitInstance::memory_warning");
        let _ = event_loop;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("Failed to initialize tokio runtime")]
    TokioRuntime,
    #[error("Failed to initialize GPU")]
    Gpu,
    #[error(transparent)]
    WindowUi(#[from] WindowUiError),
    #[error(transparent)]
    WindowSurface(#[from] window_surface::WindowSurfaceError),
}
