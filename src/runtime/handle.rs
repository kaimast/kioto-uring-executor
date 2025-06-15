use std::future::Future;
use std::sync::Arc;

use crate::spawn::SpawnRing;
use crate::{FutureWith, JoinHandle, LocalJoinHandle};

use super::{Error, RuntimeInner};

/// A reference to an existing kioto runtime
pub struct Handle {
    inner: Arc<RuntimeInner>,
}

impl Handle {
    pub(crate) fn new(inner: Arc<RuntimeInner>) -> Self {
        Self { inner }
    }

    /// Blocks the current thread until the runtime has finished th task
    pub fn block_on<T: Send + 'static, F: Future<Output = T> + Send + 'static>(
        &self,
        task: F,
    ) -> Result<T, Error> {
        self.inner.block_on(task)
    }

    /// Blocks the current thread until the runtime has finished th task
    pub fn block_on_with<T: Send + 'static, F: FutureWith<T> + 'static>(
        &self,
        fut: F,
    ) -> Result<T, Error> {
        self.inner.block_on_with(fut)
    }

    /// Spawns the task on a random thread
    pub fn spawn<O: Send + Sized, F: Future<Output = O> + Send + 'static>(
        &self,
        func: F,
    ) -> JoinHandle<O> {
        self.inner.spawn(func)
    }

    /// Spawns the task on the current thread
    pub fn spawn_local<O: Sized, F: Future<Output = O> + 'static>(
        &self,
        func: F,
    ) -> LocalJoinHandle<O> {
        self.inner.spawn_local(func)
    }

    /// Spawns the task on a random thread
    ///
    /// This allows to send some data even if the resulting
    /// future is not Sync
    pub fn spawn_with<O: Send + Sized + 'static, F: FutureWith<O>>(
        &self,
        func: F,
    ) -> JoinHandle<O> {
        self.inner.spawn_with(func)
    }

    /// How many worker threads are there?
    pub fn get_thread_count(&self) -> usize {
        self.inner.get_thread_count()
    }

    /// Create a primitive that lets you distribute tasks
    /// across worker threads in a round-robin fashion
    pub fn new_spawn_ring(&self) -> SpawnRing {
        SpawnRing::new(self.inner.clone())
    }
}
