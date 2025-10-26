use std::{sync::Arc, time::Duration};

use log::{debug, trace};

use crate::{debug_config::DebugConfig, ui::component::AnyComponent, window_ui::WindowUi};
use winit::dpi::PhysicalSize;

use crate::{
    backend::Backend,
    color::Color,
    device_input::mouse_state::MousePrimaryButton,
    winit_instance::{InitError, WinitInstance},
};

// --- Constants ---

// gpu
const POWER_PREFERENCE: wgpu::PowerPreference = wgpu::PowerPreference::LowPower;
const BASE_COLOR: Color = Color::TRANSPARENT;
const PREFERRED_SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const STENCIL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

// input
const DOUBLE_CLICK_THRESHOLD: Duration = Duration::from_millis(300);
const LONG_PRESS_THRESHOLD: Duration = Duration::from_millis(500);
const SCROLL_PIXEL_PER_LINE: f32 = 40.0;
const DEFAULT_FONT_SIZE: f32 = 16.0;
const MOUSE_PRIMARY_BUTTON: MousePrimaryButton = MousePrimaryButton::Left;

// --- Builder ---

pub struct WinitInstanceBuilder<Message: Send + 'static, Event: Send + 'static, B: Backend<Event> + Send + Sync + 'static> {
    pub(crate) component: Box<dyn AnyComponent<Message, Event>>,
    pub(crate) backend: B,
    pub(crate) runtime_builder: RuntimeBuilder,
    // window settings
    pub(crate) title: String,
    pub(crate) init_size: PhysicalSize<u32>,
    pub(crate) maximized: bool,
    pub(crate) full_screen: bool,
    // render settings
    pub(crate) power_preference: wgpu::PowerPreference,
    pub(crate) base_color: Color,
    pub(crate) surface_preferred_format: wgpu::TextureFormat,
    // input settings
    pub(crate) double_click_threshold: Duration,
    pub(crate) long_press_threshold: Duration,
    pub(crate) mouse_primary_button: MousePrimaryButton,
    pub(crate) scroll_pixel_per_line: f32,
    // font settings
    pub(crate) default_font_size: f32,
    // debug / profiling config
    pub(crate) debug_config: DebugConfig,
}

pub(crate) enum RuntimeBuilder {
    GivenRuntime(tokio::runtime::Runtime),
    CreateInternally { threads: usize },
}

impl RuntimeBuilder {
    pub fn build(self) -> Result<tokio::runtime::Runtime, std::io::Error> {
        match self {
            Self::GivenRuntime(runtime) => {
                trace!("RuntimeBuilder::build: using provided runtime");
                Ok(runtime)
            }
            Self::CreateInternally { threads } => {
                let cpu_threads = std::thread::available_parallelism().map_or(1, |n| n.get());
                let threads = threads.min(cpu_threads);

                trace!(
                    "RuntimeBuilder::build: creating runtime with threads={} (cpu={})",
                    threads, cpu_threads
                );
                if threads == 1 {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                } else {
                    tokio::runtime::Builder::new_multi_thread()
                        .worker_threads(threads)
                        .enable_all()
                        .build()
                }
            }
        }
    }
}

impl<Message: Send + 'static, Event: Send + 'static, B: Backend<Event> + Send + Sync + 'static> WinitInstanceBuilder<Message, Event, B> {
    pub fn new(component: impl AnyComponent<Message, Event> + 'static, backend: B) -> Self {
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
        trace!(
            "WinitInstanceBuilder::new: initializing with default configuration (threads={threads})"
        );
        Self {
            component: Box::new(component),
            backend,
            runtime_builder: RuntimeBuilder::CreateInternally { threads },
            title: "Matcha App".to_string(),
            init_size: PhysicalSize::new(800, 600),
            maximized: false,
            full_screen: false,
            power_preference: POWER_PREFERENCE,
            base_color: BASE_COLOR,
            surface_preferred_format: PREFERRED_SURFACE_FORMAT,
            double_click_threshold: DOUBLE_CLICK_THRESHOLD,
            long_press_threshold: LONG_PRESS_THRESHOLD,
            mouse_primary_button: MOUSE_PRIMARY_BUTTON,
            scroll_pixel_per_line: SCROLL_PIXEL_PER_LINE,
            default_font_size: DEFAULT_FONT_SIZE,
            debug_config: DebugConfig::default(),
        }
    }

    // --- Settings ---

    pub fn tokio_runtime(mut self, runtime: tokio::runtime::Runtime) -> Self {
        self.runtime_builder = RuntimeBuilder::GivenRuntime(runtime);
        self
    }

    pub fn worker_threads(mut self, threads: usize) -> Self {
        self.runtime_builder = RuntimeBuilder::CreateInternally { threads };
        self
    }

    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    pub fn init_size(mut self, width: u32, height: u32) -> Self {
        self.init_size = PhysicalSize::new(width, height);
        self
    }

    pub fn maximized(mut self, maximized: bool) -> Self {
        self.maximized = maximized;
        self
    }

    pub fn full_screen(mut self, full_screen: bool) -> Self {
        self.full_screen = full_screen;
        self
    }

    pub fn power_preference(mut self, preference: wgpu::PowerPreference) -> Self {
        self.power_preference = preference;
        self
    }

    pub fn base_color(mut self, color: Color) -> Self {
        self.base_color = color;
        self
    }

    pub fn surface_preferred_format(mut self, format: wgpu::TextureFormat) -> Self {
        self.surface_preferred_format = format;
        self
    }

    pub fn double_click_threshold(mut self, duration: Duration) -> Self {
        self.double_click_threshold = duration;
        self
    }

    pub fn long_press_threshold(mut self, duration: Duration) -> Self {
        self.long_press_threshold = duration;
        self
    }

    pub fn mouse_primary_button(mut self, button: MousePrimaryButton) -> Self {
        self.mouse_primary_button = button;
        self
    }

    pub fn scroll_pixel_per_line(mut self, pixel: f32) -> Self {
        self.scroll_pixel_per_line = pixel;
        self
    }

    pub fn default_font_size(mut self, size: f32) -> Self {
        self.default_font_size = size;
        self
    }

    /// Provide a DebugConfig instance to the builder.
    pub fn debug_config(mut self, cfg: DebugConfig) -> Self {
        self.debug_config = cfg;
        self
    }

    /// Convenience: toggle measure cache disabling.
    pub fn disable_layout_measure_cache(self, v: bool) -> Self {
        self.debug_config
            .disable_layout_measure_cache
            .store(v, std::sync::atomic::Ordering::Relaxed);
        self
    }

    /// Convenience: toggle arrange cache disabling.
    pub fn disable_layout_arrange_cache(self, v: bool) -> Self {
        self.debug_config
            .disable_layout_arrange_cache
            .store(v, std::sync::atomic::Ordering::Relaxed);
        self
    }

    /// Convenience: toggle render node cache disabling.
    pub fn disable_rendernode_cache(self, v: bool) -> Self {
        self.debug_config
            .disable_render_node_cache
            .store(v, std::sync::atomic::Ordering::Relaxed);
        self
    }

    /// Convenience: toggle widget-level cache disabling.
    pub fn always_rebuild_widget(self, v: bool) -> Self {
        self.debug_config
            .always_rebuild_widget
            .store(v, std::sync::atomic::Ordering::Relaxed);
        self
    }

    // --- Build ---

    pub fn build(self) -> Result<WinitInstance<Message, Event, B>, InitError> {
        debug!("WinitInstanceBuilder::build: starting build pipeline");
        // 1) Build Tokio runtime
        let tokio_runtime = self
            .runtime_builder
            .build()
            .map_err(|_| InitError::TokioRuntime)?;
        trace!("WinitInstanceBuilder::build: tokio runtime initialized");

        // 2) Initialize GPU
        let gpu = tokio_runtime
            .block_on(gpu_utils::gpu::Gpu::new(gpu_utils::gpu::GpuDescriptor {
                backends: wgpu::Backends::PRIMARY,
                power_preference: self.power_preference,
                required_features: wgpu::Features::VERTEX_WRITABLE_STORAGE
                    | wgpu::Features::PUSH_CONSTANTS,
                required_limits: None,
                preferred_surface_format: self.surface_preferred_format,
                auto_recover_enabled: false,
            }))
            .map_err(|_| InitError::Gpu)?;
        debug!("WinitInstanceBuilder::build: GPU initialized successfully");

        // 3) Global resources
        let resource = crate::context::GlobalResources::new(gpu);
        trace!("WinitInstanceBuilder::build: global resources created");

        // 4) Create Window UI and apply builder settings
        let mut window_ui = WindowUi::new(
            self.component,
            crate::device_input::mouse_state::MouseStateConfig {
                combo_duration: self.double_click_threshold,
                long_press_duration: self.long_press_threshold,
                primary_button: self.mouse_primary_button,
                pixel_per_line: self.scroll_pixel_per_line,
            },
        )?;
        // Apply window configuration (effective both before and after window creation)
        window_ui.set_title(&self.title);
        window_ui.init_size(self.init_size.width, self.init_size.height);
        window_ui.set_maximized(self.maximized);
        window_ui.set_fullscreen(self.full_screen);
        trace!(
            "WinitInstanceBuilder::build: configured window title='{}' size={}x{}",
            self.title, self.init_size.width, self.init_size.height
        );

        // 5) Renderer
        let renderer = renderer::CoreRenderer::new(&resource.gpu().device());
        trace!("WinitInstanceBuilder::build: renderer initialized");

        // 6) Build instance (single-window Vec 管理)
        debug!("WinitInstanceBuilder::build: finalizing instance");

        // Wrap backend and build ApplicationInstance which owns runtime, resources and windows.
        let backend = Arc::new(self.backend);

        let app_instance = crate::application_instance::ApplicationInstance::new(
            tokio_runtime,
            resource,
            vec![window_ui],
            self.base_color,
            renderer,
            backend,
        );

        // Prepare a oneshot sender for controlling the render loop lifecycle.
        let (exit_signal_sender, _exit_signal_receiver) = tokio::sync::oneshot::channel::<()>();

        Ok(WinitInstance {
            application_instance: app_instance,
            render_loop_exit_signal: Some(exit_signal_sender),
        })
    }
}
