#[derive(Debug, Clone)]
pub struct RawDeviceEvent {}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RawDeviceId {
    pub id: winit::event::DeviceId,
}