use crate::recursive_atlas::CacheKey;

// MARK: CacheNode

pub struct CacheNode<const N: u32> {
    // the size of the texture atlas
    current_size: u32, // equals to x_range[1] - x_range[0] + 1 and y_range[1] - y_range[0] + 1
    current_level: u32,

    // texture atlas range of this node
    x_range: [u32; 2],
    y_range: [u32; 2],

    // the data stored in this node
    data: CacheData<N>,
}

pub enum CacheData<const N: u32> {
    Set {
        // divide the texture atlas into 4 parts
        // +---+---+- x
        // | 0 | 1 |
        // +---+---+
        // | 2 | 3 |
        // +---+---+
        // y
        item: Vec<CacheNode<N>>, // 4 items
    },
    Data {
        key: CacheKey<N>, // Update to use CacheKey<N>
        create_at: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreData<const N: u32> {
    pub key: CacheKey<N>,
    pub atlas_x_range: [u32; 2],
    pub atlas_y_range: [u32; 2],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreResult<const N: u32> {
    pub store_data: StoreData<N>,
    pub dead_items: Vec<CacheKey<N>>,
}

impl<const N: u32> StoreResult<N> {
    pub fn add_dead_item(mut self, key: CacheKey<N>) -> Self {
        self.dead_items.push(key);
        self
    }
}

// MARK: impl CacheNode

impl<const N: u32> CacheNode<N> {
    pub fn new(
        current_size: u32,
        current_level: u32,
        offset_x: u32,
        offset_y: u32,
    ) -> Option<Self> {
        // check if the current can be divided by 2^current_level
        if current_size % (2u32.pow(current_level)) != 0 {
            return None;
        }

        Some(Self {
            current_size,
            current_level,
            x_range: [offset_x, offset_x + current_size - 1],
            y_range: [offset_y, offset_y + current_size - 1],
            data: CacheData::Set { item: Vec::new() },
        })
    }

    pub fn store(&mut self, key: CacheKey<N>, time: u32) -> Option<StoreResult<N>> {
        let new_key = key;

        match &mut self.data {
            CacheData::Set { item } => {
                let current_size = self.current_size;

                let half_size = current_size / 2;

                if new_key.get_texture_size() > current_size {
                    // the new key is too big to be stored in this node
                    None
                } else if half_size < new_key.get_texture_size() || self.current_level == 0 {
                    // cache will be stored here

                    // collect old items that will be removed
                    let dead_items = item
                        .iter()
                        .flat_map(|node| node.items())
                        .collect::<Vec<_>>();

                    // store the new item
                    self.data = CacheData::Data {
                        key: new_key,
                        create_at: time,
                    };

                    Some(StoreResult {
                        store_data: StoreData {
                            key: new_key,
                            atlas_x_range: self.x_range,
                            atlas_y_range: self.y_range,
                        },
                        dead_items,
                    })
                } else {
                    // new_key.size < half_size as f32 and self.current_level > 0
                    // store in child node

                    match item.len() {
                        0 => {
                            let new_node = CacheNode::new(
                                half_size,
                                self.current_level - 1,
                                self.x_range[0],
                                self.y_range[0],
                            )
                            .unwrap();
                            item.push(new_node);

                            item[0].store(new_key, time)
                        }
                        1 => {
                            let new_node = CacheNode::new(
                                half_size,
                                self.current_level - 1,
                                self.x_range[0] + half_size,
                                self.y_range[0],
                            )
                            .unwrap();
                            item.push(new_node);

                            item[1].store(new_key, time)
                        }
                        2 => {
                            let new_node = CacheNode::new(
                                half_size,
                                self.current_level - 1,
                                self.x_range[0],
                                self.y_range[0] + half_size,
                            )
                            .unwrap();
                            item.push(new_node);

                            item[2].store(new_key, time)
                        }
                        3 => {
                            let new_node = CacheNode::new(
                                half_size,
                                self.current_level - 1,
                                self.x_range[0] + half_size,
                                self.y_range[0] + half_size,
                            )
                            .unwrap();
                            item.push(new_node);

                            item[3].store(new_key, time)
                        }
                        4 => {
                            // the node is full, we need to find the appropriate node to store the new item
                            // find the child node that has oldest age and store the new item in it
                            let mut oldest_newest_age = u32::MAX;
                            let mut oldest_newest_idx = 0;
                            for (idx, node) in item.iter().enumerate() {
                                let age = node.oldest(new_key.get_texture_size());
                                if age < oldest_newest_age {
                                    oldest_newest_age = age;
                                    oldest_newest_idx = idx;
                                }
                            }

                            // store in the oldest node
                            item[oldest_newest_idx].store(new_key, time)
                        }
                        _ => unreachable!(),
                    }
                }
            }
            CacheData::Data { key, .. } => {
                let current_size = self.current_size;
                let half_size = current_size / 2;

                if new_key.get_texture_size() > current_size {
                    // the new key is too big to be stored in this node
                    None
                } else if half_size < new_key.get_texture_size() || self.current_level == 0 {
                    // cache will be stored here

                    // the cache data that will be removed
                    let dead_items = vec![*key];

                    // store the new item
                    self.data = CacheData::Data {
                        key: new_key,
                        create_at: time,
                    };

                    Some(StoreResult {
                        store_data: StoreData {
                            key: new_key,
                            atlas_x_range: self.x_range,
                            atlas_y_range: self.y_range,
                        },
                        dead_items,
                    })
                } else {
                    // delete current cache and create a new child node

                    // collect old items that will be removed
                    let dead_items = *key;

                    // make this node a CacheData::Set and let it store the new item
                    self.data = CacheData::Set { item: vec![] };

                    Some(self.store(new_key, time)?.add_dead_item(dead_items))
                }
            }
        }
    }

    pub fn items(&self) -> Vec<CacheKey<N>> {
        match &self.data {
            CacheData::Set { item, .. } => item.iter().flat_map(|node| node.items()).collect(),
            CacheData::Data { key, .. } => vec![*key],
        }
    }

    pub fn oldest(&self, size: u32) -> u32 {
        match &self.data {
            CacheData::Set { item, .. } => {
                if self.current_size / 2 < size {
                    // glyph will be stored this level if this node was selected.
                    // return the oldest age of all child nodes
                    item.iter().map(|node| node.oldest(size)).min().unwrap_or(0)
                } else {
                    // glyph will be stored in child node if this node was selected.
                    // return the oldest age of all child nodes
                    if item.len() < 4 {
                        // if the child node is not full, return 0 to show glyph can be stored in this node
                        0
                    } else {
                        // if the child node is full, return the oldest age of all child nodes
                        item.iter().map(|node| node.oldest(size)).min().unwrap_or(0)
                    }
                }
            }
            CacheData::Data { create_at, .. } => *create_at,
        }
    }

    // todo: remove
    pub fn newest(&self, size: u32) -> u32 {
        match &self.data {
            CacheData::Set { item, .. } => {
                item.iter().map(|node| node.newest(size)).max().unwrap_or(0)
            }
            CacheData::Data { create_at, .. } => *create_at,
        }
    }
    // hash_map(&self) -> &HashMap<u32, CacheData>;
}

// MARK: tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_1_depth_node() {
        let current_size = 1024;

        let mut node = CacheNode::<256>::new(current_size, 0, 0, 0).unwrap();

        // nothing is stored yet

        assert!(node.items().is_empty());

        assert_eq!(node.oldest(10), 0);
        assert_eq!(node.newest(10), 0);

        // store a key.

        let new_key_1 = CacheKey::new('a', 10.0, 10);
        let time = 1;
        let store_result = node.store(new_key_1, time);
        assert_eq!(
            store_result,
            Some(StoreResult {
                store_data: StoreData {
                    key: new_key_1,
                    atlas_x_range: node.x_range,
                    atlas_y_range: node.y_range,
                },
                dead_items: vec![],
            })
        );

        assert_eq!(node.items(), vec![new_key_1]);
        assert_eq!(node.oldest(10), 1);
        assert_eq!(node.newest(10), 1);

        // store another key. this node can only store one key so it will remove the old key

        let new_key_2 = CacheKey::new('b', 10.0, 10);
        let time_2 = 2;
        let store_result = node.store(new_key_2, time_2);

        assert_eq!(
            store_result,
            Some(StoreResult {
                store_data: StoreData {
                    key: new_key_2,
                    atlas_x_range: [0, current_size - 1],
                    atlas_y_range: [0, current_size - 1],
                },
                dead_items: vec![new_key_1]
            })
        );

        // store a key that is too big to be stored in this node (will be rejected)
        // this will not remove the old key

        let new_key_large = CacheKey::new('c', 2000.0, 2000);
        let time_large = 3;
        let store_result_large = node.store(new_key_large, time_large);

        assert_eq!(store_result_large, None);

        assert_eq!(node.items(), vec![new_key_2]);
        assert_eq!(node.oldest(10), 2);
        assert_eq!(node.newest(10), 2);
    }

    #[test]
    fn test_2_depth_node() {
        let current_size = 1024;

        // create a node with 2 depth

        let mut node = CacheNode::<256>::new(current_size, 1, 0, 0).unwrap();

        // nothing is stored yet
        assert!(node.items().is_empty());
        assert_eq!(node.oldest(10), 0);
        assert_eq!(node.newest(10), 0);

        // this node can store 1 key that is larger than 512.0
        // or 4 keys that are smaller than 512.0.

        // storing order: 0 -> 1 -> 2 -> 3
        // +---+---+
        // | 0 | 1 |
        // +---+---+
        // | 2 | 3 |
        // +---+---+

        // store 1st key that is smaller than 512.0
        let small_key_1 = CacheKey::new('a', 10.0, 10);
        let time_small_1 = 1;
        let store_result_small_1 = node.store(small_key_1, time_small_1);

        assert_eq!(
            store_result_small_1,
            Some(StoreResult {
                store_data: StoreData {
                    key: small_key_1,
                    atlas_x_range: [0, current_size / 2 - 1],
                    atlas_y_range: [0, current_size / 2 - 1],
                },
                dead_items: vec![],
            })
        );

        assert_eq!(node.items(), vec![small_key_1]);
        assert_eq!(node.oldest(10), 0); // there is still space in the node to store 10px glyph
        assert_eq!(node.oldest(600), 1); // to store 600px glyph, it have to remove existing key: [small_key_1]
        assert_eq!(node.newest(10), 1);

        // store 2nd key that is smaller than 512.0
        let small_key_2 = CacheKey::new('b', 10.0, 10);
        let time_small_2 = 2;
        let store_result_small_2 = node.store(small_key_2, time_small_2);
        assert_eq!(
            store_result_small_2,
            Some(StoreResult {
                store_data: StoreData {
                    key: small_key_2,
                    atlas_x_range: [current_size / 2, current_size - 1],
                    atlas_y_range: [0, current_size / 2 - 1],
                },
                dead_items: vec![],
            })
        );

        assert_eq!(node.items(), vec![small_key_1, small_key_2]);
        assert_eq!(node.oldest(10), 0);
        assert_eq!(node.oldest(600), 1);
        assert_eq!(node.newest(10), 2);

        // store 3rd key that is smaller than 512.0
        let small_key_3 = CacheKey::new('c', 10.0, 10);
        let time_small_3 = 3;
        let store_result_small_3 = node.store(small_key_3, time_small_3);
        assert_eq!(
            store_result_small_3,
            Some(StoreResult {
                store_data: StoreData {
                    key: small_key_3,
                    atlas_x_range: [0, current_size / 2 - 1],
                    atlas_y_range: [current_size / 2, current_size - 1],
                },
                dead_items: vec![],
            })
        );

        assert_eq!(node.items(), vec![small_key_1, small_key_2, small_key_3]);
        assert_eq!(node.oldest(10), 0);
        assert_eq!(node.oldest(600), 1);
        assert_eq!(node.newest(10), 3);

        // store 4th key that is smaller than 512.0
        let small_key_4 = CacheKey::new('d', 10.0, 10);
        let time_small_4 = 4;
        let store_result_small_4 = node.store(small_key_4, time_small_4);
        assert_eq!(
            store_result_small_4,
            Some(StoreResult {
                store_data: StoreData {
                    key: small_key_4,
                    atlas_x_range: [current_size / 2, current_size - 1],
                    atlas_y_range: [current_size / 2, current_size - 1],
                },
                dead_items: vec![],
            })
        );

        assert_eq!(node.items().len(), 4);
        assert_eq!(node.oldest(10), 1);
        assert_eq!(node.oldest(600), 1);
        assert_eq!(node.newest(10), 4);

        // store 5th key that is smaller than 512.0
        // this will remove the oldest key (small_key_1) and store the new key in the 0th node
        let small_key_5 = CacheKey::new('e', 10.0, 10);
        let time_small_5 = 5;
        let store_result_small_5 = node.store(small_key_5, time_small_5);
        assert_eq!(
            store_result_small_5,
            Some(StoreResult {
                store_data: StoreData {
                    key: small_key_5,
                    atlas_x_range: [0, current_size / 2 - 1],
                    atlas_y_range: [0, current_size / 2 - 1],
                },
                dead_items: vec![small_key_1],
            })
        );

        assert_eq!(
            node.items(),
            vec![small_key_5, small_key_2, small_key_3, small_key_4]
        );
        assert_eq!(node.oldest(600), 2);
        assert_eq!(node.oldest(10), 2);
        assert_eq!(node.newest(10), 5);

        // store 6th key that is larger than 512.0
        // this will remove all of existing keys and store the new key in this node
        let large_key_1 = CacheKey::new('f', 600.0, 600);
        let time_large_1 = 6;
        let store_result_large_1 = node.store(large_key_1, time_large_1);
        assert_eq!(
            store_result_large_1,
            Some(StoreResult {
                store_data: StoreData {
                    key: large_key_1,
                    atlas_x_range: [0, current_size - 1],
                    atlas_y_range: [0, current_size - 1],
                },
                dead_items: vec![small_key_5, small_key_2, small_key_3, small_key_4,],
            })
        );

        assert_eq!(node.items(), vec![large_key_1]);
        assert_eq!(node.oldest(600), 6);
        assert_eq!(node.oldest(10), 6);
        assert_eq!(node.newest(10), 6);
    }
}
