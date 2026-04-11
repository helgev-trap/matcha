use crate::ui::component::AnyComponent;
use log::{debug, trace};

use super::{
    backend::Backend, color::Color, device_input::mouse_state::MousePrimaryButton,
    ui::component::Component, winit_instance::WinitInstanceBuilder,
};
use std::{num::NonZeroUsize, time::Duration};

/// Top-level application builder.
/// Generics:
/// - Model: application model stored inside `Component` (must be Send+Sync)
/// - Message: external message type sent to the app
/// - B: backend type implementing `Backend<Event>`
/// - Event: event type produced by UI
/// - InnerEvent: event type used internally by Component's view
pub struct App<Model, Message, B, Event, InnerEvent>
where
    Model: Send + Sync + 'static,
    Message: Send + Sync + 'static,
    Event: Send + 'static,
    B: Backend<Event> + Send + Sync + 'static,
{
    builder: WinitInstanceBuilder<Message, Event, B>,
    // Phantom markers for unused type parameters so the struct keeps them in its
    // type signature and avoids "type parameter is never used" errors.
    _model: std::marker::PhantomData<Model>,
    _inner_event: std::marker::PhantomData<InnerEvent>,
}

impl<Message, Event> App<(), Message, (), Event, ()>
where
    Message: Send + Sync + 'static,
    Event: Send + 'static,
{
    /// Convenience constructor for apps that don't use a typed `Model` / `InnerEvent`
    /// and use the unit backend `()`.
    pub fn new(component: impl AnyComponent<Message, Event> + 'static) -> Self {
        trace!("App::new: creating default app instance");
        Self {
            builder: WinitInstanceBuilder::new(component, ()),
            _model: std::marker::PhantomData,
            _inner_event: std::marker::PhantomData,
        }
    }
}

impl<Model, Message, B, Event, InnerEvent> App<Model, Message, B, Event, InnerEvent>
where
    Model: Send + Sync + 'static,
    Message: Send + Sync + 'static,
    Event: std::fmt::Debug + Send + 'static,
    B: Backend<Event> + Clone + Send + Sync + 'static,
{
    pub fn with_backend<NewMessage, NewB>(
        self,
        component: Component<Model, NewMessage, Event, InnerEvent>,
        backend: NewB,
    ) -> App<Model, NewMessage, NewB, Event, InnerEvent>
    where
        NewMessage: Send + Sync + 'static,
        NewB: Backend<Event> + Clone + Send + Sync + 'static,
    {
        debug!("App::with_backend: swapping backend for new type");
        let mut new_builder = WinitInstanceBuilder::new(component, backend);
        // carry over settings
        new_builder.runtime_builder = self.builder.runtime_builder;
        new_builder.title = self.builder.title;
        new_builder.init_size = self.builder.init_size;
        new_builder.maximized = self.builder.maximized;
        new_builder.full_screen = self.builder.full_screen;
        new_builder.power_preference = self.builder.power_preference;
        new_builder.base_color = self.builder.base_color;
        new_builder.surface_preferred_format = self.builder.surface_preferred_format;
        new_builder.double_click_threshold = self.builder.double_click_threshold;
        new_builder.long_press_threshold = self.builder.long_press_threshold;
        new_builder.mouse_primary_button = self.builder.mouse_primary_button;
        new_builder.scroll_pixel_per_line = self.builder.scroll_pixel_per_line;
        new_builder.default_font_size = self.builder.default_font_size;
        new_builder.debug_config = self.builder.debug_config;

        App {
            builder: new_builder,
            _model: std::marker::PhantomData,
            _inner_event: std::marker::PhantomData,
        }
    }

    pub fn tokio_runtime(mut self, runtime: tokio::runtime::Runtime) -> Self {
        self.builder = self.builder.tokio_runtime(runtime);
        self
    }

    pub fn worker_threads(mut self, threads: NonZeroUsize) -> Self {
        self.builder = self.builder.worker_threads(threads);
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.builder = self.builder.title(title);
        self
    }

    pub fn init_size(mut self, width: u32, height: u32) -> Self {
        self.builder = self.builder.init_size(width, height);
        self
    }

    pub fn maximized(mut self, maximized: bool) -> Self {
        self.builder = self.builder.maximized(maximized);
        self
    }

    pub fn full_screen(mut self, full_screen: bool) -> Self {
        self.builder = self.builder.full_screen(full_screen);
        self
    }

    pub fn power_preference(mut self, preference: wgpu::PowerPreference) -> Self {
        self.builder = self.builder.power_preference(preference);
        self
    }

    pub fn base_color(mut self, color: Color) -> Self {
        self.builder = self.builder.base_color(color);
        self
    }

    pub fn surface_preferred_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.builder = self.builder.surface_preferred_format(format);
        self
    }

    pub fn double_click_threshold(mut self, duration: Duration) -> Self {
        self.builder = self.builder.double_click_threshold(duration);
        self
    }

    pub fn long_press_threshold(mut self, duration: Duration) -> Self {
        self.builder = self.builder.long_press_threshold(duration);
        self
    }

    pub fn mouse_primary_button(mut self, button: MousePrimaryButton) -> Self {
        self.builder = self.builder.mouse_primary_button(button);
        self
    }

    pub fn scroll_pixel_per_line(mut self, pixel: f32) -> Self {
        self.builder = self.builder.scroll_pixel_per_line(pixel);
        self
    }

    pub fn default_font_size(mut self, size: f32) -> Self {
        self.builder = self.builder.default_font_size(size);
        self
    }

    /// Inject a shared DebugConfig instance.
    pub(crate) fn debug_config(mut self, cfg: crate::debug_config::DebugConfig) -> Self {
        self.builder = self.builder.debug_config(cfg);
        self
    }

    /// Convenience wrapper to toggle measure cache disabling.
    pub fn disable_layout_measure_cache(mut self, v: bool) -> Self {
        self.builder = self.builder.disable_layout_measure_cache(v);
        self
    }

    /// Convenience wrapper to toggle arrange cache disabling.
    pub fn disable_layout_arrange_cache(mut self, v: bool) -> Self {
        self.builder = self.builder.disable_layout_arrange_cache(v);
        self
    }

    /// Convenience wrapper to toggle rendernode cache disabling.
    pub fn disable_rendernode_cache(mut self, v: bool) -> Self {
        self.builder = self.builder.disable_rendernode_cache(v);
        self
    }

    /// Convenience wrapper to toggle widget-level cache disabling.
    pub fn always_rebuild_widget(mut self, v: bool) -> Self {
        self.builder = self.builder.always_rebuild_widget(v);
        self
    }

    pub fn run(self) -> Result<(), AppRunError> {
        debug!("App::run: building WinitInstance");
        let mut winit_app = self.builder.build()?;
        let event_loop = winit::event_loop::EventLoop::<Message>::with_user_event().build()?;
        trace!("App::run: starting event loop");
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
        event_loop.run_app(&mut winit_app)?;
        trace!("App::run: event loop exited");
        Ok(())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum AppRunError {
    #[error("Failed to initialize WinitInstance")]
    InitError(#[from] crate::winit_instance::InitError),
    #[error("With in winit event loop: {0}")]
    WinitEventLoopError(#[from] winit::error::EventLoopError),
}
