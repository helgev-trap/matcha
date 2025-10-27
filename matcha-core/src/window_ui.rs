use core::panic;
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

use gpu_utils::gpu::Gpu;
use log::{debug, trace, warn};
use parking_lot::RwLock;
use renderer::{RenderNode, core_renderer};
use tokio::task;
use utils::{back_prop_dirty::BackPropDirty, update_flag::UpdateFlag};
use winit::dpi::{PhysicalPosition, PhysicalSize};

use crate::{
    context::GlobalResources,
    device_input::{
        DeviceInput, DeviceInputData, KeyboardState, MouseState,
        mouse_state::{MousePrimaryButton, MouseStateConfig},
        window_state::WindowState,
    },
    metrics::Constraints,
    ui::{AnyWidgetFrame, Background, component::AnyComponent},
    window_surface::WindowSurface,
};

pub struct WindowUi<Message: 'static, Event: 'static> {
    // window
    window: Arc<RwLock<WindowSurface>>,

    surface_guard: SurfaceLock,

    // ui
    component: Box<dyn AnyComponent<Message, Event>>,
    widget: tokio::sync::Mutex<Option<Box<dyn AnyWidgetFrame<Event>>>>,
    model_update_detector: tokio::sync::Mutex<UpdateFlag>,

    // input handling
    window_state: tokio::sync::Mutex<WindowState>,
    mouse_state_config: MouseStateConfig,
    mouse_state: tokio::sync::Mutex<MouseState>,
    keyboard_state: tokio::sync::Mutex<KeyboardState>,
}

struct SurfaceLock {
    state: AtomicU8,
}

impl SurfaceLock {
    const STATE_IDLE: u8 = 0;
    const STATE_RENDERING: u8 = 1;
    const STATE_CONFIGURING: u8 = 2;

    fn new() -> Self {
        Self {
            state: AtomicU8::new(Self::STATE_IDLE),
        }
    }

    async fn lock_for_render(&self) -> SurfaceLockGuard<'_> {
        self.lock_with_state(Self::STATE_RENDERING).await
    }

    async fn lock_for_configure(&self) -> SurfaceLockGuard<'_> {
        self.lock_with_state(Self::STATE_CONFIGURING).await
    }

    async fn lock_with_state(&self, state: u8) -> SurfaceLockGuard<'_> {
        loop {
            if self
                .state
                .compare_exchange(Self::STATE_IDLE, state, Ordering::AcqRel, Ordering::Relaxed)
                .is_ok()
            {
                return SurfaceLockGuard { lock: self };
            }

            std::hint::spin_loop();
            task::yield_now().await;
        }
    }

    fn release(&self) {
        self.state.store(Self::STATE_IDLE, Ordering::Release);
    }
}

struct SurfaceLockGuard<'a> {
    lock: &'a SurfaceLock,
}

impl Drop for SurfaceLockGuard<'_> {
    fn drop(&mut self) {
        self.lock.release();
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WindowUiError {
    #[error("combo_duration must be less than or equal to long_press_duration")]
    InvalidDuration,
}

impl<Message: 'static, Event: 'static> WindowUi<Message, Event> {
    pub fn new(
        component: Box<dyn AnyComponent<Message, Event>>,
        mouse_state_config: MouseStateConfig,
    ) -> Result<Self, WindowUiError> {
        trace!("WindowUi::new: initializing window UI");
        Ok(Self {
            window: Arc::new(RwLock::new(WindowSurface::new())),
            surface_guard: SurfaceLock::new(),
            component,
            model_update_detector: tokio::sync::Mutex::new(UpdateFlag::new()),
            widget: tokio::sync::Mutex::new(None),
            window_state: tokio::sync::Mutex::new(WindowState::default()),
            mouse_state_config,
            mouse_state: tokio::sync::Mutex::new(
                mouse_state_config
                    .init()
                    .ok_or(WindowUiError::InvalidDuration)?,
            ),
            keyboard_state: tokio::sync::Mutex::new(KeyboardState::new()),
        })
    }

    pub async fn set_mouse_primary_button(&mut self, button: MousePrimaryButton) {
        self.mouse_state.lock().await.set_primary_button(button);
    }

    pub async fn mouse_primary_button(&self) -> MousePrimaryButton {
        self.mouse_state.lock().await.primary_button()
    }

    pub async fn set_scroll_pixel_per_line(&mut self, pixel: f32) {
        self.mouse_state
            .lock()
            .await
            .set_scroll_pixel_per_line(pixel);
    }

    pub async fn scroll_pixel_per_line(&self) -> f32 {
        self.mouse_state.lock().await.scroll_pixel_per_line()
    }

    // Window configuration delegation APIs
    pub fn set_title(&mut self, title: &str) {
        self.window.write().set_title(title);
    }

    pub fn init_size(&mut self, width: u32, height: u32) {
        self.window
            .write()
            .request_inner_size(PhysicalSize::new(width, height));
    }

    pub fn set_maximized(&mut self, maximized: bool) {
        self.window.write().set_maximized(maximized);
    }

    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        self.window.write().set_fullscreen(fullscreen);
    }
}

impl<Message: 'static, Event: 'static> WindowUi<Message, Event> {
    pub fn window_id(&self) -> Option<winit::window::WindowId> {
        self.window.read().window_id()
    }

    pub async fn resize_window(&self, new_size: PhysicalSize<u32>, device: &wgpu::Device) {
        trace!(
            "WindowUi::resize_window: new_size={}x{}",
            new_size.width, new_size.height
        );
        let _surface_guard = self.surface_guard.lock_for_configure().await;
        self.window.write().set_surface_size(new_size, device);
    }

    pub fn request_redraw(&self) {
        trace!("WindowUi::request_redraw called");
        self.window.read().request_redraw();
    }
}

impl<Message: 'static, Event: 'static> WindowUi<Message, Event> {
    pub async fn start_window(
        &self,
        winit_event_loop: &winit::event_loop::ActiveEventLoop,
        gpu: &Gpu,
    ) -> Result<(), crate::window_surface::WindowSurfaceError> {
        trace!("WindowUi::start_window: initializing window surface");
        let _surface_guard = self.surface_guard.lock_for_configure().await;
        self.window.write().start_window(winit_event_loop, gpu)
    }

    // start component setup function
    // TODO: This is provisional implementation. Refactor this after organizing async execution flow.
    pub async fn setup(&self, tokio_handle: &tokio::runtime::Handle, resource: &GlobalResources) {
        trace!("WindowUi::setup: invoking component setup");
        self.component
            .setup(&resource.application_context(tokio_handle, &self.window))
    }

    /// Returns true if a render should be performed.
    /// Render is required when the model update flag or animation update flag is true,
    /// or when the widget is not yet initialized.
    pub async fn needs_render(&self) -> bool {
        self.model_update_detector.lock().await.is_true()
            || self
                .widget
                .lock()
                .await
                .as_ref()
                .is_none_or(|w| w.need_redraw())
    }

    pub async fn render(
        &self,
        tokio_handle: &tokio::runtime::Handle,
        resource: &GlobalResources,
        base_color: &crate::color::Color,
        core_renderer: &core_renderer::CoreRenderer,
        benchmark: &mut utils::benchmark::Benchmark,
    ) {
        trace!("WindowUi::render: begin");

        let _surface_guard = self.surface_guard.lock_for_render().await;

        // get surface texture, format, viewport size
        let (surface_texture, surface_format, viewport_size) = {
            let mut window_guard = self.window.upgradable_read();
            match self.acquire_surface(&mut window_guard, resource) {
                Some(v) => v,
                None => return,
            }
        };

        let surface_texture_view = surface_texture.texture.create_view(&Default::default());

        // placeholder background
        // TODO: use black transparent texture as root background
        let background = Background::new(&surface_texture_view, [0.0, 0.0]);

        let ctx = resource.widget_context(tokio_handle, &self.window);

        // Ensure widget tree is initialized or updated
        self.ensure_widget_ready(benchmark).await;

        // Layout and render
        let render_node = self
            .layout_and_render(viewport_size, background, &ctx, benchmark)
            .await;

        let _ = core_renderer.render(
            &resource.gpu().device(),
            &resource.gpu().queue(),
            surface_format,
            &surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default()),
            viewport_size,
            &render_node,
            base_color.to_wgpu_color(),
            &resource.texture_atlas().texture(),
            &resource.stencil_atlas().texture(),
        );

        surface_texture.present();

        // surface_guard keeps configuration serialized with render duration.
    }

    // Acquire surface/format/viewport with all recovery paths encapsulated
    fn acquire_surface(
        &self,
        window_guard: &mut parking_lot::RwLockUpgradableReadGuard<'_, WindowSurface>,
        resource: &GlobalResources,
    ) -> Option<(wgpu::SurfaceTexture, wgpu::TextureFormat, [f32; 2])> {
        // Ensure window already started; do not create here
        if window_guard.window().is_none() {
            trace!("WindowUi::render: window not started, skipping render");
            return None;
        }

        let viewport_size_physical = window_guard
            .inner_size()
            .expect("we checked window existence");
        let viewport_size = [
            viewport_size_physical.width as f32,
            viewport_size_physical.height as f32,
        ];

        let surface = match window_guard
            .current_texture()
            .expect("we checked window existence")
        {
            Ok(texture) => texture,
            Err(e) => {
                warn!("WindowUi::render: failed to get surface texture: {e:?}");
                match e {
                    wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated => {
                        // reconfigure the surface
                        debug!("WindowUi::render: surface lost, reconfiguring");
                        window_guard.with_upgraded(|w| {
                            w.reconfigure_surface(&resource.gpu().device());
                        });

                        // call rerender event
                        window_guard.request_redraw();
                        return None;
                    }
                    wgpu::SurfaceError::Timeout => {
                        // skip this frame
                        warn!("WindowUi::render: surface timeout, skipping frame");
                        return None;
                    }
                    wgpu::SurfaceError::OutOfMemory => {
                        warn!("WindowUi::render: surface out of memory");
                        panic!("out of memory");
                    }
                    wgpu::SurfaceError::Other => {
                        warn!("WindowUi::render: surface returned unknown error");
                        panic!("unknown error at wgpu surface");
                    }
                }
            }
        };

        let surface_format = window_guard.format().expect("we checked window existence");

        Some((surface, surface_format, viewport_size))
    }

    // Ensure widget tree is built or updated as needed
    async fn ensure_widget_ready(&self, benchmark: &mut utils::benchmark::Benchmark) {
        let mut widget_lock = self.widget.lock().await;
        let mut model_update_detector_lock = self.model_update_detector.lock().await;

        if widget_lock.is_none() {
            // directly build widget tree from dom
            trace!("WindowUi::render: building widget tree");
            let dom = benchmark
                .with_async("create_dom", self.component.view())
                .await;
            let widget =
                widget_lock.insert(benchmark.with("create_widget", || dom.build_widget_tree()));

            // set model update notifier
            *model_update_detector_lock = UpdateFlag::new();
            widget
                .set_model_update_notifier(&model_update_detector_lock.notifier())
                .await;
            // set dirty flags
            widget.update_dirty_flags(BackPropDirty::new(true), BackPropDirty::new(true));
        } else if model_update_detector_lock.is_true() {
            // Widget update is required
            trace!("WindowUi::render: updating widget tree");
            let dom = benchmark
                .with_async("create_dom", self.component.view())
                .await;

            if let Some(widget) = widget_lock.as_mut()
                && benchmark
                    .with_async("update_widget", widget.update_widget_tree(&*dom))
                    .await
                    .is_err()
            {
                widget_lock.take();
            }

            let widget = widget_lock.get_or_insert_with(|| dom.build_widget_tree());

            // set model update notifier
            *model_update_detector_lock = UpdateFlag::new();
            widget
                .set_model_update_notifier(&model_update_detector_lock.notifier())
                .await;
            // set dirty flags
            widget.update_dirty_flags(BackPropDirty::new(true), BackPropDirty::new(true));
        }
    }

    // Layout pass and render node creation
    async fn layout_and_render<'a>(
        &'a self,
        viewport_size: [f32; 2],
        background: Background<'a>,
        ctx: &crate::context::WidgetContext,
        benchmark: &mut utils::benchmark::Benchmark,
    ) -> Arc<RenderNode> {
        let mut widget_lock = self.widget.lock().await;

        let widget = widget_lock.as_mut().expect("widget initialized above");

        let constraints: Constraints =
            Constraints::new([0.0, viewport_size[0]], [0.0, viewport_size[1]]);

        let preferred_size = benchmark.with("layout_measure", || widget.measure(&constraints, ctx));
        let final_size = [
            preferred_size[0].clamp(0.0, viewport_size[0]),
            preferred_size[1].clamp(0.0, viewport_size[1]),
        ];

        benchmark.with("layout_arrange", || widget.arrange(final_size, ctx));
        benchmark.with("widget_render", || widget.render(background, ctx))
    }

    async fn convert_winit_to_window_event(
        &self,
        window_event: winit::event::WindowEvent,
        get_window_size: impl Fn() -> (PhysicalSize<u32>, PhysicalSize<u32>),
        get_window_position: impl Fn() -> (PhysicalPosition<i32>, PhysicalPosition<i32>),
    ) -> Option<DeviceInput> {
        let device_input_data = match &window_event {
            // we don't handle these events here
            winit::event::WindowEvent::ScaleFactorChanged { .. }
            | winit::event::WindowEvent::Occluded(_)
            | winit::event::WindowEvent::ActivationTokenDone { .. }
            | winit::event::WindowEvent::RedrawRequested
            | winit::event::WindowEvent::Destroyed => None,

            // window interactions
            winit::event::WindowEvent::Resized(_) => {
                let (inner_size, outer_size) = get_window_size();
                Some(
                    self.window_state
                        .lock()
                        .await
                        .resized(inner_size.into(), outer_size.into()),
                )
            }
            winit::event::WindowEvent::Moved(_) => {
                let (inner_position, outer_position) = get_window_position();
                Some(
                    self.window_state
                        .lock()
                        .await
                        .moved(inner_position.into(), outer_position.into()),
                )
            }
            winit::event::WindowEvent::CloseRequested => Some(DeviceInputData::CloseRequested),
            winit::event::WindowEvent::Focused(focused) => {
                Some(DeviceInputData::WindowFocus(*focused))
            }
            winit::event::WindowEvent::ThemeChanged(theme) => Some(DeviceInputData::Theme(*theme)),

            // file drop events
            winit::event::WindowEvent::DroppedFile(path_buf) => Some(DeviceInputData::FileDrop {
                path_buf: path_buf.clone(),
            }),
            winit::event::WindowEvent::HoveredFile(path_buf) => Some(DeviceInputData::FileHover {
                path_buf: path_buf.clone(),
            }),
            winit::event::WindowEvent::HoveredFileCancelled => {
                Some(DeviceInputData::FileHoverCancelled)
            }

            // keyboard events
            winit::event::WindowEvent::KeyboardInput { event, .. } => self
                .keyboard_state
                .lock()
                .await
                .keyboard_input(event.clone()),
            winit::event::WindowEvent::ModifiersChanged(modifiers) => {
                self.keyboard_state
                    .lock()
                    .await
                    .modifiers_changed(modifiers.state());
                None
            }
            winit::event::WindowEvent::Ime(_) => Some(DeviceInputData::Ime),

            // mouse events
            winit::event::WindowEvent::CursorMoved { position, .. } => {
                Some(self.mouse_state.lock().await.cursor_moved(*position))
            }
            winit::event::WindowEvent::CursorEntered { .. } => {
                Some(self.mouse_state.lock().await.cursor_entered())
            }
            winit::event::WindowEvent::CursorLeft { .. } => {
                Some(self.mouse_state.lock().await.cursor_left())
            }
            winit::event::WindowEvent::MouseWheel { delta, .. } => {
                Some(self.mouse_state.lock().await.mouse_wheel(*delta))
            }
            winit::event::WindowEvent::MouseInput { state, button, .. } => {
                self.mouse_state.lock().await.mouse_input(*button, *state)
            }

            // touch events
            winit::event::WindowEvent::PinchGesture { .. }
            | winit::event::WindowEvent::PanGesture { .. }
            | winit::event::WindowEvent::DoubleTapGesture { .. }
            | winit::event::WindowEvent::RotationGesture { .. }
            | winit::event::WindowEvent::TouchpadPressure { .. }
            | winit::event::WindowEvent::Touch(..)
            | winit::event::WindowEvent::AxisMotion { .. } => Some(DeviceInputData::Touch),
        };

        if let Some(device_input_data) = device_input_data {
            let mouse_position = self.mouse_state.lock().await.position();
            Some(DeviceInput::new(
                mouse_position,
                device_input_data,
                Some(window_event),
            ))
        } else {
            None
        }
    }

    pub async fn window_event(
        &self,
        window_event: winit::event::WindowEvent,
        tokio_handle: &tokio::runtime::Handle,
        resource: &GlobalResources,
    ) -> Option<Event> {
        // check window existence
        if self.window.read().window().is_none() {
            trace!("WindowUi::window_event: ignoring event before window start");
            return None;
        }

        trace!("WindowUi::window_event: received {window_event:?}");
        let ctx = resource.widget_context(tokio_handle, &self.window);

        let window_clone = self.window.clone();
        let get_window_size = || {
            let window = window_clone.read();
            (
                window.inner_size().expect("we checked window existence"),
                window.outer_size().expect("we checked window existence"),
            )
        };
        let window_clone = self.window.clone();
        let get_window_position = || {
            let window = window_clone.read();
            (
                window
                    .inner_position()
                    .expect("we checked window existence")
                    .expect("window should be there and when Android / Wayland window moving event should not be called"),
                window
                    .outer_position()
                    .expect("we checked window existence")
                    .expect("window should be there and when Android / Wayland window moving event should not be called"),
            )
        };

        let event = self
            .convert_winit_to_window_event(window_event, get_window_size, get_window_position)
            .await;

        if let (Some(widget), Some(event)) = (self.widget.lock().await.as_mut(), event) {
            let result = widget.device_input(&event, &ctx);
            if result.is_some() {
                trace!("WindowUi::window_event: widget produced event");
            }
            result
        } else {
            trace!("WindowUi::window_event: no widget or no device input");
            None
        }
    }

    pub async fn poll_mouse_state(
        &self,
        tokio_handle: &tokio::runtime::Handle,
        resource: &GlobalResources,
    ) -> Vec<Event> {
        if self.window.read().window().is_none() {
            trace!("WindowUi::poll_mouse_state: ignoring before window start");
            return Vec::new();
        }

        let (mouse_events, mouse_position) = {
            let mut mouse_state = self.mouse_state.lock().await;
            (
                mouse_state.long_pressing_detection(),
                mouse_state.position(),
            )
        };

        if mouse_events.is_empty() {
            return Vec::new();
        }

        trace!(
            "WindowUi::poll_mouse_state: detected {} pending mouse event(s)",
            mouse_events.len()
        );

        let ctx = resource.widget_context(tokio_handle, &self.window);
        let mut widget_lock = self.widget.lock().await;
        let Some(widget) = widget_lock.as_mut() else {
            trace!("WindowUi::poll_mouse_state: widget not initialized");
            return Vec::new();
        };

        let mut produced_events = Vec::new();

        for device_input_data in mouse_events {
            let device_input = DeviceInput::new(mouse_position, device_input_data, None);

            if let Some(event) = widget.device_input(&device_input, &ctx) {
                produced_events.push(event);
            }
        }

        produced_events
    }

    pub fn user_event(
        &self,
        user_event: &Message,
        tokio_handle: &tokio::runtime::Handle,
        resource: &GlobalResources,
    ) {
        trace!("WindowUi::user_event: forwarding user event");
        let widget_ctx = &resource.widget_context(tokio_handle, &self.window);

        let app_ctx = widget_ctx.application_context();

        self.component.update(user_event, &app_ctx);
    }
}
