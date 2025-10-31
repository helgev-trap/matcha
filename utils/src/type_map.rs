use std::{
    any::{Any, TypeId},
    sync::Arc,
};

const TYPE_LOGIC_ERROR: &str =
    "Type map in `TypeMap` should ensure `key == value.type_id()`. This is a bug of TypeMap.";

#[derive(Default)]
pub struct TypeMap {
    map: dashmap::DashMap<TypeId, Arc<dyn Any + Send + Sync>, fxhash::FxBuildHasher>,
}

impl TypeMap {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn get_or_insert<T>(&self, v: T) -> Arc<T>
    where
        T: Send + Sync + 'static,
    {
        self.get_or_insert_with(|| v)
    }

    pub fn get_or_insert_default<T>(&self) -> Arc<T>
    where
        T: Default + Send + Sync + 'static,
    {
        self.get_or_insert_with(T::default)
    }

    pub fn get_or_insert_with<T, F>(&self, f: F) -> Arc<T>
    where
        T: Send + Sync + 'static,
        F: FnOnce() -> T,
    {
        self.map
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Arc::new(f()))
            .clone()
            .downcast()
            .expect(TYPE_LOGIC_ERROR)
    }

    pub fn get_or_try_insert_with<T, E, F>(&self, f: F) -> Result<Arc<T>, E>
    where
        T: Send + Sync + 'static,
        F: FnOnce() -> Result<T, E>,
    {
        Ok(self
            .map
            .entry(TypeId::of::<T>())
            .or_try_insert_with(|| f().map(|f| Arc::new(f) as Arc<dyn Any + Send + Sync>))?
            .clone()
            .downcast()
            .expect(TYPE_LOGIC_ERROR))
    }

    pub async fn get_or_insert_with_async<T, E, F, Fut>(&self, f: F) -> Arc<T>
    where
        T: Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        match self.map.entry(TypeId::of::<T>()) {
            dashmap::Entry::Occupied(occupied_entry) => occupied_entry
                .get()
                .clone()
                .downcast()
                .expect(TYPE_LOGIC_ERROR),
            dashmap::Entry::Vacant(vacant_entry) => {
                let v = f().await;
                vacant_entry
                    .insert(Arc::new(v))
                    .clone()
                    .downcast()
                    .expect(TYPE_LOGIC_ERROR)
            }
        }
    }

    pub async fn get_or_try_insert_with_async<T, E, F, Fut>(&self, f: F) -> Result<Arc<T>, E>
    where
        T: Send + Sync + 'static,
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<T, E>>,
    {
        match self.map.entry(TypeId::of::<T>()) {
            dashmap::Entry::Occupied(occupied_entry) => Ok(occupied_entry
                .get()
                .clone()
                .downcast()
                .expect(TYPE_LOGIC_ERROR)),
            dashmap::Entry::Vacant(vacant_entry) => {
                let v = f().await?;
                Ok(vacant_entry
                    .insert(Arc::new(v))
                    .clone()
                    .downcast()
                    .expect(TYPE_LOGIC_ERROR))
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

    #[test]
    fn test_common_resource() {
        let resource = TypeMap::new();

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
