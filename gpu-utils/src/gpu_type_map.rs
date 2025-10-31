use std::{
    any::{Any, TypeId},
    sync::Arc,
};

use crate::device_loss_recoverable::DeviceLossRecoverable;

const TYPE_LOGIC_ERROR: &str =
    "Type map in `TypeMap` should ensure `key == value.type_id()`. This is a bug of TypeMap.";

trait AnyDeviceLossRecoverable: DeviceLossRecoverable + Any + Send + Sync + 'static {}

impl<T> AnyDeviceLossRecoverable for T where T: DeviceLossRecoverable + Send + Sync + 'static {}

#[derive(Default)]
pub struct GpuTypeMap {
    map: dashmap::DashMap<TypeId, Arc<dyn AnyDeviceLossRecoverable>, fxhash::FxBuildHasher>,
}

impl DeviceLossRecoverable for GpuTypeMap {
    fn recover(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        for entry in self.map.iter() {
            entry.value().recover(device, queue);
        }
    }
}

impl GpuTypeMap {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn get_or_insert<T>(&self, v: T) -> Arc<T>
    where
        T: DeviceLossRecoverable + Send + Sync + 'static,
    {
        self.get_or_insert_with(|| v)
    }

    pub fn get_or_insert_default<T>(&self) -> Arc<T>
    where
        T: DeviceLossRecoverable + Default + Send + Sync + 'static,
    {
        self.get_or_insert_with(T::default)
    }

    pub fn get_or_insert_with<T, F>(&self, f: F) -> Arc<T>
    where
        T: DeviceLossRecoverable + Send + Sync + 'static,
        F: FnOnce() -> T,
    {
        (self
            .map
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Arc::new(f()))
            .clone() as Arc<dyn Any + Send + Sync>)
            .downcast()
            .expect(TYPE_LOGIC_ERROR)
    }

    pub fn get_or_try_insert_with<T, E, F>(&self, f: F) -> Result<Arc<T>, E>
    where
        T: DeviceLossRecoverable + Send + Sync + 'static,
        F: FnOnce() -> Result<T, E>,
    {
        Ok((self
            .map
            .entry(TypeId::of::<T>())
            .or_try_insert_with(|| f().map(|f| Arc::new(f) as Arc<dyn AnyDeviceLossRecoverable>))?
            .clone() as Arc<dyn Any + Send + Sync>)
            .downcast()
            .expect(TYPE_LOGIC_ERROR))
    }

    pub async fn get_or_insert_with_async<T, E, F, Fut>(&self, f: F) -> Arc<T>
    where
        T: DeviceLossRecoverable + Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        match self.map.entry(TypeId::of::<T>()) {
            dashmap::Entry::Occupied(occupied_entry) => (occupied_entry.get().clone()
                as Arc<dyn Any + Send + Sync>)
                .downcast()
                .expect(TYPE_LOGIC_ERROR),
            dashmap::Entry::Vacant(vacant_entry) => {
                let v = f().await;
                (vacant_entry.insert(Arc::new(v)).clone() as Arc<dyn Any + Send + Sync>)
                    .downcast()
                    .expect(TYPE_LOGIC_ERROR)
            }
        }
    }

    pub async fn get_or_try_insert_with_async<T, E, F, Fut>(&self, f: F) -> Result<Arc<T>, E>
    where
        T: DeviceLossRecoverable + Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        match self.map.entry(TypeId::of::<T>()) {
            dashmap::Entry::Occupied(occupied_entry) => Ok((occupied_entry.get().clone()
                as Arc<dyn Any + Send + Sync>)
                .downcast()
                .expect(TYPE_LOGIC_ERROR)),
            dashmap::Entry::Vacant(vacant_entry) => {
                let v = f().await?;
                Ok(
                    (vacant_entry.insert(Arc::new(v)).clone() as Arc<dyn Any + Send + Sync>)
                        .downcast()
                        .expect(TYPE_LOGIC_ERROR),
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TypeA;
    struct TypeB;
    #[derive(Default)]
    struct TypeC {
        v: u32,
    }

    impl DeviceLossRecoverable for TypeA {
        fn recover(&self, _device: &wgpu::Device, _queue: &wgpu::Queue) {}
    }

    impl DeviceLossRecoverable for TypeB {
        fn recover(&self, _device: &wgpu::Device, _queue: &wgpu::Queue) {}
    }

    impl DeviceLossRecoverable for TypeC {
        fn recover(&self, _device: &wgpu::Device, _queue: &wgpu::Queue) {}
    }

    #[test]
    fn test_common_resource() {
        let resource = GpuTypeMap::new();

        let a = resource.get_or_insert(TypeA);
        let b = resource.get_or_insert_with(|| TypeB);
        let c = resource.get_or_insert_default::<TypeC>();

        assert!(TypeId::of::<Arc<TypeA>>() == a.type_id());
        assert!(TypeId::of::<Arc<TypeB>>() == b.type_id());
        assert!(TypeId::of::<Arc<TypeC>>() == c.type_id());

        let c2 = resource.get_or_insert(TypeC { v: 42 });
        assert_eq!(c2.v, u32::default());
    }
}
