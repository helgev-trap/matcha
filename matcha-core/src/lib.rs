#[cfg(all(feature = "winit", feature = "baseview"))]
compile_error!("feature \"winit\" and feature \"baseview\" cannot be enabled at the same time");

pub mod application;
pub mod backend;
pub mod event;
pub mod renderer;
pub mod ui_arch;
pub mod window;
pub mod window_manager;

#[cfg(feature = "winit")]
pub(crate) mod winit_interface;
