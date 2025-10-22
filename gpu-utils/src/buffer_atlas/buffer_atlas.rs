//! # Buffer Atlas
//!
//! This module provides `BufferAtlas`, a system for efficiently managing a large number of
//! small, fixed-size buffers on a single, large GPU buffer.
//!
//! ## Purpose
//!
//! In GUI applications and other scenarios, it's common to send small amounts of uniform data
//! to the GPU for each widget or object. Creating individual buffers for each piece of data
//! can lead to resource management complexity and performance overhead. `BufferAtlas` addresses
//! this by consolidating these small buffers into one large "atlas" buffer, providing several
//! benefits:
//!
//! - **Resource Aggregation**: Reduces the number of GPU buffers to one, simplifying management.
//! - **Efficient Updates**: The `flash()` method batches updates to multiple buffers into a single
//!   GPU command.
//! - **Automatic Lifetime Management**: When a `Buffer` handle is dropped, its space within the
//!   atlas is automatically marked for reuse.
//!
//! ## Usage
//!
//! 1. Create an atlas with `BufferAtlas::new()`.
//! 2. Allocate individual `Buffer`s from the atlas using `BufferAtlas::allocate()`.
//! 3. Write data to a `Buffer` with `Buffer::store()`.
//! 4. At the beginning of your rendering cycle, call `BufferAtlas::flash()` to apply all
//!    changes to the GPU.

use log::{debug, trace};
use std::{
    collections::VecDeque,
    sync::{Arc, Weak},
};

use parking_lot::Mutex;

/// A handle to a single buffer within the atlas.
///
/// This handle is cloneable, allowing multiple owners to reference the same buffer.
/// When all handles are dropped, the corresponding space in the atlas is automatically
/// freed and becomes available for reuse on the next `flash()` call.
#[derive(Clone)]
pub struct Buffer<const N: usize> {
    data: Arc<BufferData<N>>,
}

impl<const N: usize> Buffer<N> {
    /// Stores data in the buffer.
    ///
    /// The data written with this method will be uploaded to the GPU the next time
    /// `BufferAtlas::flash()` is called.
    pub fn store(&self, data: [u8; N]) {
        self.data.store(data);
    }

    /// Returns the unique ID of the `BufferAtlas` this buffer belongs to.
    pub fn atlas_id(&self) -> BufferAtlasId {
        self.data.atlas_id
    }
}

/// The internal data structure for a buffer.
///
/// This is shared via an `Arc` among all `Buffer` handles.
pub struct BufferData<const N: usize> {
    /// The ID of the atlas this buffer belongs to.
    atlas_id: BufferAtlasId,
    /// The actual buffer data and update status (true if updated).
    data: Mutex<(Option<[u8; N]>, bool)>,
}

impl<const N: usize> BufferData<N> {
    /// Creates a new `BufferData`.
    fn new(atlas_id: BufferAtlasId) -> Arc<Self> {
        Arc::new(Self {
            atlas_id,
            data: Mutex::new((None, false)),
        })
    }

    /// Stores data in the buffer.
    /// If the data is different from the current data, it marks the buffer as updated.
    fn store(&self, data: [u8; N]) {
        let (buffer_data, updated) = &mut *self.data.lock();
        if *buffer_data != Some(data) {
            *buffer_data = Some(data);
            *updated = true;
        }
    }

    /// Takes the updated data if it has been modified, resetting the updated flag.
    ///
    /// Returns `Some(data)` if the buffer was updated, otherwise `None`.
    fn copy_updated(&self) -> Option<[u8; N]> {
        let (buffer_data, updated) = &mut *self.data.lock();
        if *updated {
            *updated = false;
            *buffer_data
        } else {
            None
        }
    }
}

/// A unique identifier for a `BufferAtlas`.
static ATLAS_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferAtlasId {
    id: usize,
}

#[allow(clippy::new_without_default)]
impl BufferAtlasId {
    /// Creates a new, unique ID.
    pub fn new() -> Self {
        let id = ATLAS_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        BufferAtlasId { id }
    }
}

/// An atlas that manages many fixed-size buffers on a single GPU buffer.
pub struct BufferAtlas<const N: usize> {
    id: BufferAtlasId,

    /// The single GPU buffer that holds all buffer data.
    ///
    /// This is `None` until the first `flash()` call, after which it is always `Some`.
    atlas: Option<wgpu::Buffer>,

    /// A vector tracking the state of slots in the atlas.
    ///
    /// The index of the vector corresponds to a slot in the atlas.
    /// If `Weak::upgrade()` returns `None`, the slot is considered empty.
    allocations: Vec<Weak<BufferData<N>>>,

    /// A list of buffers scheduled to be allocated in the next `flash()` call.
    ///
    /// Buffers created with `allocate()` are first added here.
    to_be_allocated: Vec<Weak<BufferData<N>>>,
}

impl<const N: usize> Default for BufferAtlas<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> BufferAtlas<N> {
    /// Creates a new `BufferAtlas`.
    pub fn new() -> Self {
        let atlas = Self {
            id: BufferAtlasId::new(),
            atlas: None,
            allocations: Vec::new(),
            to_be_allocated: Vec::new(),
        };
        trace!("BufferAtlas::new: created atlas_id={:?}", atlas.id);
        atlas
    }

    /// Allocates a new buffer within the atlas.
    ///
    /// The actual GPU memory allocation and data upload will occur
    /// the next time `flash()` is called.
    pub fn allocate(&mut self) -> Buffer<N> {
        let buffer = BufferData::new(self.id);
        self.to_be_allocated.push(Arc::downgrade(&buffer));
        trace!(
            "BufferAtlas::allocate: scheduled buffer for atlas_id={:?}; pending={}",
            self.id,
            self.to_be_allocated.len()
        );
        Buffer { data: buffer }
    }

    /// Applies all pending changes to the GPU.
    ///
    /// This method performs the following operations in order:
    /// 1. **Garbage Collection**: Frees slots used by dropped `Buffer` handles.
    /// 2. **Reallocation**: Assigns newly allocated `Buffer`s to the freed slots.
    /// 3. **Resizing**: Expands the GPU buffer if there are not enough free slots.
    /// 4. **Data Transfer**: Uploads data from all `Buffer`s updated with `store()` to the GPU.
    ///
    /// Typically, this method should be called once per frame, before rendering.
    pub fn flash(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        trace!(
            "BufferAtlas::flash: atlas_id={:?} allocations={} pending={}",
            self.id,
            self.allocations.len(),
            self.to_be_allocated.len()
        );
        // 1. Garbage Collection: Collect slots from dropped `Buffer`s in `allocations`.
        let mut empty_slots: VecDeque<usize> = self
            .allocations
            .iter()
            .enumerate()
            .filter_map(|(i, weak)| {
                if weak.upgrade().is_none() {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();

        trace!(
            "BufferAtlas::flash: garbage collected {} empty slots",
            empty_slots.len()
        );

        // Remove any pending allocations for buffers that were dropped before `flash()` was called.
        self.to_be_allocated.retain(|weak| weak.upgrade().is_some());
        trace!(
            "BufferAtlas::flash: {} pending allocations after cleanup",
            self.to_be_allocated.len()
        );

        // 2. Resize Check: If more slots are needed than are available, resize the atlas.
        let empty_slots_count = empty_slots.len();
        let needed_slots = self.to_be_allocated.len();

        if needed_slots > empty_slots_count {
            let additional_slots = needed_slots - empty_slots_count;
            let current_capacity = self.allocations.len();
            let needed_capacity = current_capacity + additional_slots;
            // For performance, round up the capacity to the next power of two.
            let new_capacity = needed_capacity.next_power_of_two();
            debug!(
                "BufferAtlas::flash: resizing atlas_id={:?} from {} to {} slots",
                self.id, current_capacity, new_capacity
            );
            Self::resize(
                device,
                queue,
                &mut self.atlas,
                &mut self.allocations,
                &mut empty_slots,
                new_capacity,
            );
        }

        // 3. Reallocation: Move buffers from `to_be_allocated` into the empty slots of `allocations`.
        for new_item in std::mem::take(&mut self.to_be_allocated)
            .into_iter()
            .filter_map(|weak| weak.upgrade())
        {
            // This `expect` is safe because we resized the atlas to ensure enough space.
            let index = empty_slots
                .pop_front()
                .expect("We checked there is enough space in the atlas");

            // Place the new buffer into the free slot.
            self.allocations[index] = Arc::downgrade(&new_item);
        }

        // 4. Data Transfer: Upload updated data to the GPU.
        //    To improve performance, we batch consecutive memory writes into a single chunk
        //    to reduce the number of `write_buffer` calls.
        let mut chunk_start: usize = 0;
        let mut chunk_data: Vec<u8> = Vec::new();

        // By chaining a dummy element, we ensure the last chunk in the loop is always processed.
        for (i, weak) in self
            .allocations
            .iter()
            .chain(std::iter::once(&Weak::new()))
            .enumerate()
        {
            let updated_data = weak.upgrade().and_then(|b| b.copy_updated());

            if let Some(data) = updated_data {
                // Start a new chunk.
                if chunk_data.is_empty() {
                    chunk_start = i;
                }
                chunk_data.extend_from_slice(&data);
            } else if !chunk_data.is_empty() {
                // End of a chunk. Write the collected data to the GPU.
                if let Some(atlas_buffer) = &self.atlas {
                    trace!(
                        "BufferAtlas::flash: writing chunk start={} bytes={}",
                        chunk_start,
                        chunk_data.len()
                    );
                    queue.write_buffer(
                        atlas_buffer,
                        (chunk_start * N) as wgpu::BufferAddress,
                        &chunk_data,
                    );
                }
                chunk_data.clear();
            }
        }
    }
}

// Helper methods
impl<const N: usize> BufferAtlas<N> {
    /// Resizes the atlas, creating a new GPU buffer and copying the old content.
    fn resize(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        atlas: &mut Option<wgpu::Buffer>,
        allocations: &mut Vec<Weak<BufferData<N>>>,
        empty_slots: &mut VecDeque<usize>,
        new_size: usize,
    ) {
        let old_size = allocations.len();
        if new_size <= old_size {
            return;
        }

        let new_buffer_size = (N * new_size) as wgpu::BufferAddress;

        let new_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("buffer-atlas buffer"),
            size: new_buffer_size,
            usage: wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });

        // If an old buffer exists, copy its contents to the new, larger buffer.
        if let Some(old_buffer) = atlas.take() {
            let old_buffer_size = (N * old_size) as wgpu::BufferAddress;
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("buffer-atlas resize encoder"),
            });
            trace!(
                "BufferAtlas::resize: copying old buffer (size={} bytes) into new size={} bytes",
                old_buffer_size, new_buffer_size
            );
            encoder.copy_buffer_to_buffer(&old_buffer, 0, &new_buffer, 0, old_buffer_size);
            queue.submit(std::iter::once(encoder.finish()));
        }

        *atlas = Some(new_buffer);
        // Expand the `allocations` vector and `empty_slots` queue to the new size.
        allocations.resize_with(new_size, Weak::new);
        empty_slots.extend(old_size..new_size);
        trace!(
            "BufferAtlas::resize: atlas expanded to {} slots ({} new empty slots)",
            new_size,
            new_size - old_size
        );
    }
}
