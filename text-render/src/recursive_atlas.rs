use cache_node::{CacheNode, StoreData};
use std::{collections::HashMap, num::NonZero};

mod cache_node;

// MARK: CacheKey

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey<const N: u32> {
    char: char,
    // store size multiplied by N to make it able to derive Eq and Hash
    multiplied_font_size: u32,
    texture_size: u32,
    // store more rasterize details here
}

impl<const N: u32> CacheKey<N> {
    pub fn new(char: char, font_size: f32, texture_size: u32) -> Self {
        let multiplied_font_size = (font_size * N as f32).round() as u32;

        CacheKey {
            char,
            multiplied_font_size,
            texture_size,
        }
    }

    pub fn get_texture_size(&self) -> u32 {
        self.texture_size
    }

    pub fn get_font_size(&self) -> f32 {
        self.multiplied_font_size as f32 / N as f32
    }

    pub fn get_char(&self) -> char {
        self.char
    }
}

// MARK: CacheAtlas

pub struct RecursiveAtlas<const N: u32> {
    // label
    label: Option<String>,

    // the size of the texture atlas
    atlas_size: u32, // width == height
    texture_atlas: wgpu::Texture,
    device: wgpu::Device,
    queue: wgpu::Queue,

    // cache management
    map: HashMap<CacheKey<N>, StoreData<N>>,
    node: CacheNode<N>,
    levels: u32,   // the number of levels of the cache atlas
    min_size: u32, // the size of the smallest cache node
    // the max size is the size of the texture atlas
    time: u32,
}

impl<const N: u32> RecursiveAtlas<N> {
    pub fn new(
        label: Option<&str>,
        device: wgpu::Device,
        queue: wgpu::Queue,
        levels: NonZero<u32>,
        min_size: NonZero<u32>,
    ) -> Self {
        let atlas_size = 2u32.pow(levels.get() - 1) * min_size.get();

        let texture_atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("{} Texture Atlas", label.unwrap_or("Cache Atlas"))),
            size: wgpu::Extent3d {
                width: atlas_size,
                height: atlas_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let node = CacheNode::new(atlas_size, levels.get() - 1, 0, 0).unwrap();

        Self {
            label: label.map(|s| s.to_string()),
            atlas_size,
            texture_atlas,
            device,
            queue,
            map: HashMap::new(),
            node,
            levels: levels.get(),
            min_size: min_size.get(),
            time: 0,
        }
    }

    pub fn get_label(&self) -> Option<&String> {
        self.label.as_ref()
    }

    pub fn get_atlas_size(&self) -> u32 {
        self.atlas_size
    }

    pub fn get_texture(&self) -> &wgpu::Texture {
        &self.texture_atlas
    }

    pub fn get_device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn get_queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn get_levels(&self) -> u32 {
        self.levels
    }

    pub fn get_min_size(&self) -> u32 {
        self.min_size
    }
}

// MARK: Cache get/store

impl<const N: u32> RecursiveAtlas<N> {
    pub fn get(&self, key: &CacheKey<N>) -> Option<GlyphTexture<N>> {
        if let Some(store_data) = self.map.get(key) {
            let x_range = store_data.atlas_x_range;
            let y_range = store_data.atlas_y_range;

            Some(GlyphTexture {
                texture: &self.texture_atlas,
                key: *key,
                atlas_x_range: x_range,
                atlas_y_range: y_range,
            })
        } else {
            None
        }
    }

    pub fn store(&mut self, key: &CacheKey<N>) -> Option<GlyphTexture<N>> {
        let new_key = key;

        if let Some(store_data) = self.map.get(new_key) {
            // If the key is already in the cache, return the existing texture
            let x_range = store_data.atlas_x_range;
            let y_range = store_data.atlas_y_range;

            Some(GlyphTexture {
                texture: &self.texture_atlas,
                key: *new_key,
                atlas_x_range: x_range,
                atlas_y_range: y_range,
            })
        } else {
            // Store the new key in the cache atlas
            let store_data = self.node.store(*new_key, self.time)?;

            self.time += 1;

            for dead_item in store_data.dead_items {
                self.map.remove(&dead_item);
            }

            self.map.insert(*new_key, store_data.store_data);

            Some(GlyphTexture {
                texture: &self.texture_atlas,
                key: *new_key,
                atlas_x_range: store_data.store_data.atlas_x_range,
                atlas_y_range: store_data.store_data.atlas_y_range,
            })
        }
    }

    pub fn get_or_insert_with<F>(&mut self, key: &CacheKey<N>, f: F) -> Option<GlyphTexture<N>>
    where
        F: FnOnce() -> Vec<u8>,
    {
        let new_key = key;

        if let Some(store_data) = self.map.get(new_key) {
            // If the key is already in the cache, return the existing texture
            let x_range = store_data.atlas_x_range;
            let y_range = store_data.atlas_y_range;

            Some(GlyphTexture {
                texture: &self.texture_atlas,
                key: *new_key,
                atlas_x_range: x_range,
                atlas_y_range: y_range,
            })
        } else {
            // Store the new key in the cache atlas
            let store_data = self.node.store(*new_key, self.time)?;

            self.time += 1;

            for dead_item in store_data.dead_items {
                self.map.remove(&dead_item);
            }

            self.map.insert(*new_key, store_data.store_data);

            // Upload the texture data to the GPU
            let texture_data = f();

            self.upload_texture_data(
                [
                    store_data.store_data.atlas_x_range[0],
                    store_data.store_data.atlas_y_range[0],
                ],
                [new_key.get_texture_size(); 2],
                &texture_data,
            );

            Some(GlyphTexture {
                texture: &self.texture_atlas,
                key: *new_key,
                atlas_x_range: store_data.store_data.atlas_x_range,
                atlas_y_range: store_data.store_data.atlas_y_range,
            })
        }
    }

    fn upload_texture_data(&self, offset: [u32; 2], size: [u32; 2], data: &[u8]) {
        // self.queue.write_texture(
        //     wgpu::TexelCopyTextureInfo {
        //         texture: &self.texture_atlas,
        //         mip_level: 0,
        //         origin: wgpu::Origin3d {
        //             x: offset[0],
        //             y: offset[1],
        //             z: 0,
        //         },
        //         aspect: wgpu::TextureAspect::All,
        //     },
        //     data,
        //     wgpu::TexelCopyBufferLayout {
        //         offset: 0,
        //         bytes_per_row: Some(size[0]),
        //         rows_per_image: Some(size[1]),
        //     },
        //     wgpu::Extent3d {
        //         width: size[0],
        //         height: size[1],
        //         depth_or_array_layers: 1,
        //     },
        // );
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture_atlas,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: offset[0],
                    y: offset[1],
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(size[0]),
                rows_per_image: Some(size[1]),
            },
            wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
        );
    }
}

pub struct GlyphTexture<'a, const N: u32> {
    pub texture: &'a wgpu::Texture,
    pub key: CacheKey<N>,
    pub atlas_x_range: [u32; 2],
    pub atlas_y_range: [u32; 2],
}

// MARK: tests

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // storing order: 0 -> 1 -> 2 -> 3
    // +---+---+
    // | 0 | 1 |
    // +---+---+
    // | 2 | 3 |
    // +---+---+

    const DUMMY_DATA_5X5: [u8; 25] = [0; 25];
    const DUMMY_DATA_15X15: [u8; 225] = [0; 225];

    fn get_device_queue() -> (wgpu::Device, wgpu::Queue) {
        // prepare gpu
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let adaptor =
            futures::executor::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: true, // for testing on environments without a gpu
            }))
            .unwrap();

        let (device, queue) =
            futures::executor::block_on(adaptor.request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::default(),
                trace: wgpu::Trace::Off,
            }))
            .unwrap();

        (device, queue)
    }

    #[test]
    fn cache_atlas_1_depth() {
        let (device, queue) = get_device_queue();

        // initialize cache atlas

        let mut atlas = RecursiveAtlas::<256>::new(
            Some("Test Atlas"),
            device,
            queue,
            NonZero::new(1).unwrap(),
            NonZero::new(10).unwrap(),
        );

        assert_eq!(atlas.get_label(), Some(&"Test Atlas".to_string()));
        assert_eq!(atlas.get_atlas_size(), 10);
        assert_eq!(atlas.get_levels(), 1);
        assert_eq!(atlas.get_min_size(), 10);

        assert_eq!(atlas.get_texture().size().width, 10);
        assert_eq!(atlas.get_texture().size().height, 10);

        // storing test

        let key_a = CacheKey::<256>::new('A', 5.0, 5);
        {
            // let texture = atlas.store(&key_a).unwrap();
            let texture = atlas
                .get_or_insert_with(&key_a, || DUMMY_DATA_5X5.into())
                .unwrap();
            assert_eq!(texture.atlas_x_range, [0, 9]);
            assert_eq!(texture.atlas_y_range, [0, 9]);
        }

        assert_eq!(atlas.get(&key_a).unwrap().atlas_x_range, [0, 9]);
        assert_eq!(atlas.get(&key_a).unwrap().atlas_y_range, [0, 9]);

        let key_b = CacheKey::<256>::new('B', 5.0, 5);
        {
            let texture = atlas
                .get_or_insert_with(&key_b, || DUMMY_DATA_5X5.into())
                .unwrap();
            assert_eq!(texture.atlas_x_range, [0, 9]);
            assert_eq!(texture.atlas_y_range, [0, 9]);
        }

        assert_eq!(atlas.get(&key_b).unwrap().atlas_x_range, [0, 9]);
        assert_eq!(atlas.get(&key_b).unwrap().atlas_y_range, [0, 9]);

        assert!(atlas.get(&key_a).is_none());
    }

    // MARK: 2_depth

    #[test]
    fn cache_atlas_2_depth() {
        let (device, queue) = get_device_queue();

        // initialize cache atlas

        let mut atlas = RecursiveAtlas::<256>::new(
            Some("Test Atlas"),
            device,
            queue,
            NonZero::new(2).unwrap(),
            NonZero::new(10).unwrap(),
        );

        assert_eq!(atlas.get_label(), Some(&"Test Atlas".to_string()));
        assert_eq!(atlas.get_atlas_size(), 20);
        assert_eq!(atlas.get_levels(), 2);
        assert_eq!(atlas.get_min_size(), 10);

        assert_eq!(atlas.get_texture().size().width, 20);
        assert_eq!(atlas.get_texture().size().height, 20);

        // storing test

        // small a

        let key_a = CacheKey::<256>::new('A', 5.0, 5);
        {
            let texture = atlas
                .get_or_insert_with(&key_a, || DUMMY_DATA_5X5.into())
                .unwrap();
            assert_eq!(texture.atlas_x_range, [0, 9]);
            assert_eq!(texture.atlas_y_range, [0, 9]);
        }

        assert_eq!(atlas.get(&key_a).unwrap().atlas_x_range, [0, 9]);
        assert_eq!(atlas.get(&key_a).unwrap().atlas_y_range, [0, 9]);

        // small b

        let key_b = CacheKey::<256>::new('B', 5.0, 5);
        {
            let texture = atlas
                .get_or_insert_with(&key_b, || DUMMY_DATA_5X5.into())
                .unwrap();
            assert_eq!(texture.atlas_x_range, [10, 19]);
            assert_eq!(texture.atlas_y_range, [0, 9]);
        }

        assert_eq!(atlas.get(&key_b).unwrap().atlas_x_range, [10, 19]);
        assert_eq!(atlas.get(&key_b).unwrap().atlas_y_range, [0, 9]);

        assert_eq!(atlas.get(&key_a).unwrap().atlas_x_range, [0, 9]);
        assert_eq!(atlas.get(&key_a).unwrap().atlas_y_range, [0, 9]);

        // small c

        let key_c = CacheKey::<256>::new('C', 5.0, 5);
        {
            let texture = atlas
                .get_or_insert_with(&key_c, || DUMMY_DATA_5X5.into())
                .unwrap();
            assert_eq!(texture.atlas_x_range, [0, 9]);
            assert_eq!(texture.atlas_y_range, [10, 19]);
        }
        assert_eq!(atlas.get(&key_c).unwrap().atlas_x_range, [0, 9]);
        assert_eq!(atlas.get(&key_c).unwrap().atlas_y_range, [10, 19]);

        assert_eq!(atlas.get(&key_a).unwrap().atlas_x_range, [0, 9]);
        assert_eq!(atlas.get(&key_a).unwrap().atlas_y_range, [0, 9]);

        assert_eq!(atlas.get(&key_b).unwrap().atlas_x_range, [10, 19]);
        assert_eq!(atlas.get(&key_b).unwrap().atlas_y_range, [0, 9]);

        // small d

        let key_d = CacheKey::<256>::new('D', 5.0, 5);
        {
            let texture = atlas
                .get_or_insert_with(&key_d, || DUMMY_DATA_5X5.into())
                .unwrap();
            assert_eq!(texture.atlas_x_range, [10, 19]);
            assert_eq!(texture.atlas_y_range, [10, 19]);
        }
        assert_eq!(atlas.get(&key_d).unwrap().atlas_x_range, [10, 19]);
        assert_eq!(atlas.get(&key_d).unwrap().atlas_y_range, [10, 19]);

        assert_eq!(atlas.get(&key_a).unwrap().atlas_x_range, [0, 9]);
        assert_eq!(atlas.get(&key_a).unwrap().atlas_y_range, [0, 9]);

        assert_eq!(atlas.get(&key_b).unwrap().atlas_x_range, [10, 19]);
        assert_eq!(atlas.get(&key_b).unwrap().atlas_y_range, [0, 9]);

        assert_eq!(atlas.get(&key_c).unwrap().atlas_x_range, [0, 9]);
        assert_eq!(atlas.get(&key_c).unwrap().atlas_y_range, [10, 19]);

        // small e
        let key_e = CacheKey::<256>::new('E', 5.0, 5);
        {
            let texture = atlas
                .get_or_insert_with(&key_e, || DUMMY_DATA_5X5.into())
                .unwrap();
            assert_eq!(texture.atlas_x_range, [0, 9]);
            assert_eq!(texture.atlas_y_range, [0, 9]);
        }
        assert_eq!(atlas.get(&key_e).unwrap().atlas_x_range, [0, 9]);
        assert_eq!(atlas.get(&key_e).unwrap().atlas_y_range, [0, 9]);

        assert!(atlas.get(&key_a).is_none());

        assert_eq!(atlas.get(&key_b).unwrap().atlas_x_range, [10, 19]);
        assert_eq!(atlas.get(&key_b).unwrap().atlas_y_range, [0, 9]);

        assert_eq!(atlas.get(&key_c).unwrap().atlas_x_range, [0, 9]);
        assert_eq!(atlas.get(&key_c).unwrap().atlas_y_range, [10, 19]);

        assert_eq!(atlas.get(&key_d).unwrap().atlas_x_range, [10, 19]);
        assert_eq!(atlas.get(&key_d).unwrap().atlas_y_range, [10, 19]);

        // large A
        let key_large_a = CacheKey::<256>::new('A', 15.0, 15);
        {
            let texture = atlas
                .get_or_insert_with(&key_large_a, || DUMMY_DATA_15X15.into())
                .unwrap();
            assert_eq!(texture.atlas_x_range, [0, 19]);
            assert_eq!(texture.atlas_y_range, [0, 19]);
        }
        assert_eq!(atlas.get(&key_large_a).unwrap().atlas_x_range, [0, 19]);
        assert_eq!(atlas.get(&key_large_a).unwrap().atlas_y_range, [0, 19]);

        assert!(atlas.get(&key_a).is_none());
        assert!(atlas.get(&key_b).is_none());
        assert!(atlas.get(&key_c).is_none());
        assert!(atlas.get(&key_d).is_none());
        assert!(atlas.get(&key_e).is_none());
    }
}
