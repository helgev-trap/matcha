use std::sync::Arc;

use crate::keys::GlyphKey;

// MARK: CacheAtlas

pub struct CacheAtlas<const N: u32 = 256> {
    // config
    panel_size: u32,
    index_width: u32,
    index_height: u32,

    // wgpu
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    texture: wgpu::Texture, // R8Unorm or R8Uint

    // cache management
    // map CacheKey<N> to GlyphCache
    cache: std::collections::HashMap<GlyphKey<N>, GlyphCache>,
    life_queue: std::collections::VecDeque<GlyphKey<N>>,
}

pub struct GlyphCache {
    pub index: [u32; 2],
    pub data_offset: [u32; 2],
    pub data_size: [usize; 2],
    pub bound_offset: [f32; 2],
    pub bound_size: [f32; 2],
}

impl<const N: u32> CacheAtlas<N> {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        panel_size: u32,
        index_width: u32,
        index_height: u32,
    ) -> Self {
        let texture_width = panel_size * index_width;
        let texture_height = panel_size * index_height;

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("CacheAtlas Texture"),
            size: wgpu::Extent3d {
                width: texture_width,
                height: texture_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        CacheAtlas {
            panel_size,
            index_width,
            index_height,
            device,
            queue,
            texture,
            cache: std::collections::HashMap::new(),
            life_queue: std::collections::VecDeque::new(),
        }
    }

    pub fn get_panel_size(&self) -> u32 {
        self.panel_size
    }

    pub fn get_index_size(&self) -> (u32, u32) {
        (self.index_width, self.index_height)
    }

    pub fn get_texture(&self) -> &wgpu::Texture {
        &self.texture
    }
}

impl<const N: u32> CacheAtlas<N> {
    pub fn is_full(&self) -> bool {
        self.life_queue.len() as u32 >= self.index_width * self.index_height
    }

    pub fn get(&self, key: GlyphKey<N>) -> Option<&GlyphCache> {
        self.cache.get(&key)
    }

    pub fn store(&mut self, key: GlyphKey<N>, metrics: fontdue::Metrics) -> &GlyphCache {
        let queue_len = self.life_queue.len() as u32;

        // Check if the cache is full
        if queue_len < self.index_width * self.index_height {
            // Cache is not full, store the new key

            let index = [queue_len % self.index_width, queue_len / self.index_width];
            let data_offset = [index[0] * self.panel_size, index[1] * self.panel_size];
            let data_size = [metrics.width, metrics.height];
            let bound_offset = [
                metrics.bounds.xmin - metrics.xmin as f32,
                (metrics.ymin + metrics.height as i32) as f32
                    - (metrics.bounds.ymin + metrics.bounds.height),
            ];
            let bound_size = [metrics.bounds.width, metrics.bounds.height];

            self.cache.insert(
                key,
                GlyphCache {
                    index,
                    data_offset,
                    data_size,
                    bound_offset,
                    bound_size,
                },
            );
            self.life_queue.push_back(key);

            self.cache.get(&key).unwrap()
        } else {
            // Cache is full, evict the oldest key
            let oldest_key = self.life_queue.pop_front().unwrap();

            // update key mapping
            let old_glyph_cache = self.cache.remove(&oldest_key).unwrap();
            let glyph_cache = GlyphCache {
                index: old_glyph_cache.index,
                data_offset: old_glyph_cache.data_offset,
                data_size: [metrics.width, metrics.height],
                bound_offset: [
                    metrics.bounds.xmin - metrics.xmin as f32,
                    (metrics.ymin + metrics.height as i32) as f32
                        - (metrics.bounds.ymin + metrics.bounds.height),
                ],
                bound_size: [metrics.bounds.width, metrics.bounds.height],
            };
            self.cache.insert(key, glyph_cache);

            // update life queue
            self.life_queue.push_back(key);

            self.cache.get(&key).unwrap()
        }
    }

    pub fn store_data(&mut self, key: GlyphKey<N>, metrics: fontdue::Metrics, data: &[u8]) {
        let GlyphCache {
            data_offset,
            data_size,
            ..
        } = *self.store(key, metrics);

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: data_offset[0],
                    y: data_offset[1],
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.panel_size),
                rows_per_image: Some(self.panel_size),
            },
            wgpu::Extent3d {
                width: data_size[0] as u32,
                height: data_size[1] as u32,
                depth_or_array_layers: 1,
            },
        );
    }

    // pub fn change_device(&mut self, new_device: Arc<wgpu::Device>, new_queue: Arc<wgpu::Queue>) {
    //     todo!()
    // }
}
