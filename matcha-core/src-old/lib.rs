// application entry point. wrapper of winit_instance.
pub mod app;

mod application_instance;
mod rendering_loop;
mod window_surface;
mod window_ui;
mod winit_instance;

// widget system
pub mod backend;
pub mod context;
pub mod ui;
// debug / profiling config
pub mod debug_config;

// winit event handling
pub mod device_input;

// types
pub mod color;
pub mod metrics;
