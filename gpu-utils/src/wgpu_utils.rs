pub async fn noop_wgpu() -> (wgpu::Instance, wgpu::Adapter, wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::NOOP,
        backend_options: wgpu::BackendOptions {
            noop: wgpu::NoopBackendOptions { enable: true },
            ..Default::default()
        },
        ..Default::default()
    });

    let adapter = instance
        .enumerate_adapters(wgpu::Backends::NOOP)
        .pop()
        .expect("Failed to find noop adapter");

    let (device, queue) = adapter
        .request_device(&Default::default())
        .await
        .expect("Failed to create device");

    (instance, adapter, device, queue)
}
