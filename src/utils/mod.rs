use bevy_ecs::resource::Resource;
use common::utils::debug::SmartRwLock;
use std::sync::Arc;

#[derive(Resource, Clone)]
pub struct Shared<T> {
    inner: Arc<SmartRwLock<T>>,
}

impl<T> Shared<T> {
    pub fn new(inner: Arc<SmartRwLock<T>>) -> Self {
        Self { inner }
    }

    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, T> {
        self.inner.read()
    }

    pub fn write(&self) -> parking_lot::RwLockWriteGuard<'_, T> {
        self.inner.write()
    }

    pub fn clone_inner(&self) -> Arc<SmartRwLock<T>> {
        self.inner.clone()
    }
}
