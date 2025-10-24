use std::collections::HashMap;
use std::sync::{Arc, Weak};

use guillotiere::euclid::Box2D;
use guillotiere::{AllocId, AtlasAllocator, Size, euclid};
use log::{trace, warn};
use parking_lot::Mutex;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AtlasRegion {
    inner: Arc<RegionData>,
}

// We only store the texture id and reference to the atlas,
// to make `Texture` remain valid after `TextureAtlas` resizes or changes,
// except for data loss when the atlas shrinks.
#[derive()]
struct RegionData {
    // allocation info
    region_id: RegionId,
    atlas_id: TextureAtlasId,
    // interaction with the atlas
    atlas: Weak<Mutex<TextureAtlas>>,
    // It may be useful to store some information about the texture that will not change during atlas resizing
    size: [u32; 2],              // size of the texture in pixels
    format: wgpu::TextureFormat, // format of the texture
}

impl std::fmt::Debug for RegionData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegionData")
            .field("region_id", &self.region_id)
            .field("atlas_id", &self.atlas_id)
            .field("size", &self.size)
            .field("format", &self.format)
            .finish()
    }
}

/// Public API to interact with a texture.
/// User code should not need to know about its id, location, or atlas.
impl AtlasRegion {
    pub fn atlas_id(&self) -> TextureAtlasId {
        trace!(
            "AtlasRegion::atlas_id called for region={:?}",
            self.inner.region_id
        );
        self.inner.atlas_id
    }

    pub fn position_in_atlas(&self) -> Result<(u32, Box2D<f32, euclid::UnknownUnit>), RegionError> {
        trace!(
            "AtlasRegion::position_in_atlas: querying region={:?}",
            self.inner.region_id
        );
        // Get the texture location in the atlas
        let Some(atlas) = self.inner.atlas.upgrade() else {
            warn!("AtlasRegion::position_in_atlas: atlas dropped");
            return Err(RegionError::AtlasGone);
        };
        let atlas = atlas.lock();
        let Some(location) = atlas.get_location(self.inner.region_id) else {
            warn!("AtlasRegion::position_in_atlas: region not found in atlas");
            return Err(RegionError::TextureNotFoundInAtlas);
        };

        Ok((location.page_index, location.uv))
    }

    pub fn area(&self) -> u32 {
        self.inner.size[0] * self.inner.size[1]
    }

    pub fn size(&self) -> [u32; 2] {
        self.inner.size
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.inner.format
    }

    pub fn atlas_pointer(&self) -> Option<usize> {
        self.inner
            .atlas
            .upgrade()
            .map(|arc| Arc::as_ptr(&arc) as usize)
    }

    pub fn translate_uv(&self, uvs: &[[f32; 2]]) -> Result<Vec<[f32; 2]>, RegionError> {
        trace!(
            "AtlasRegion::translate_uv: translating {} vertices for region={:?}",
            uvs.len(),
            self.inner.region_id
        );
        // Get the texture location in the atlas
        let Some(atlas) = self.inner.atlas.upgrade() else {
            warn!("AtlasRegion::translate_uv: atlas dropped");
            return Err(RegionError::AtlasGone);
        };
        let atlas = atlas.lock();
        let Some(location) = atlas.get_location(self.inner.region_id) else {
            warn!("AtlasRegion::translate_uv: region not found in atlas");
            return Err(RegionError::TextureNotFoundInAtlas);
        };
        let x_max = location.uv.max.x;
        let y_max = location.uv.max.y;
        let x_min = location.uv.min.x;
        let y_min = location.uv.min.y;

        // Translate the vertices to the texture area
        let translated_vertices = uvs
            .iter()
            .map(|&[x, y]| {
                [
                    (x_min + (x * (x_max - x_min))).clamp(0.0, 1.0),
                    (y_min + (y * (y_max - y_min))).clamp(0.0, 1.0),
                ]
            })
            .collect::<Vec<_>>();

        Ok(translated_vertices)
    }

    pub fn write_data(&self, queue: &wgpu::Queue, data: &[u8]) -> Result<(), RegionError> {
        trace!(
            "AtlasRegion::write_data: uploading {} bytes to region={:?}",
            data.len(),
            self.inner.region_id
        );
        // Check data consistency
        let bytes_per_pixel = self
            .inner
            .format
            .block_copy_size(None)
            .ok_or(RegionError::InvalidFormatBlockCopySize)?;
        let expected_size = self.inner.size[0] * self.inner.size[1] * bytes_per_pixel;
        if data.len() as u32 != expected_size {
            warn!(
                "AtlasRegion::write_data: data size mismatch (expected {} bytes, got {})",
                expected_size,
                data.len()
            );
            return Err(RegionError::DataConsistencyError(format!(
                "Data size({}byte) does not match expected size({}byte)",
                data.len(),
                expected_size
            )));
        }

        // Get the texture in the atlas and location
        let Some(atlas) = self.inner.atlas.upgrade() else {
            warn!("AtlasRegion::write_data: atlas dropped");
            return Err(RegionError::AtlasGone);
        };
        let atlas = atlas.lock();

        let texture = atlas.texture();
        let Some(location) = atlas.get_location(self.inner.region_id) else {
            warn!("AtlasRegion::write_data: region not found in atlas");
            return Err(RegionError::TextureNotFoundInAtlas);
        };

        let bytes_per_row = self.inner.size[0] * bytes_per_pixel;

        let origin = wgpu::Origin3d {
            x: location.bounds.min.x as u32,
            y: location.bounds.min.y as u32,
            z: location.page_index,
        };

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width: self.inner.size[0],
                height: self.inner.size[1],
                depth_or_array_layers: 1,
            },
        );

        trace!("AtlasRegion::write_data: upload completed");

        Ok(())
    }

    pub fn read_data(&self) -> Result<(), RegionError> {
        todo!()
    }

    pub fn copy_from_texture(&self) -> Result<(), RegionError> {
        todo!()
    }

    pub fn copy_to_texture(&self) -> Result<(), RegionError> {
        todo!()
    }

    pub fn copy_from_buffer(&self) -> Result<(), RegionError> {
        todo!()
    }

    pub fn copy_to_buffer(&self) -> Result<(), RegionError> {
        todo!()
    }

    pub fn set_viewport(&self, render_pass: &mut wgpu::RenderPass<'_>) -> Result<(), RegionError> {
        // Get the texture location in the atlas
        let Some(atlas) = self.inner.atlas.upgrade() else {
            return Err(RegionError::AtlasGone);
        };
        let atlas = atlas.lock();
        let Some(location) = atlas.get_location(self.inner.region_id) else {
            return Err(RegionError::TextureNotFoundInAtlas);
        };

        // Set the viewport to the texture area
        render_pass.set_viewport(
            location.bounds.min.x as f32,
            location.bounds.min.y as f32,
            location.bounds.width() as f32,
            location.bounds.height() as f32,
            0.0,
            1.0,
        );

        Ok(())
    }

    pub fn begin_render_pass<'a>(
        &'a self,
        encoder: &'a mut wgpu::CommandEncoder,
    ) -> Result<wgpu::RenderPass<'a>, RegionError> {
        // Get the texture location in the atlas
        let Some(atlas) = self.inner.atlas.upgrade() else {
            return Err(RegionError::AtlasGone);
        };

        let atlas = atlas.lock();
        let Some(location) = atlas.get_location(self.inner.region_id) else {
            return Err(RegionError::TextureNotFoundInAtlas);
        };

        // Create a render pass for the texture area, targeting the specific array layer (page) with 2D views
        let view = &atlas.layer_texture_views[location.page_index as usize];
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Texture Atlas Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Set the viewport to the texture area
        render_pass.set_viewport(
            location.bounds.min.x as f32,
            location.bounds.min.y as f32,
            location.bounds.width() as f32,
            location.bounds.height() as f32,
            0.0,
            1.0,
        );

        Ok(render_pass)
    }

    pub fn uv(&self) -> Result<Box2D<f32, euclid::UnknownUnit>, RegionError> {
        // Get the texture location in the atlas
        let Some(atlas) = self.inner.atlas.upgrade() else {
            return Err(RegionError::AtlasGone);
        };
        let atlas = atlas.lock();
        let Some(location) = atlas.get_location(self.inner.region_id) else {
            return Err(RegionError::TextureNotFoundInAtlas);
        };

        Ok(location.uv)
    }

    // pub fn with_data<Init, F>(&self, init: Init, f: F) -> Result<(), TextureError>
    // where
    //     Init: FnOnce(&Texture) -> Result<(), TextureError>,
    //     F: FnOnce(&Texture) -> Result<(), TextureError>,
    // {
    //     todo!()
    // }
}

// Ensure the texture area will be deallocated when the texture is dropped.
impl Drop for RegionData {
    fn drop(&mut self) {
        if let Some(atlas) = self.atlas.upgrade() {
            match atlas.lock().deallocate(self.region_id) {
                Ok(_) => {
                    // Successfully deallocated
                }
                Err(DeallocationErrorTextureNotFound) => {
                    // We do not need to handle this error because this means the texture was already deallocated.
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RegionId {
    texture_uuid: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct RegionLocation {
    page_index: u32,
    /// The inclusive bounds in pixels within the atlas.
    bounds: euclid::Box2D<i32, euclid::UnknownUnit>,
    /// The inclusive bounding UV coordinates in the atlas.
    uv: euclid::Box2D<f32, euclid::UnknownUnit>,
}

impl RegionLocation {
    fn new(rec: Box2D<i32, euclid::UnknownUnit>, atlas_size: [u32; 2], page_index: usize) -> Self {
        // rectangle from guillotiere is half-open, we want inclusive bounds

        let bounds = euclid::Box2D::new(
            euclid::Point2D::new(rec.min.x, rec.min.y),
            euclid::Point2D::new(rec.max.x - 1, rec.max.y - 1),
        );
        let uv = euclid::Box2D::new(
            euclid::Point2D::new(
                bounds.min.x as f32 / atlas_size[0] as f32,
                bounds.min.y as f32 / atlas_size[1] as f32,
            ),
            euclid::Point2D::new(
                bounds.max.x as f32 / atlas_size[0] as f32,
                bounds.max.y as f32 / atlas_size[1] as f32,
            ),
        );
        Self {
            page_index: page_index as u32,
            bounds,
            uv,
        }
    }

    fn area(&self) -> u32 {
        self.bounds.area() as u32
    }

    fn size(&self) -> [u32; 2] {
        [
            (self.bounds.max.x - self.bounds.min.x) as u32,
            (self.bounds.max.y - self.bounds.min.y) as u32,
        ]
    }
}

static ATLAS_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureAtlasId {
    id: usize,
}

impl TextureAtlasId {
    fn new() -> Self {
        let id = ATLAS_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self { id }
    }
}

pub struct TextureAtlas {
    id: TextureAtlasId,

    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    layer_texture_views: Vec<wgpu::TextureView>,
    size: wgpu::Extent3d,
    format: wgpu::TextureFormat,

    state: TextureAtlasState,

    weak_self: Weak<Mutex<Self>>,
}

struct TextureAtlasState {
    allocators: Vec<AtlasAllocator>,
    texture_id_to_location: HashMap<RegionId, RegionLocation>,
    texture_id_to_alloc_id: HashMap<RegionId, AllocId>,
    usage: usize,
}

/// Constructor and information methods.
impl TextureAtlas {
    pub fn new(
        device: &wgpu::Device,
        size: wgpu::Extent3d,
        format: wgpu::TextureFormat,
    ) -> Arc<Mutex<Self>> {
        let (texture, texture_view, layer_texture_views) =
            Self::create_texture_and_view(device, format, size);

        // Initialize the state with an empty allocator and allocation map.
        let state = TextureAtlasState {
            allocators: (0..size.depth_or_array_layers)
                .map(|_| Size::new(size.width as i32, size.height as i32))
                .map(AtlasAllocator::new)
                .collect(),
            texture_id_to_location: HashMap::new(),
            texture_id_to_alloc_id: HashMap::new(),
            usage: 0,
        };

        Arc::new_cyclic(|weak_self| {
            Mutex::new(Self {
                id: TextureAtlasId::new(),
                texture,
                texture_view,
                layer_texture_views,
                size,
                format,
                state,
                weak_self: weak_self.clone(),
            })
        })
    }

    pub fn size(&self) -> wgpu::Extent3d {
        self.size
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn capacity(&self) -> usize {
        self.size.width as usize
            * self.size.height as usize
            * self.size.depth_or_array_layers as usize
    }

    pub fn usage(&self) -> usize {
        self.state.usage
    }

    // todo: we can optimize this performance.
    pub fn max_allocation_size(&self) -> [u32; 2] {
        let mut max_size = [0; 2];

        for location in self.state.texture_id_to_location.values() {
            let size = location.size();
            max_size[0] = max_size[0].max(size[0]);
            max_size[1] = max_size[1].max(size[1]);
        }
        max_size
    }
}

/// TextureAtlas allocation and deallocation
impl TextureAtlas {
    /// Allocate a texture in the atlas.
    pub fn allocate(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: [u32; 2],
    ) -> Result<AtlasRegion, TextureAtlasError> {
        // Check if size is smaller than the atlas size
        if size[0] == 0 || size[1] == 0 {
            return Err(TextureAtlasError::AllocationFailedInvalidSize { requested: size });
        }
        if size[0] > self.size.width || size[1] > self.size.height {
            return Err(TextureAtlasError::AllocationFailedTooLarge {
                requested: size,
                available: [self.size.width, self.size.height],
            });
        }

        let size = Size::new(size[0] as i32, size[1] as i32);

        for (page_index, allocator) in self.state.allocators.iter_mut().enumerate() {
            if let Some(alloc) = allocator.allocate(size) {
                let location = RegionLocation::new(
                    alloc.rectangle,
                    [self.size.width, self.size.height],
                    page_index,
                );

                // Create a new TextureId and Texture
                let texture_id = RegionId {
                    texture_uuid: Uuid::new_v4(),
                };
                let texture_inner = RegionData {
                    region_id: texture_id,
                    atlas_id: self.id,
                    atlas: self.weak_self.clone(),
                    size: [size.width as u32, size.height as u32],
                    format: self.format,
                };
                let texture = AtlasRegion {
                    inner: Arc::new(texture_inner),
                };

                // Store the texture location and allocation id in the atlas state.
                self.state
                    .texture_id_to_location
                    .insert(texture_id, location);
                self.state
                    .texture_id_to_alloc_id
                    .insert(texture_id, alloc.id);
                // Update usage
                self.state.usage += location.bounds.area() as usize;

                // Return the allocated texture
                return Ok(texture);
            }
        }

        self.add_one_page(device, queue);

        // Retry allocation after adding a new page
        let page_index = self.state.allocators.len() - 1;
        let allocator = &mut self.state.allocators[page_index];

        let alloc = match allocator.allocate(size) {
            Some(a) => a,
            None => {
                return Err(TextureAtlasError::AllocationFailedNotEnoughSpace);
            }
        };
        let location = RegionLocation::new(
            alloc.rectangle,
            [self.size.width, self.size.height],
            page_index,
        );

        // Create a new TextureId and Texture
        let texture_id = RegionId {
            texture_uuid: Uuid::new_v4(),
        };
        let texture_inner = RegionData {
            region_id: texture_id,
            atlas_id: self.id,
            atlas: self.weak_self.clone(),
            size: [size.width as u32, size.height as u32],
            format: self.format,
        };
        let texture = AtlasRegion {
            inner: Arc::new(texture_inner),
        };

        // Store the texture location and allocation id in the atlas state.
        self.state
            .texture_id_to_location
            .insert(texture_id, location);
        self.state
            .texture_id_to_alloc_id
            .insert(texture_id, alloc.id);
        // Update usage
        self.state.usage += location.bounds.area() as usize;

        // Return the allocated texture
        Ok(texture)
    }

    /// Deallocate a texture from the atlas.
    /// This will be called automatically when the `TextureInner` is dropped.
    fn deallocate(&mut self, id: RegionId) -> Result<(), DeallocationErrorTextureNotFound> {
        // Find the texture location and remove it from the id-to-location map.
        let location = self
            .state
            .texture_id_to_location
            .remove(&id)
            .ok_or(DeallocationErrorTextureNotFound)?;

        // Find the allocation id and remove it from the id-to-alloc-id map.
        let alloc_id = self
            .state
            .texture_id_to_alloc_id
            .remove(&id)
            .ok_or(DeallocationErrorTextureNotFound)?;

        // Deallocate the texture from the allocator.
        self.state.allocators[location.page_index as usize].deallocate(alloc_id);

        // Update usage
        self.state.usage -= location.area() as usize;

        Ok(())
    }
}

/// Resize the atlas to a new size.
impl TextureAtlas {
    fn add_one_page(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let new_size = wgpu::Extent3d {
            width: self.size.width,
            height: self.size.height,
            depth_or_array_layers: self.size.depth_or_array_layers + 1,
        };

        let (new_texture, new_texture_view, new_layer_texture_views) =
            Self::create_texture_and_view(device, self.format, new_size);

        self.state.allocators.push(AtlasAllocator::new(Size::new(
            new_size.width as i32,
            new_size.height as i32,
        )));

        // Copy existing texture data to the new textures.
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("TextureAtlas Resize Encoder"),
        });

        // Copy existing pages into the new texture
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                aspect: wgpu::TextureAspect::All,
                origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
            },
            wgpu::TexelCopyTextureInfo {
                texture: &new_texture,
                mip_level: 0,
                aspect: wgpu::TextureAspect::All,
                origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
            },
            wgpu::Extent3d {
                width: self.size.width,
                height: self.size.height,
                depth_or_array_layers: self.size.depth_or_array_layers,
            },
        );

        // Clear only the newly added layer to ensure it is initialized and transparent.
        // This prevents uninitialized memory in the new layer and keeps existing pages intact.
        let new_layer_index = new_size.depth_or_array_layers - 1;
        if let Some(view) = new_layer_texture_views.get(new_layer_index as usize) {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("TextureAtlas Init New Layer Clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        queue.submit(Some(encoder.finish()));

        // Update the atlas state with the new textures and views.
        self.texture = new_texture;
        self.texture_view = new_texture_view;
        self.layer_texture_views = new_layer_texture_views;
        self.size = new_size;
    }
}

impl TextureAtlas {
    fn get_location(&self, id: RegionId) -> Option<RegionLocation> {
        self.state.texture_id_to_location.get(&id).copied()
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    pub fn texture_view(&self) -> &wgpu::TextureView {
        &self.texture_view
    }
}

// helper functions
impl TextureAtlas {
    fn create_texture_and_view(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        page_size: wgpu::Extent3d,
    ) -> (wgpu::Texture, wgpu::TextureView, Vec<wgpu::TextureView>) {
        let texture_label = format!("texture_atlas_texture_{format:?}");
        let texture_view_label = format!("texture_atlas_texture_view_{format:?}");

        let texture_descriptor = wgpu::TextureDescriptor {
            label: Some(&texture_label),
            size: page_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        };
        let texture = device.create_texture(&texture_descriptor);

        // D2Array view for sampling all layers
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&texture_view_label),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            aspect: wgpu::TextureAspect::All,
            ..Default::default()
        });

        // Per-layer D2 views for render attachments (one per array layer)
        let mut per_layer_views = Vec::with_capacity(page_size.depth_or_array_layers as usize);
        for layer in 0..page_size.depth_or_array_layers {
            let layer_view = texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!("texture_atlas_layer_view_{format:?}_{layer}")),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_mip_level: 0,
                mip_level_count: Some(1),
                base_array_layer: layer,
                array_layer_count: Some(1),
                aspect: wgpu::TextureAspect::All,
                ..Default::default()
            });
            per_layer_views.push(layer_view);
        }

        (texture, texture_view, per_layer_views)
    }
}

/// `DeallocationErrorTextureNotFound` only be used in this file.
struct DeallocationErrorTextureNotFound;

#[derive(Error, Debug)]
pub enum RegionError {
    #[error("The texture's atlas has been dropped.")]
    AtlasGone,
    #[error("The texture was not found in the atlas.")]
    TextureNotFoundInAtlas,
    #[error("Data consistency error: {0}")]
    DataConsistencyError(String),
    #[error("Invalid format block copy size.")]
    InvalidFormatBlockCopySize,
}

#[derive(Error, Debug)]
pub enum TextureAtlasError {
    #[error("Allocation failed because there was not enough space in the atlas.")]
    AllocationFailedNotEnoughSpace,
    #[error("Resizing the atlas failed because there was not enough space for all the textures.")]
    ResizeFailedNotEnoughSpace,
    #[error(
        "Allocation failed because the requested size is too large for the atlas. requested: {requested:?} available: {available:?}"
    )]
    AllocationFailedTooLarge {
        requested: [u32; 2],
        available: [u32; 2],
    },
    #[error("Allocation failed because the requested size is invalid. requested: {requested:?}")]
    AllocationFailedInvalidSize { requested: [u32; 2] },
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
    impl TextureAtlas {
        fn allocation_count(&self) -> usize {
            self.state.texture_id_to_location.len()
        }
    }

    #[cfg(test)]
    impl AtlasRegion {
        fn location(&self) -> Option<RegionLocation> {
            let atlas = self.inner.atlas.upgrade()?;
            let atlas = atlas.lock();
            atlas.get_location(self.inner.region_id)
        }
    }

    /// Tests if the `TextureAtlas` is initialized with the correct parameters.
    #[test]
    fn test_atlas_initialization() {
        futures::executor::block_on(async {
            let (device, _queue) = setup_wgpu().await;
            let size = wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 4,
            };
            let format = wgpu::TextureFormat::Rgba8UnormSrgb;
            let atlas = TextureAtlas::new(&device, size, format);
            let atlas = atlas.lock();

            assert_eq!(atlas.size(), size);
            assert_eq!(atlas.format(), format);
            assert_eq!(atlas.capacity(), 256 * 256 * 4);
            assert_eq!(atlas.usage(), 0);
            assert_eq!(atlas.allocation_count(), 0);

            let texture = atlas.texture();
            assert_eq!(texture.format(), format);

            let _texture_view = atlas.texture_view();
        });
    }

    /// Tests the basic allocation and deallocation of textures.
    /// It verifies that allocation increases usage and deallocation (on drop) decreases it.
    #[test]
    fn test_texture_allocation_and_deallocation() {
        futures::executor::block_on(async {
            let (device, queue) = setup_wgpu().await;
            let size = wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            };
            let format = wgpu::TextureFormat::Rgba8UnormSrgb;
            let atlas = TextureAtlas::new(&device, size, format);

            // Allocate one texture
            let texture1 = atlas.lock().allocate(&device, &queue, [32, 32]).unwrap();
            assert_eq!(atlas.lock().allocation_count(), 1);
            assert_eq!(atlas.lock().usage(), 32 * 32);

            // Allocate another texture
            let texture2 = atlas.lock().allocate(&device, &queue, [16, 16]).unwrap();
            assert_eq!(atlas.lock().allocation_count(), 2);
            assert_eq!(atlas.lock().usage(), 32 * 32 + 16 * 16);

            // Deallocate one texture
            drop(texture1);
            assert_eq!(atlas.lock().allocation_count(), 1);
            assert_eq!(atlas.lock().usage(), 16 * 16);

            // Deallocate the other texture
            drop(texture2);
            assert_eq!(atlas.lock().allocation_count(), 0);
            assert_eq!(atlas.lock().usage(), 0);
        });
    }

    /*
    /// Tests that the atlas correctly returns an error when there is not enough space for a new allocation.
    #[test]
    fn test_allocation_failure() {
        pollster::block_on(async {
            let (device, queue) = setup_wgpu().await;
            let size = wgpu::Extent3d {
                width: 32,
                height: 32,
                depth_or_array_layers: 1,
            };
            let formats = &[wgpu::TextureFormat::Rgba8UnormSrgb];
            let atlas = TextureAtlas::new(&device, size, formats);

            // This should succeed
            let _texture1 = atlas.lock().allocate(&device, &queue, [32, 32]).unwrap();

            // This should fail
            let result = atlas.lock().allocate(&device, &queue, [1, 1]);

            assert!(matches!(
                result,
                Err(TextureAtlasError::AllocationFailedNotEnoughSpace)
            ));
        });
    }
    */

    /// Tests if the space freed by a deallocated texture can be reused by a new allocation.
    #[test]
    fn test_reuse_deallocated_space() {
        futures::executor::block_on(async {
            let (device, queue) = setup_wgpu().await;
            let size = wgpu::Extent3d {
                width: 64,
                height: 64,
                depth_or_array_layers: 1,
            };
            let format = wgpu::TextureFormat::Rgba8UnormSrgb;
            let atlas = TextureAtlas::new(&device, size, format);

            let texture1 = atlas.lock().allocate(&device, &queue, [64, 64]).unwrap();
            assert_eq!(atlas.lock().allocation_count(), 1);

            drop(texture1);
            assert_eq!(atlas.lock().allocation_count(), 0);

            // Should be able to allocate again in the same space
            let _texture2 = atlas.lock().allocate(&device, &queue, [64, 64]).unwrap();
            assert_eq!(atlas.lock().allocation_count(), 1);
        });
    }

    /// Tests if the UV coordinates of an allocated texture are calculated correctly.
    #[test]
    fn test_texture_uv() {
        futures::executor::block_on(async {
            let (device, queue) = setup_wgpu().await;
            let size = wgpu::Extent3d {
                width: 128,
                height: 128,
                depth_or_array_layers: 1,
            };
            let format = wgpu::TextureFormat::Rgba8UnormSrgb;
            let atlas = TextureAtlas::new(&device, size, format);

            let texture = atlas.lock().allocate(&device, &queue, [32, 64]).unwrap();
            let uv = texture.uv().unwrap();

            assert!(uv.min.x >= 0.0 && uv.min.x < 1.0);
            assert!(uv.min.y >= 0.0 && uv.min.y < 1.0);
            assert!(uv.max.x > uv.min.x && uv.max.x <= 1.0);
            assert!(uv.max.y > uv.min.y && uv.max.y <= 1.0);

            let expected_uv_width = 32.0 / 128.0;
            let expected_uv_height = 64.0 / 128.0;
            assert!((uv.width() - expected_uv_width).abs() < f32::EPSILON);
            assert!((uv.height() - expected_uv_height).abs() < f32::EPSILON);
        });
    }

    /// Tests that texture methods return `TextureError::AtlasGone` after the atlas has been dropped.
    #[test]
    fn test_texture_error_when_atlas_gone() {
        futures::executor::block_on(async {
            let (device, queue) = setup_wgpu().await;
            let size = wgpu::Extent3d {
                width: 128,
                height: 128,
                depth_or_array_layers: 1,
            };
            let format = wgpu::TextureFormat::Rgba8UnormSrgb;
            let atlas = TextureAtlas::new(&device, size, format);

            let texture = atlas.lock().allocate(&device, &queue, [32, 32]).unwrap();

            drop(atlas);

            let result = texture.uv();
            assert!(matches!(result, Err(RegionError::AtlasGone)));
        });
    }

    /// Tests that `write_data` writes to the correct location in the atlas.
    #[test]
    fn test_texture_write_and_read_data() {
        futures::executor::block_on(async {
            let (device, queue) = setup_wgpu().await;
            let atlas_size = wgpu::Extent3d {
                width: 512,
                height: 512,
                depth_or_array_layers: 1,
            };
            let texture_format = wgpu::TextureFormat::R8Uint;
            let atlas = TextureAtlas::new(&device, atlas_size, texture_format);

            // Allocate two textures to ensure the second one is not at the origin
            let _texture1 = atlas.lock().allocate(&device, &queue, [10, 10]).unwrap();
            let texture2 = atlas.lock().allocate(&device, &queue, [17, 17]).unwrap(); // Use non-aligned size

            let texture_size = texture2.size();
            let location = texture2.location().unwrap();

            // Create sample data to write
            let data: Vec<u8> = (0..texture_size[0] * texture_size[1])
                .map(|i| (i % 256) as u8)
                .collect();
            texture2.write_data(&queue, &data).unwrap();

            // Create a buffer to read the data back, respecting alignment
            let bytes_per_pixel = texture_format.block_copy_size(None).unwrap();
            let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
            let bytes_per_row_unaligned = texture_size[0] * bytes_per_pixel;
            let padded_bytes_per_row = bytes_per_row_unaligned.div_ceil(align) * align;
            let buffer_size = (padded_bytes_per_row * texture_size[1]) as u64;

            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Readback Buffer"),
                size: buffer_size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });

            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

            // Copy the written data from the atlas texture to the buffer
            let copy_size = wgpu::Extent3d {
                width: texture_size[0],
                height: texture_size[1],
                depth_or_array_layers: 1,
            };
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: atlas.lock().texture(),
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: location.bounds.min.x as u32,
                        y: location.bounds.min.y as u32,
                        z: location.page_index,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &buffer,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_bytes_per_row),
                        rows_per_image: Some(texture_size[1]),
                    },
                },
                copy_size,
            );

            queue.submit(Some(encoder.finish()));

            // Read the buffer and verify the data
            let buffer_slice = buffer.slice(..);
            let (tx, rx) = std::sync::mpsc::channel();
            buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                tx.send(result).unwrap();
            });
            let _ = device.poll(wgpu::MaintainBase::Wait);
            rx.recv().unwrap().unwrap();

            let padded_data = buffer_slice.get_mapped_range();
            // Compare the original data with the (potentially padded) data from the buffer
            for y in 0..texture_size[1] {
                let start_padded = (y * padded_bytes_per_row) as usize;
                let end_padded = start_padded + bytes_per_row_unaligned as usize;
                let start_original = (y * bytes_per_row_unaligned) as usize;
                let end_original = start_original + bytes_per_row_unaligned as usize;
                assert_eq!(
                    &padded_data[start_padded..end_padded],
                    &data[start_original..end_original]
                );
            }
        });
    }
}
