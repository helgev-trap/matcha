use crate::{
    adapter::EventLoop,
    event::{
        device_event::DeviceEvent,
        raw_device_event::{RawDeviceEvent, RawDeviceId},
        window_event::WindowEvent,
    },
    window::WindowId,
};

#[async_trait::async_trait]
pub trait Application: Send + Sync + 'static {
    type Msg: Send + 'static;

    // lifecycle methods
    fn init(&self, runtime: &tokio::runtime::Handle, event_loop: &impl EventLoop);
    fn resumed(&self, runtime: &tokio::runtime::Handle, event_loop: &impl EventLoop);
    fn create_surface(&self, runtime: &tokio::runtime::Handle, event_loop: &impl EventLoop);
    fn destroy_surface(&self, runtime: &tokio::runtime::Handle, event_loop: &impl EventLoop);
    fn suspended(&self, runtime: &tokio::runtime::Handle, event_loop: &impl EventLoop);
    fn exiting(&self, runtime: &tokio::runtime::Handle, event_loop: &impl EventLoop);

    // rendering methods — no event_loop, spawnable in parallel
    async fn render(&self, window_id: WindowId);

    // event methods
    fn window_event(
        &self,
        runtime: &tokio::runtime::Handle,
        event_loop: &impl EventLoop,
        window_id: WindowId,
        event: WindowEvent,
    );
    fn window_destroyed(
        &self,
        runtime: &tokio::runtime::Handle,
        event_loop: &impl EventLoop,
        window_id: WindowId,
    );
    fn device_event(
        &self,
        runtime: &tokio::runtime::Handle,
        event_loop: &impl EventLoop,
        window_id: WindowId,
        event: DeviceEvent,
    );
    fn raw_device_event(
        &self,
        runtime: &tokio::runtime::Handle,
        event_loop: &impl EventLoop,
        raw_device_id: RawDeviceId,
        raw_event: RawDeviceEvent,
    ) {
        let _ = runtime;
        let _ = event_loop;
        let _ = raw_device_id;
        let _ = raw_event;
    }
    fn buffer_updated(&self, runtime: &tokio::runtime::Handle, event_loop: &impl EventLoop);
    fn backend_message(
        &self,
        runtime: &tokio::runtime::Handle,
        event_loop: &impl EventLoop,
        msg: Self::Msg,
    );

    // Default Methods
    fn poll(&self, runtime: &tokio::runtime::Handle, event_loop: &impl EventLoop) {
        let _ = runtime;
        let _ = event_loop;
    }
    fn resume_time_reached(
        &self,
        runtime: &tokio::runtime::Handle,
        event_loop: &impl EventLoop,
        start: std::time::Instant,
        requested_resume: std::time::Instant,
    ) {
        let _ = runtime;
        let _ = event_loop;
        let _ = start;
        let _ = requested_resume;
    }
    fn wait_cancelled(
        &self,
        runtime: &tokio::runtime::Handle,
        event_loop: &impl EventLoop,
        start: std::time::Instant,
        requested_resume: Option<std::time::Instant>,
    ) {
        let _ = runtime;
        let _ = event_loop;
        let _ = start;
        let _ = requested_resume;
    }
    fn about_to_wait(&self, runtime: &tokio::runtime::Handle, event_loop: &impl EventLoop) {
        let _ = runtime;
        let _ = event_loop;
    }
    fn memory_warning(&self, runtime: &tokio::runtime::Handle, event_loop: &impl EventLoop) {
        let _ = runtime;
        let _ = event_loop;
    }
}
