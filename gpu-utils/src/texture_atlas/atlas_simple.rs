pub mod atlas;
pub use atlas::{AtlasRegion, RegionError, TextureAtlas, TextureAtlasError, TextureAtlasId};
pub mod manager;
pub use manager::{AtlasManager, AtlasManagerError, MemoryAllocateStrategy};
