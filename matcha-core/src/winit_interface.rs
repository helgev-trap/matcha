use crate::{
    adapter::{Adapter, ApplicationCommand, EventLoop},
    application::Application,
    event::device_event::{
        ElementState, KeyInput, KeyboardState, MouseInput, MouseLogicalButton, MousePrimaryButton,
    },
    window::WindowId,
};

// ---------------------------------------------------------------------------
// WinitInterface
// ---------------------------------------------------------------------------

pub(crate) struct WinitInterface<App: Application> {
    pub(crate) adapter: Adapter<App>,
}

impl<App: Application> WinitInterface<App> {
    pub fn new(adapter: Adapter<App>) -> Self {
        Self { adapter }
    }
}

// ---------------------------------------------------------------------------
// winit::application::ApplicationHandler impl
// ---------------------------------------------------------------------------

impl<App: Application> winit::application::ApplicationHandler<WinitUserMessage<App::Msg>>
    for WinitInterface<App>
{
    fn new_events(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        cause: winit::event::StartCause,
    ) {
        match cause {
            winit::event::StartCause::Init => {
                self.adapter.init(event_loop);
            }
            winit::event::StartCause::Poll => {
                self.adapter.poll(event_loop);
            }
            winit::event::StartCause::ResumeTimeReached {
                start,
                requested_resume,
            } => {
                self.adapter
                    .resume_time_reached(event_loop, start, requested_resume);
            }
            winit::event::StartCause::WaitCancelled {
                start,
                requested_resume,
            } => {
                self.adapter
                    .wait_cancelled(event_loop, start, requested_resume);
            }
        }
    }

    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.adapter.resumed(event_loop);
        self.adapter.create_window(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        winit_window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let window_id = WindowId::from(winit_window_id);

        match event {
            // --------------
            // Redraw request
            // --------------
            winit::event::WindowEvent::RedrawRequested => {
                self.adapter.render(event_loop, window_id);
            }

            // --------------
            // Window lifecycle
            // --------------
            winit::event::WindowEvent::CloseRequested => {
                let e = crate::event::window_event::WindowEvent::stateless(
                    crate::event::window_event::WindowEventData::CloseRequested,
                );
                self.adapter.window_event(event_loop, window_id, e);
            }

            winit::event::WindowEvent::Destroyed => {
                self.adapter.window_destroyed(event_loop, window_id);
            }

            winit::event::WindowEvent::Occluded(occluded) => {
                let e = crate::event::window_event::WindowEvent::stateless(
                    crate::event::window_event::WindowEventData::Occluded(occluded),
                );
                self.adapter.window_event(event_loop, window_id, e);
            }

            winit::event::WindowEvent::ActivationTokenDone { .. } => {
                // Not mapped to a custom event yet.
            }

            // --------------
            // Window state
            // --------------
            winit::event::WindowEvent::Resized(physical_size) => {
                let inner = [physical_size.width as f32, physical_size.height as f32];
                let e = crate::event::window_event::WindowEvent::stateless(
                    crate::event::window_event::WindowEventData::Resized {
                        inner_size: inner,
                        outer_size: inner,
                    },
                );
                self.adapter.window_event(event_loop, window_id, e);
            }

            winit::event::WindowEvent::Moved(physical_position) => {
                let pos = [physical_position.x as f32, physical_position.y as f32];
                let e = crate::event::window_event::WindowEvent::stateless(
                    crate::event::window_event::WindowEventData::Moved {
                        inner_position: pos,
                        outer_position: pos,
                    },
                );
                self.adapter.window_event(event_loop, window_id, e);
            }

            winit::event::WindowEvent::Focused(focused) => {
                let e = crate::event::window_event::WindowEvent::stateless(
                    crate::event::window_event::WindowEventData::Focus(focused),
                );
                self.adapter.window_event(event_loop, window_id, e);
            }

            // --------------
            // Mouse events
            // --------------
            winit::event::WindowEvent::CursorMoved { position, .. } => {
                let pos = [position.x as f32, position.y as f32];
                let e = crate::event::device_event::DeviceEvent::stateless(
                    crate::event::device_event::DeviceEventData::MouseInput {
                        dragging_from_primary: None,
                        dragging_from_secondary: None,
                        dragging_from_middle: None,
                        event: Some(crate::event::device_event::MouseInput::Moved { position: pos }),
                    },
                );
                self.adapter.device_event(event_loop, window_id, e);
            }

            winit::event::WindowEvent::CursorEntered { .. } => {
                let e = crate::event::device_event::DeviceEvent::stateless(
                    crate::event::device_event::DeviceEventData::MouseInput {
                        dragging_from_primary: None,
                        dragging_from_secondary: None,
                        dragging_from_middle: None,
                        event: Some(crate::event::device_event::MouseInput::Entered),
                    },
                );
                self.adapter.device_event(event_loop, window_id, e);
            }

            winit::event::WindowEvent::CursorLeft { .. } => {
                let e = crate::event::device_event::DeviceEvent::stateless(
                    crate::event::device_event::DeviceEventData::MouseInput {
                        dragging_from_primary: None,
                        dragging_from_secondary: None,
                        dragging_from_middle: None,
                        event: Some(crate::event::device_event::MouseInput::Left),
                    },
                );
                self.adapter.device_event(event_loop, window_id, e);
            }

            winit::event::WindowEvent::MouseWheel { delta, .. } => {
                let converted_delta = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        crate::event::device_event::mouse_input::ScrollDelta::LineDelta(x, y)
                    }
                    winit::event::MouseScrollDelta::PixelDelta(pos) => {
                        crate::event::device_event::mouse_input::ScrollDelta::PixelDelta([pos.x as f32, pos.y as f32])
                    }
                };
                let e = crate::event::device_event::DeviceEvent::stateless(
                    crate::event::device_event::DeviceEventData::MouseInput {
                        dragging_from_primary: None,
                        dragging_from_secondary: None,
                        dragging_from_middle: None,
                        event: Some(crate::event::device_event::MouseInput::ScrollRaw { delta: converted_delta }),
                    },
                );
                self.adapter.device_event(event_loop, window_id, e);
            }

            winit::event::WindowEvent::MouseInput { button, state, .. } => {
                if let Some(custom_button) = map_mouse_button(button) {
                    let custom_state = match state {
                        winit::event::ElementState::Pressed => ElementState::Pressed(0),
                        winit::event::ElementState::Released => ElementState::Released(0),
                    };
                    let e = crate::event::device_event::DeviceEvent::stateless(
                        crate::event::device_event::DeviceEventData::MouseInput {
                            dragging_from_primary: None,
                            dragging_from_secondary: None,
                            dragging_from_middle: None,
                            event: Some(crate::event::device_event::MouseInput::ButtonInput {
                                state: custom_state,
                                button: custom_button,
                            }),
                        },
                    );
                    self.adapter.device_event(event_loop, window_id, e);
                }
            }

            // --------------
            // File drag and drop
            // --------------
            winit::event::WindowEvent::DroppedFile(path_buf) => {
                let e = crate::event::device_event::DeviceEvent::stateless(
                    crate::event::device_event::DeviceEventData::FileDrop { path_buf },
                );
                self.adapter.device_event(event_loop, window_id, e);
            }

            winit::event::WindowEvent::HoveredFile(path_buf) => {
                let e = crate::event::device_event::DeviceEvent::stateless(
                    crate::event::device_event::DeviceEventData::FileHover { path_buf },
                );
                self.adapter.device_event(event_loop, window_id, e);
            }

            winit::event::WindowEvent::HoveredFileCancelled => {
                let e = crate::event::device_event::DeviceEvent::stateless(
                    crate::event::device_event::DeviceEventData::FileHoverCancelled,
                );
                self.adapter.device_event(event_loop, window_id, e);
            }

            // --------------
            // Keyboard events
            // --------------
            winit::event::WindowEvent::KeyboardInput { event, .. } => {
                let key_input = map_key_input(event);
                let e = crate::event::device_event::DeviceEvent::stateless(
                    crate::event::device_event::DeviceEventData::Keyboard(key_input),
                );
                self.adapter.device_event(event_loop, window_id, e);
            }

            winit::event::WindowEvent::ModifiersChanged(modifiers) => {
                let e = crate::event::device_event::DeviceEvent::stateless(
                    crate::event::device_event::DeviceEventData::ModifiersChanged(modifiers.state()),
                );
                self.adapter.device_event(event_loop, window_id, e);
            }

            winit::event::WindowEvent::Ime(_) => {
                // Not mapped yet.
            }

            // --------------
            // UI events
            // --------------
            winit::event::WindowEvent::ThemeChanged(theme) => {
                let custom_theme = crate::window::window_config::Theme::from(theme);
                let e = crate::event::window_event::WindowEvent::stateless(
                    crate::event::window_event::WindowEventData::Theme(custom_theme),
                );
                self.adapter.window_event(event_loop, window_id, e);
            }

            winit::event::WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let e = crate::event::window_event::WindowEvent::stateless(
                    crate::event::window_event::WindowEventData::ScaleFactorChanged { scale_factor },
                );
                self.adapter.window_event(event_loop, window_id, e);
            }

            // --------------
            // Touch / gesture / touchpad / axis — not mapped yet
            // --------------
            winit::event::WindowEvent::Touch(_)
            | winit::event::WindowEvent::PinchGesture { .. }
            | winit::event::WindowEvent::PanGesture { .. }
            | winit::event::WindowEvent::DoubleTapGesture { .. }
            | winit::event::WindowEvent::RotationGesture { .. }
            | winit::event::WindowEvent::TouchpadPressure { .. }
            | winit::event::WindowEvent::AxisMotion { .. } => {
                // Not mapped yet.
            }
        }
    }

    fn user_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        event: WinitUserMessage<App::Msg>,
    ) {
        match event {
            WinitUserMessage::BackendMessage { msg } => {
                self.adapter.backend_message(event_loop, msg);
            }
            WinitUserMessage::EventLoopCommand { cmd } => {
                self.adapter.event_loop_commands(cmd);
            }
            WinitUserMessage::BufferUpdated => {
                self.adapter.buffer_updated(event_loop);
            }
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        _event: winit::event::DeviceEvent,
    ) {
        // Raw device events are not yet mapped to custom types.
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.adapter.about_to_wait(event_loop);
    }

    fn suspended(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.adapter.destroy_window(event_loop);
        self.adapter.suspended(event_loop);
    }

    fn exiting(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.adapter.exiting(event_loop);
    }

    fn memory_warning(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.adapter.memory_warning(event_loop);
    }
}

// ---------------------------------------------------------------------------
// User message type
// ---------------------------------------------------------------------------

pub(crate) enum WinitUserMessage<Msg: Send + 'static> {
    BufferUpdated,
    BackendMessage { msg: Msg },
    EventLoopCommand { cmd: ApplicationCommand },
}

// ---------------------------------------------------------------------------
// EventLoop / WindowControler impls for winit types
// ---------------------------------------------------------------------------

impl EventLoop for winit::event_loop::ActiveEventLoop {}

impl crate::adapter::EventLoopProxy for winit::event_loop::EventLoopProxy<()> {}

// ---------------------------------------------------------------------------
// run_on_winit entry point
// ---------------------------------------------------------------------------

pub(crate) fn run_on_winit<App: Application>(
    adapter: Adapter<App>,
) -> Result<(), winit::error::EventLoopError> {
    let event_loop =
        winit::event_loop::EventLoop::<WinitUserMessage<App::Msg>>::with_user_event().build()?;

    // Spawn the bridge thread that forwards SharedValue change signals to the
    // event loop as `BufferUpdated` messages.  This is what makes
    // `SharedValue::store()` → redraw work.
    let proxy = event_loop.create_proxy();
    spawn_bridge_thread(proxy);

    let mut interface = WinitInterface::new(adapter);
    event_loop.run_app(&mut interface)
}

/// Spawns a background thread that watches for [`shared_buffer::SharedValue`]
/// change signals and forwards them to the winit event loop.
///
/// The bridge thread blocks on [`BufferContext::wait_for_signal`] and sends a
/// `BufferUpdated` user event for every signal, waking the event loop from
/// `ControlFlow::Wait`.  The thread exits automatically when all senders are
/// dropped (i.e. when the event loop shuts down).
fn spawn_bridge_thread<Msg: Send + 'static>(
    proxy: winit::event_loop::EventLoopProxy<WinitUserMessage<Msg>>,
) {
    shared_buffer::BufferContext::init_global();
    let ctx = shared_buffer::BufferContext::global().clone();

    std::thread::Builder::new()
        .name("matcha-buffer-bridge".into())
        .spawn(move || {
            while ctx.wait_for_signal() {
                // If the event loop is gone the send fails; we exit the loop.
                if proxy.send_event(WinitUserMessage::BufferUpdated).is_err() {
                    break;
                }
            }
        })
        .expect("failed to spawn buffer bridge thread");
}

// ---------------------------------------------------------------------------
// Mapping helpers (winit → custom types)
// ---------------------------------------------------------------------------

fn map_mouse_button(
    button: winit::event::MouseButton,
) -> Option<crate::event::device_event::mouse_input::PhysicalMouseButton> {
    use crate::event::device_event::mouse_input::PhysicalMouseButton as P;
    use winit::event::MouseButton as W;
    match button {
        W::Left => Some(P::Left),
        W::Right => Some(P::Right),
        W::Middle => Some(P::Middle),
        W::Back => Some(P::Back),
        W::Forward => Some(P::Forward),
        W::Other(b) => Some(P::Other(b)),
    }
}

fn map_key_input(key_event: winit::event::KeyEvent) -> KeyInput {
    let state = match key_event.state {
        winit::event::ElementState::Pressed => ElementState::Pressed(0),
        winit::event::ElementState::Released => ElementState::Released(0),
    };
    KeyInput {
        physical_key: key_event.physical_key,
        logical_key: key_event.logical_key,
        text: key_event.text.map(|s| s.to_string()),
        location: key_event.location,
        state,
        repeat: key_event.repeat,
        snapshot: KeyboardState::default(),
    }
}
