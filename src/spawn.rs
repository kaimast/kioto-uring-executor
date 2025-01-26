use std::future::Future;
use std::sync::Arc;

use crate::runtime::{FutureWith, JoinHandle, RuntimeInner, ACTIVE_RUNTIME};

pub struct SpawnRing {
    inner: Arc<RuntimeInner>,
    thread_idx: usize,
}

impl SpawnRing {
    pub(super) fn new(inner: Arc<RuntimeInner>) -> Self {
        Self {
            inner,
            thread_idx: 0,
        }
    }

    pub fn spawn<O: Send + Sized, F: Future<Output = O> + Send + 'static>(
        &mut self,
        func: F,
    ) -> JoinHandle<O> {
        let hdl = self.inner.spawn_at(self.thread_idx, func);
        self.advance();
        hdl
    }

    pub fn spawn_with<O: Send + Sized, F: FutureWith<O>>(&mut self, func: F) -> JoinHandle<O> {
        let hdl = self.inner.spawn_with_at(self.thread_idx, func);
        self.advance();
        hdl
    }

    fn advance(&mut self) {
        let num_worker_threads = self.inner.get_num_threads();
        self.thread_idx = (self.thread_idx + 1) % num_worker_threads;
    }
}

/// Create a primitive that lets you distribute tasks
/// across worker threads in a round-robin fashion
pub fn new_spawn_ring() -> SpawnRing {
    ACTIVE_RUNTIME.with_borrow(|r| {
        let inner = r.as_ref().expect("No active runtime").clone();

        SpawnRing::new(inner)
    })
}

/// Spawns the task on a random thread
pub fn spawn<O: Send + Sized + 'static, F: Future<Output = O> + Send + 'static>(
    func: F,
) -> JoinHandle<O> {
    ACTIVE_RUNTIME.with_borrow(|r| r.as_ref().expect("No active runtime").spawn(func))
}

/// Spawns the task on the current thread
pub fn spawn_local<O: Send + Sized + 'static, F: Future<Output = O> + 'static>(
    func: F,
) -> JoinHandle<O> {
    ACTIVE_RUNTIME.with_borrow(|r| r.as_ref().expect("No active runtime").spawn_local(func))
}

/// Spawns the task on a random thread
pub fn spawn_with<O: Send + Sized + 'static, F: FutureWith<O>>(func: F) -> JoinHandle<O> {
    ACTIVE_RUNTIME.with_borrow(|r| r.as_ref().expect("No active runtime").spawn_with(func))
}

/// Spawns the task on a specific thread
pub fn spawn_at<O: Send + Sized + 'static, F: Future<Output = O> + Send + 'static>(
    offset: usize,
    func: F,
) -> JoinHandle<O> {
    ACTIVE_RUNTIME.with_borrow(|r| {
        r.as_ref()
            .expect("No active runtime")
            .spawn_at(offset, func)
    })
}

/// # Safety
///
/// Make sure task is Send before polled for the first time
/// (Can be not Send afterwards)
#[deprecated]
pub unsafe fn unsafe_spawn<O: Send + 'static, F: Future<Output = O> + 'static>(
    task: F,
) -> JoinHandle<O> {
    ACTIVE_RUNTIME.with_borrow(|r| r.as_ref().expect("No active runtime").unsafe_spawn(task))
}

/// # Safety
///
/// Make sure task is Send before polled for the first time
/// (Can be not Send afterwards)
#[deprecated]
pub unsafe fn unsafe_spawn_at<O: Send + Sized + 'static, F: Future<Output = O> + 'static>(
    offset: usize,
    task: F,
) -> JoinHandle<O> {
    ACTIVE_RUNTIME.with_borrow(|r| {
        r.as_ref()
            .expect("No active runtime")
            .unsafe_spawn_at(offset, task)
    })
}
