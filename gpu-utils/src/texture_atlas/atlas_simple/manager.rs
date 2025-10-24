use std::sync::Arc;

use dashmap::DashMap;
use log::{debug, trace, warn};
use thiserror::Error;

use super::{AtlasRegion, TextureAtlas, TextureAtlasError};

pub struct MemoryAllocateStrategy {
    pub initial_pages: u32,
    pub resize_threshold: Option<f32>,
    pub resize_factor: f32,
    pub shrink_threshold: f32,
    pub shrink_factor: f32,
}

pub struct AtlasManager {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,

    max_size_of_3d_texture: wgpu::Extent3d,
    memory_strategy: MemoryAllocateStrategy,

    atlases: DashMap<wgpu::TextureFormat, Arc<TextureAtlas>>,
}

impl AtlasManager {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        memory_strategy: MemoryAllocateStrategy,
        max_size_of_3d_texture: wgpu::Extent3d,
    ) -> Self {
        trace!(
            "AtlasManager::new: max_size={}x{} layers={}",
            max_size_of_3d_texture.width,
            max_size_of_3d_texture.height,
            max_size_of_3d_texture.depth_or_array_layers
        );
        Self {
            device,
            queue,
            max_size_of_3d_texture,
            memory_strategy,
            atlases: DashMap::new(),
        }
    }

    pub fn add_format(&self, format: wgpu::TextureFormat) -> Result<(), AtlasManagerError> {
        if self.atlases.contains_key(&format) {
            warn!(
                "AtlasManager::add_format: format {:?} already exists",
                format
            );
            return Err(AtlasManagerError::FormatSetAlreadyExists);
        }

        let atlas = TextureAtlas::new(
            &self.device,
            wgpu::Extent3d {
                width: self.max_size_of_3d_texture.width,
                height: self.max_size_of_3d_texture.height,
                depth_or_array_layers: self.memory_strategy.initial_pages,
            },
            format,
        );
        self.atlases.insert(format, atlas);
        debug!("AtlasManager::add_format: added format {:?}", format);

        Ok(())
    }

    pub fn allocate(
        &self,
        size: [u32; 2],
        format: wgpu::TextureFormat,
    ) -> Result<AtlasRegion, AtlasManagerError> {
        if size[0] == 0 || size[1] == 0 {
            warn!("AtlasManager::allocate: zero-sized allocation requested");
            return Err(AtlasManagerError::InvalidTextureSize);
        }
        if size[0] > self.max_size_of_3d_texture.width
            || size[1] > self.max_size_of_3d_texture.height
        {
            warn!(
                "AtlasManager::allocate: requested size {:?} exceeds max",
                size
            );
            return Err(AtlasManagerError::InvalidTextureSize);
        }

        let atlas = self
            .atlases
            .get(&format)
            .ok_or(AtlasManagerError::FormatSetNotFound)?;
        trace!(
            "AtlasManager::allocate: allocating {:?} in format {:?}",
            size, format
        );
        // Try to allocate directly.
        atlas
            .allocate(&self.device, &self.queue, size)
            .map_err(AtlasManagerError::AtlasError)
    }
}

#[derive(Debug, Error)]
pub enum AtlasManagerError {
    #[error("Format set already exists in the manager")]
    FormatSetAlreadyExists,
    #[error(
        "Requested texture size is invalid (width or height is zero, or exceeds max texture dimension)"
    )]
    InvalidTextureSize,
    #[error("The specified format set was not found in the manager")]
    FormatSetNotFound,
    #[error("Failed to allocate texture, even after attempting to resize the atlas")]
    AllocationFailed,
    #[error("An error occurred in the texture atlas")]
    AtlasError(#[from] TextureAtlasError),
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_wgpu() -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
                force_fallback_adapter: true,
            })
            .await
            .unwrap();
        adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .unwrap()
    }

    #[cfg(test)]
    impl AtlasManager {
        fn atlas_count(&self) -> usize {
            self.atlases.len()
        }

        fn get_atlas_size(&self, format: wgpu::TextureFormat) -> Option<wgpu::Extent3d> {
            self.atlases.get(&format).map(|atlas| atlas.value().size())
        }

        fn get_atlas_usage(&self, format: wgpu::TextureFormat) -> Option<usize> {
            self.atlases.get(&format).map(|atlas| atlas.value().usage())
        }
    }

    /// Tests the initialization of `AtlasManager`.
    #[test]
    fn test_manager_new() {
        futures::executor::block_on(async {
            let (device, queue) = setup_wgpu().await;
            let memory_strategy = MemoryAllocateStrategy {
                initial_pages: 1,
                resize_threshold: Some(0.8),
                resize_factor: 2.0,
                shrink_threshold: 0.2,
                shrink_factor: 0.5,
            };
            let max_size = wgpu::Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers: 8,
            };
            let manager =
                AtlasManager::new(Arc::new(device), Arc::new(queue), memory_strategy, max_size);

            assert_eq!(manager.atlas_count(), 0);
        });
    }

    /// Tests adding a new format set to the manager.
    #[test]
    fn test_add_format() {
        futures::executor::block_on(async {
            let (device, queue) = setup_wgpu().await;
            let memory_strategy = MemoryAllocateStrategy {
                initial_pages: 2,
                resize_threshold: Some(0.8),
                resize_factor: 2.0,
                shrink_threshold: 0.2,
                shrink_factor: 0.5,
            };
            let max_size = wgpu::Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers: 8,
            };
            let manager =
                AtlasManager::new(Arc::new(device), Arc::new(queue), memory_strategy, max_size);

            let format = wgpu::TextureFormat::Rgba8UnormSrgb;
            manager.add_format(format).unwrap();
            assert_eq!(manager.atlas_count(), 1);
            assert_eq!(
                manager
                    .get_atlas_size(format)
                    .unwrap()
                    .depth_or_array_layers,
                2
            );

            // Test adding existing format set
            let result = manager.add_format(format);
            assert!(matches!(
                result,
                Err(AtlasManagerError::FormatSetAlreadyExists)
            ));
        });
    }

    /// Tests basic texture allocation.
    #[test]
    fn test_allocate_basic() {
        futures::executor::block_on(async {
            let (device, queue) = setup_wgpu().await;
            let memory_strategy = MemoryAllocateStrategy {
                initial_pages: 1,
                resize_threshold: Some(0.8),
                resize_factor: 2.0,
                shrink_threshold: 0.2,
                shrink_factor: 0.5,
            };
            let max_size = wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            };
            let manager =
                AtlasManager::new(Arc::new(device), Arc::new(queue), memory_strategy, max_size);

            let format = wgpu::TextureFormat::Rgba8UnormSrgb;
            manager.add_format(format).unwrap();

            let texture = manager.allocate([32, 32], format).unwrap();
            assert_eq!(texture.size(), [32, 32]);
            assert_eq!(manager.get_atlas_usage(format).unwrap(), 32 * 32);
        });
    }

    /// Tests allocation with invalid texture sizes.
    #[test]
    fn test_allocate_invalid_size() {
        futures::executor::block_on(async {
            let (device, queue) = setup_wgpu().await;
            let memory_strategy = MemoryAllocateStrategy {
                initial_pages: 1,
                resize_threshold: Some(0.8),
                resize_factor: 2.0,
                shrink_threshold: 0.2,
                shrink_factor: 0.5,
            };
            let max_size = wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            };
            let manager =
                AtlasManager::new(Arc::new(device), Arc::new(queue), memory_strategy, max_size);

            let format = wgpu::TextureFormat::Rgba8UnormSrgb;
            manager.add_format(format).unwrap();

            // Zero width
            let result = manager.allocate([0, 32], format);
            assert!(matches!(result, Err(AtlasManagerError::InvalidTextureSize)));

            // Zero height
            let result = manager.allocate([32, 0], format);
            assert!(matches!(result, Err(AtlasManagerError::InvalidTextureSize)));

            // Exceeds max width
            let result = manager.allocate([257, 32], format);
            assert!(matches!(result, Err(AtlasManagerError::InvalidTextureSize)));

            // Exceeds max height
            let result = manager.allocate([32, 257], format);
            assert!(matches!(result, Err(AtlasManagerError::InvalidTextureSize)));
        });
    }

    /// Tests allocation with a non-existent format set.
    #[test]
    fn test_allocate_format_not_found() {
        futures::executor::block_on(async {
            let (device, queue) = setup_wgpu().await;
            let memory_strategy = MemoryAllocateStrategy {
                initial_pages: 1,
                resize_threshold: Some(0.8),
                resize_factor: 2.0,
                shrink_threshold: 0.2,
                shrink_factor: 0.5,
            };
            let max_size = wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            };
            let manager =
                AtlasManager::new(Arc::new(device), Arc::new(queue), memory_strategy, max_size);

            let format = wgpu::TextureFormat::Rgba8UnormSrgb;
            let result = manager.allocate([32, 32], format);
            assert!(matches!(result, Err(AtlasManagerError::FormatSetNotFound)));
        });
    }
}
