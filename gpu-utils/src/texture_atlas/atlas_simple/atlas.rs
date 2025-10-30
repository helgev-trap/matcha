use std::collections::HashMap;
use std::sync::{Arc, Weak};

use guillotiere::euclid::Box2D;
use guillotiere::{AllocId, AtlasAllocator, Size, euclid};
use log::{trace, warn};
use parking_lot::{Mutex, RwLock};
use thiserror::Error;
use uuid::Uuid;

use crate::device_loss_recoverable::DeviceLossRecoverable;

mod viewport_clear;
use viewport_clear::ViewportClear;

#[derive(Debug, Clone)]
pub struct AtlasRegion {
    inner: Arc<RegionData>,
}

// We only store the texture id and reference to the atlas,
// to make `Texture` remain valid after `TextureAtlas` resizes or changes,
// except for data loss when the atlas shrinks.
struct RegionData {
    // allocation info
    region_id: RegionId,
    atlas_id: TextureAtlasId,
    // interaction with the atlas
    atlas: Weak<TextureAtlas>,
    // It may be useful to store some information about the texture that will not change during atlas resizing
    texture_size: [u32; 2],      // size of the texture in pixels
    atlas_size: [u32; 2],        // size of the atlas when the texture was allocated
    format: wgpu::TextureFormat, // format of the texture
}

impl std::fmt::Debug for RegionData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegionData")
            .field("region_id", &self.region_id)
            .field("atlas_id", &self.atlas_id)
            .field("texture_size", &self.texture_size)
            .field("atlas_size", &self.atlas_size)
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
        let Some(location) = atlas.get_location(self.inner.region_id) else {
            warn!("AtlasRegion::position_in_atlas: region not found in atlas");
            return Err(RegionError::TextureNotFoundInAtlas);
        };

        Ok((location.page_index, location.usable_uv_bounds))
    }

    pub fn area(&self) -> u32 {
        self.inner.texture_size[0] * self.inner.texture_size[1]
    }

    pub fn texture_size(&self) -> [u32; 2] {
        self.inner.texture_size
    }

    pub fn atlas_size(&self) -> [u32; 2] {
        self.inner.atlas_size
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
        let Some(location) = atlas.get_location(self.inner.region_id) else {
            warn!("AtlasRegion::translate_uv: region not found in atlas");
            return Err(RegionError::TextureNotFoundInAtlas);
        };
        let x_max = location.usable_uv_bounds.max.x;
        let y_max = location.usable_uv_bounds.max.y;
        let x_min = location.usable_uv_bounds.min.x;
        let y_min = location.usable_uv_bounds.min.y;

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
        let expected_size =
            self.inner.texture_size[0] * self.inner.texture_size[1] * bytes_per_pixel;
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
        let texture = atlas.texture();
        let Some(location) = atlas.get_location(self.inner.region_id) else {
            warn!("AtlasRegion::write_data: region not found in atlas");
            return Err(RegionError::TextureNotFoundInAtlas);
        };

        let bytes_per_row = self.inner.texture_size[0] * bytes_per_pixel;

        let origin = wgpu::Origin3d {
            x: location.usable_bounds.min.x as u32,
            y: location.usable_bounds.min.y as u32,
            z: location.page_index,
        };

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
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
                width: self.inner.texture_size[0],
                height: self.inner.texture_size[1],
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
        let Some(location) = atlas.get_location(self.inner.region_id) else {
            return Err(RegionError::TextureNotFoundInAtlas);
        };

        // Set the viewport to the texture area
        render_pass.set_viewport(
            location.usable_bounds.min.x as f32,
            location.usable_bounds.min.y as f32,
            location.usable_bounds.width() as f32,
            location.usable_bounds.height() as f32,
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
        let Some(location) = atlas.get_location(self.inner.region_id) else {
            return Err(RegionError::TextureNotFoundInAtlas);
        };

        // Clear the allocated region (including the margin) before exposing the render pass to users.
        let view = atlas.layer_texture_view(location.page_index as usize);
        let allocation_bounds = location.allocation_bounds();
        let allocation_width = (allocation_bounds.max.x - allocation_bounds.min.x) as u32;
        let allocation_height = (allocation_bounds.max.y - allocation_bounds.min.y) as u32;
        debug_assert!(allocation_width > 0 && allocation_height > 0);
        debug_assert!(allocation_bounds.min.x >= 0);
        debug_assert!(allocation_bounds.min.y >= 0);

        {
            let mut clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Texture Atlas Margin Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            clear_pass.set_viewport(
                allocation_bounds.min.x as f32,
                allocation_bounds.min.y as f32,
                allocation_width as f32,
                allocation_height as f32,
                0.0,
                1.0,
            );
            clear_pass.set_scissor_rect(
                allocation_bounds.min.x as u32,
                allocation_bounds.min.y as u32,
                allocation_width,
                allocation_height,
            );
            atlas.viewport_clear.render(
                &atlas.device(),
                &mut clear_pass,
                atlas.format(),
                [0.0, 0.0, 0.0, 0.0],
            );
        }

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Texture Atlas Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Set the viewport to the usable texture area (excluding margins)
        render_pass.set_viewport(
            location.usable_bounds.min.x as f32,
            location.usable_bounds.min.y as f32,
            location.usable_bounds.width() as f32,
            location.usable_bounds.height() as f32,
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
        let Some(location) = atlas.get_location(self.inner.region_id) else {
            return Err(RegionError::TextureNotFoundInAtlas);
        };

        Ok(location.usable_uv_bounds)
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
            match atlas.deallocate(self.region_id) {
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
    margin: u32,
    allocation_bounds: euclid::Box2D<i32, euclid::UnknownUnit>,
    usable_bounds: euclid::Box2D<i32, euclid::UnknownUnit>,
    usable_uv_bounds: euclid::Box2D<f32, euclid::UnknownUnit>,
}

impl RegionLocation {
    fn new(
        allocation_bounds: Box2D<i32, euclid::UnknownUnit>,
        atlas_size: [u32; 2],
        page_index: usize,
        margin: u32,
    ) -> Self {
        let bounds = if margin == 0 {
            allocation_bounds
        } else {
            euclid::Box2D::new(
                euclid::Point2D::new(
                    allocation_bounds.min.x + margin as i32,
                    allocation_bounds.min.y + margin as i32,
                ),
                euclid::Point2D::new(
                    allocation_bounds.max.x - margin as i32,
                    allocation_bounds.max.y - margin as i32,
                ),
            )
        };

        debug_assert!(bounds.min.x >= allocation_bounds.min.x);
        debug_assert!(bounds.min.y >= allocation_bounds.min.y);
        debug_assert!(bounds.max.x <= allocation_bounds.max.x);
        debug_assert!(bounds.max.y <= allocation_bounds.max.y);
        debug_assert!(bounds.max.x > bounds.min.x);
        debug_assert!(bounds.max.y > bounds.min.y);

        // Normalize the usable bounds to UV space.
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
            margin,
            allocation_bounds,
            usable_bounds: bounds,
            usable_uv_bounds: uv,
        }
    }

    fn area(&self) -> u32 {
        self.usable_bounds.area() as u32
    }

    fn allocation_area(&self) -> u32 {
        self.allocation_bounds.area() as u32
    }

    fn allocation_bounds(&self) -> euclid::Box2D<i32, euclid::UnknownUnit> {
        self.allocation_bounds
    }

    fn size(&self) -> [u32; 2] {
        [
            (self.usable_bounds.max.x - self.usable_bounds.min.x) as u32,
            (self.usable_bounds.max.y - self.usable_bounds.min.y) as u32,
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
    format: wgpu::TextureFormat,
    state: Mutex<TextureAtlasState>,
    resources: RwLock<TextureAtlasResources>,
    device: RwLock<wgpu::Device>,
    viewport_clear: ViewportClear,
    margin: u32,
    weak_self: Weak<Self>,
}

struct TextureAtlasResources {
    texture: wgpu::Texture,
    texture_view: wgpu::TextureView,
    layer_texture_views: Vec<wgpu::TextureView>,
    size: wgpu::Extent3d,
}

struct TextureAtlasState {
    allocators: Vec<AtlasAllocator>,
    texture_id_to_location: HashMap<RegionId, RegionLocation>,
    texture_id_to_alloc_id: HashMap<RegionId, AllocId>,
    usage: usize,
}

/// Constructor and information methods.
impl TextureAtlas {
    pub const DEFAULT_MARGIN_PX: u32 = 1;

    pub fn new(
        device: &wgpu::Device,
        size: wgpu::Extent3d,
        format: wgpu::TextureFormat,
        margin: u32,
    ) -> Arc<Self> {
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

        let resources = TextureAtlasResources {
            texture,
            texture_view,
            layer_texture_views,
            size,
        };

        Arc::new_cyclic(|weak_self| Self {
            id: TextureAtlasId::new(),
            format,
            state: Mutex::new(state),
            resources: RwLock::new(resources),
            device: RwLock::new(device.clone()),
            viewport_clear: ViewportClear::default(),
            margin,
            weak_self: weak_self.clone(),
        })
    }
}

impl DeviceLossRecoverable for TextureAtlas {
    fn recover(&self, device: &wgpu::Device, _: &wgpu::Queue) {
        let format = self.format;
        let size = self.size();
        let id = self.id;

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

        let resources = TextureAtlasResources {
            texture,
            texture_view,
            layer_texture_views,
            size,
        };

        let mut state_lock = self.state.lock();
        *state_lock = state;

        let mut resources_lock = self.resources.write();
        *resources_lock = resources;

        *self.device.write() = device.clone();
        self.viewport_clear.reset();

        trace!(
            "TextureAtlas::recover: recovered atlas id={id:?} with size={size:?} and format={format:?}"
        );
    }
}

impl TextureAtlas {
    pub fn size(&self) -> wgpu::Extent3d {
        self.resources.read().size
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    fn device(&self) -> wgpu::Device {
        self.device.read().clone()
    }

    pub fn margin(&self) -> u32 {
        self.margin
    }

    pub fn capacity(&self) -> usize {
        let resources = self.resources.read();
        resources.size.width as usize
            * resources.size.height as usize
            * resources.size.depth_or_array_layers as usize
    }

    pub fn usage(&self) -> usize {
        self.state.lock().usage
    }

    // todo: we can optimize this performance.
    pub fn max_allocation_size(&self) -> [u32; 2] {
        let mut max_size = [0; 2];

        let state = self.state.lock();
        for location in state.texture_id_to_location.values() {
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
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        requested_size: [u32; 2],
    ) -> Result<AtlasRegion, TextureAtlasError> {
        // Check if size is smaller than the atlas size
        if requested_size[0] == 0 || requested_size[1] == 0 {
            return Err(TextureAtlasError::AllocationFailedInvalidSize {
                requested: requested_size,
            });
        }
        let atlas_size = self.size();
        if requested_size[0] + self.margin * 2 > atlas_size.width
            || requested_size[1] + self.margin * 2 > atlas_size.height
        {
            return Err(TextureAtlasError::AllocationFailedTooLarge {
                requested: requested_size,
                available: [atlas_size.width, atlas_size.height],
                margin_needed: self.margin * 2,
            });
        }

        let doubled_margin =
            self.margin
                .checked_mul(2)
                .ok_or(TextureAtlasError::AllocationFailedInvalidSize {
                    requested: requested_size,
                })?;

        let allocation_width = requested_size[0].checked_add(doubled_margin).ok_or(
            TextureAtlasError::AllocationFailedInvalidSize {
                requested: requested_size,
            },
        )?;
        let allocation_height = requested_size[1].checked_add(doubled_margin).ok_or(
            TextureAtlasError::AllocationFailedInvalidSize {
                requested: requested_size,
            },
        )?;

        if allocation_width > i32::MAX as u32 || allocation_height > i32::MAX as u32 {
            return Err(TextureAtlasError::AllocationFailedInvalidSize {
                requested: requested_size,
            });
        }

        let allocation_size = Size::new(allocation_width as i32, allocation_height as i32);

        if let Some(region) = self.try_allocate(
            requested_size,
            allocation_size,
            [atlas_size.width, atlas_size.height],
        ) {
            return Ok(region);
        }

        self.add_one_page(device, queue);

        let updated_size = self.size();
        self.try_allocate(
            requested_size,
            allocation_size,
            [updated_size.width, updated_size.height],
        )
        .ok_or(TextureAtlasError::AllocationFailedNotEnoughSpace)
    }

    /// Deallocate a texture from the atlas.
    /// This will be called automatically when the `TextureInner` is dropped.
    fn deallocate(&self, id: RegionId) -> Result<(), DeallocationErrorTextureNotFound> {
        let mut state = self.state.lock();

        // Find the texture location and remove it from the id-to-location map.
        let location = state
            .texture_id_to_location
            .remove(&id)
            .ok_or(DeallocationErrorTextureNotFound)?;

        // Find the allocation id and remove it from the id-to-alloc-id map.
        let alloc_id = state
            .texture_id_to_alloc_id
            .remove(&id)
            .ok_or(DeallocationErrorTextureNotFound)?;

        // Deallocate the texture from the allocator.
        state.allocators[location.page_index as usize].deallocate(alloc_id);

        // Update usage
        state.usage -= location.allocation_area() as usize;

        Ok(())
    }

    fn try_allocate(
        &self,
        requested_size: [u32; 2],
        allocation_size: Size,
        atlas_size: [u32; 2],
    ) -> Option<AtlasRegion> {
        let mut state = self.state.lock();

        for (page_index, allocator) in state.allocators.iter_mut().enumerate() {
            if let Some(alloc) = allocator.allocate(allocation_size) {
                let location =
                    RegionLocation::new(alloc.rectangle, atlas_size, page_index, self.margin);

                let texture_id = RegionId {
                    texture_uuid: Uuid::new_v4(),
                };
                let texture_inner = RegionData {
                    region_id: texture_id,
                    atlas_id: self.id,
                    atlas: self.weak_self.clone(),
                    texture_size: requested_size,
                    atlas_size,
                    format: self.format,
                };
                let texture = AtlasRegion {
                    inner: Arc::new(texture_inner),
                };

                state.texture_id_to_location.insert(texture_id, location);
                state.texture_id_to_alloc_id.insert(texture_id, alloc.id);
                state.usage += location.allocation_area() as usize;

                return Some(texture);
            }
        }

        None
    }
}

/// Resize the atlas to a new size.
impl TextureAtlas {
    fn add_one_page(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let mut resources = self.resources.write();
        let previous_size = resources.size;
        let new_size = wgpu::Extent3d {
            width: previous_size.width,
            height: previous_size.height,
            depth_or_array_layers: previous_size.depth_or_array_layers + 1,
        };

        let (new_texture, new_texture_view, new_layer_texture_views) =
            Self::create_texture_and_view(device, self.format, new_size);

        {
            let mut state = self.state.lock();
            state.allocators.push(AtlasAllocator::new(Size::new(
                new_size.width as i32,
                new_size.height as i32,
            )));
        }

        let old_texture = resources.texture.clone();

        // Copy existing texture data to the new textures.
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("TextureAtlas Resize Encoder"),
        });

        // Copy existing pages into the new texture
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &old_texture,
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
                width: previous_size.width,
                height: previous_size.height,
                depth_or_array_layers: previous_size.depth_or_array_layers,
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
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        queue.submit(Some(encoder.finish()));

        resources.texture = new_texture;
        resources.texture_view = new_texture_view;
        resources.layer_texture_views = new_layer_texture_views;
        resources.size = new_size;
    }
}

impl TextureAtlas {
    fn get_location(&self, id: RegionId) -> Option<RegionLocation> {
        self.state.lock().texture_id_to_location.get(&id).copied()
    }

    pub fn texture(&self) -> wgpu::Texture {
        self.resources.read().texture.clone()
    }

    pub fn texture_view(&self) -> wgpu::TextureView {
        self.resources.read().texture_view.clone()
    }

    fn layer_texture_view(&self, index: usize) -> wgpu::TextureView {
        self.resources.read().layer_texture_views[index].clone()
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
        margin_needed: u32,
    },
    #[error("Allocation failed because the requested size is invalid. requested: {requested:?}")]
    AllocationFailedInvalidSize { requested: [u32; 2] },
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    /// Sets up a WGPU device and queue for testing.
    async fn setup_wgpu() -> (wgpu::Device, wgpu::Queue) {
        // todo: change this to use NoopBackend when update wgpu to v25 or later
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = match instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
                force_fallback_adapter: true,
            })
            .await
        {
            Ok(adapter) => adapter,
            Err(e) => instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::default(),
                    compatible_surface: None,
                    force_fallback_adapter: false,
                })
                .await
                .expect("Failed to acquire wgpu adapter"),
        };
        adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .unwrap()
    }

    /// Ensures multiple threads can allocate regions concurrently without panicking.
    #[test]
    fn test_concurrent_allocations() {
        futures::executor::block_on(async {
            let (device, queue) = setup_wgpu().await;
            let atlas_size = wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 2,
            };
            let atlas_format = wgpu::TextureFormat::Rgba8UnormSrgb;
            let atlas = TextureAtlas::new(
                &device,
                atlas_size,
                atlas_format,
                TextureAtlas::DEFAULT_MARGIN_PX,
            );

            let mut handles = Vec::new();
            for _ in 0..8 {
                let atlas = Arc::clone(&atlas);
                let device = device.clone();
                let queue = queue.clone();
                handles.push(thread::spawn(move || {
                    let region = atlas
                        .allocate(&device, &queue, [32, 32])
                        .expect("Concurrent allocation failed");
                    drop(region);
                }));
            }

            for handle in handles {
                handle.join().expect("Thread panicked during allocation");
            }

            assert_eq!(atlas.allocation_count(), 0);
        });
    }

    #[cfg(test)]
    impl TextureAtlas {
        fn allocation_count(&self) -> usize {
            self.state.lock().texture_id_to_location.len()
        }
    }

    #[cfg(test)]
    impl AtlasRegion {
        fn location(&self) -> Option<RegionLocation> {
            let atlas = self.inner.atlas.upgrade()?;
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
            let atlas = TextureAtlas::new(&device, size, format, TextureAtlas::DEFAULT_MARGIN_PX);

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
            let atlas = TextureAtlas::new(&device, size, format, TextureAtlas::DEFAULT_MARGIN_PX);
            let margin = TextureAtlas::DEFAULT_MARGIN_PX as usize;

            // Allocate one texture
            let texture1 = atlas.allocate(&device, &queue, [32, 32]).unwrap();
            assert_eq!(atlas.allocation_count(), 1);
            assert_eq!(atlas.usage(), (32 + 2 * margin) * (32 + 2 * margin));

            // Allocate another texture
            let texture2 = atlas.allocate(&device, &queue, [16, 16]).unwrap();
            assert_eq!(atlas.allocation_count(), 2);
            let expected_usage =
                (32 + 2 * margin) * (32 + 2 * margin) + (16 + 2 * margin) * (16 + 2 * margin);
            assert_eq!(atlas.usage(), expected_usage);

            // Deallocate one texture
            drop(texture1);
            assert_eq!(atlas.allocation_count(), 1);
            assert_eq!(atlas.usage(), (16 + 2 * margin) * (16 + 2 * margin));

            // Deallocate the other texture
            drop(texture2);
            assert_eq!(atlas.allocation_count(), 0);
            assert_eq!(atlas.usage(), 0);
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
            let atlas = TextureAtlas::new(
                &device,
                size,
                formats,
                TextureAtlas::DEFAULT_MARGIN_PX,
            );

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
            let atlas = TextureAtlas::new(&device, size, format, TextureAtlas::DEFAULT_MARGIN_PX);

            let texture1 = atlas.allocate(&device, &queue, [64, 64]).unwrap();
            assert_eq!(atlas.allocation_count(), 1);

            drop(texture1);
            assert_eq!(atlas.allocation_count(), 0);

            // Should be able to allocate again in the same space
            let _texture2 = atlas.allocate(&device, &queue, [64, 64]).unwrap();
            assert_eq!(atlas.allocation_count(), 1);
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
            let atlas = TextureAtlas::new(&device, size, format, TextureAtlas::DEFAULT_MARGIN_PX);

            let texture = atlas.allocate(&device, &queue, [32, 64]).unwrap();
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
            let atlas = TextureAtlas::new(&device, size, format, TextureAtlas::DEFAULT_MARGIN_PX);

            let texture = atlas.allocate(&device, &queue, [32, 32]).unwrap();

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
            let atlas = TextureAtlas::new(
                &device,
                atlas_size,
                texture_format,
                TextureAtlas::DEFAULT_MARGIN_PX,
            );

            // Allocate two textures to ensure the second one is not at the origin
            let _texture1 = atlas.allocate(&device, &queue, [10, 10]).unwrap();
            let texture2 = atlas.allocate(&device, &queue, [17, 17]).unwrap(); // Use non-aligned size

            let texture_size = texture2.texture_size();
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
            let atlas_texture = atlas.texture();
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture: &atlas_texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: location.usable_bounds.min.x as u32,
                        y: location.usable_bounds.min.y as u32,
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
            let _ = device.poll(wgpu::PollType::Wait);
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
