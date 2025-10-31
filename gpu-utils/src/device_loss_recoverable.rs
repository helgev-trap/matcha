pub trait DeviceLossRecoverable {
    fn recover(&self, device: &wgpu::Device, queue: &wgpu::Queue);
}
