use crate::{
    application::{Application, ApplicationCommand, ApplicationControler},
    event::window_event::WindowEvent,
};

pub(crate) struct WinitInterface<BackendMessage: Send + 'static> {
    pub(crate) application: Application<BackendMessage>,
}

impl<BackendMessage: Send + 'static> WinitInterface<BackendMessage> {
    pub fn new(application: Application<BackendMessage>) -> Self {
        Self { application }
    }

    pub fn run(mut self) -> Result<Self, winit::error::EventLoopError> {
        todo!()
    }
}

impl<BackendMessage: Send + 'static>
    winit::application::ApplicationHandler<WinitUserMessage<BackendMessage>>
    for WinitInterface<BackendMessage>
{
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.application.resumed(event_loop);

        self.application.create_window(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            // --------------
            // Redraw request
            // --------------
            winit::event::WindowEvent::RedrawRequested => todo!(),

            // --------------
            // Window lifecycle
            // --------------
            winit::event::WindowEvent::ActivationTokenDone { serial, token } => todo!(),
            winit::event::WindowEvent::CloseRequested => todo!(),
            winit::event::WindowEvent::Destroyed => todo!(),
            winit::event::WindowEvent::Occluded(_) => todo!(),

            // --------------
            // Window state
            // --------------
            winit::event::WindowEvent::Resized(physical_size) => todo!(),
            winit::event::WindowEvent::Moved(physical_position) => todo!(),
            winit::event::WindowEvent::Focused(_) => todo!(),

            // --------------
            // Mouse events
            // --------------
            winit::event::WindowEvent::CursorMoved {
                device_id,
                position,
            } => todo!(),
            winit::event::WindowEvent::CursorEntered { device_id } => todo!(),
            winit::event::WindowEvent::CursorLeft { device_id } => todo!(),
            winit::event::WindowEvent::MouseWheel {
                device_id,
                delta,
                phase,
            } => todo!(),
            winit::event::WindowEvent::MouseInput {
                device_id,
                state,
                button,
            } => todo!(),

            // --------------
            // File drag and drop
            // --------------
            winit::event::WindowEvent::DroppedFile(path_buf) => todo!(),
            winit::event::WindowEvent::HoveredFile(path_buf) => todo!(),
            winit::event::WindowEvent::HoveredFileCancelled => todo!(),

            // --------------
            // Keyboard events
            // --------------
            winit::event::WindowEvent::KeyboardInput {
                device_id,
                event,
                is_synthetic,
            } => todo!(),
            winit::event::WindowEvent::ModifiersChanged(modifiers) => todo!(),
            winit::event::WindowEvent::Ime(ime) => todo!(),

            // --------------
            // Touch events
            // --------------
            winit::event::WindowEvent::Touch(touch) => todo!(),
            winit::event::WindowEvent::PinchGesture {
                device_id,
                delta,
                phase,
            } => todo!(),
            winit::event::WindowEvent::PanGesture {
                device_id,
                delta,
                phase,
            } => todo!(),
            winit::event::WindowEvent::DoubleTapGesture { device_id } => todo!(),
            winit::event::WindowEvent::RotationGesture {
                device_id,
                delta,
                phase,
            } => todo!(),

            // --------------
            // Touchpad events
            // --------------
            winit::event::WindowEvent::TouchpadPressure {
                device_id,
                pressure,
                stage,
            } => todo!(),

            // --------------
            // UI events
            // --------------
            winit::event::WindowEvent::ThemeChanged(theme) => todo!(),
            winit::event::WindowEvent::ScaleFactorChanged {
                scale_factor,
                inner_size_writer,
            } => todo!(),

            // --------------
            // Device events
            // --------------
            winit::event::WindowEvent::AxisMotion {
                device_id,
                axis,
                value,
            } => todo!(),
        }
    }

    fn new_events(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        cause: winit::event::StartCause,
    ) {
        match cause {
            winit::event::StartCause::Init => {
                self.application.init(event_loop);
            }
            winit::event::StartCause::Poll => {
                self.application.poll(event_loop);
            }
            winit::event::StartCause::ResumeTimeReached {
                start,
                requested_resume,
            } => {
                self.application
                    .resume_time_reached(event_loop, start, requested_resume);
            }
            winit::event::StartCause::WaitCancelled {
                start,
                requested_resume,
            } => {
                self.application
                    .wait_cancelled(event_loop, start, requested_resume);
            }
        }
    }

    fn user_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        event: WinitUserMessage<BackendMessage>,
    ) {
        match event {
            WinitUserMessage::UpdateNeeded => {
                self.application.update_needed(event_loop);
            }
            WinitUserMessage::BackendMessage { msg } => {
                self.application.backend_message(event_loop, msg)
            }
            WinitUserMessage::EventLoopCommand { cmd } => {
                self.application.event_loop_commands(cmd);
            }
        }
    }

    fn device_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        let device_id = todo!();
        let event = todo!();

        self.application.device_event(event_loop, device_id, event);
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.application.about_to_wait(event_loop);
    }

    fn suspended(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.application.destroy_window(event_loop);

        self.application.suspended(event_loop);
    }

    fn exiting(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.application.exiting(event_loop);
    }

    fn memory_warning(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.application.memory_warning(event_loop);
    }
}

pub(crate) enum WinitUserMessage<Msg: Send + 'static> {
    UpdateNeeded,
    BackendMessage { msg: Msg },
    EventLoopCommand { cmd: ApplicationCommand },
}

// Adaptor for API of `application.rs`.

impl ApplicationControler for winit::event_loop::ActiveEventLoop {}

impl<BackendMessage: Send + 'static> crate::application::ApplicationLoopProxy<BackendMessage>
    for winit::event_loop::EventLoopProxy<crate::winit_interface::WinitUserMessage<BackendMessage>>
{
}

// mapping `winit::WindowEvent` to `WindowEvent`

impl WindowEvent {}
