use std::future::Future;
use std::sync::Arc;

use crate::runtime::{FutureWith, JoinHandle, LocalJoinHandle, RuntimeInner, ACTIVE_RUNTIME};

/// A spawn ring allows to load balance tasks across all
/// worker threads in a round-robin fashion
/// At every spawn call it will move to the next worker thread
pub struct SpawnRing {
    inner: Arc<RuntimeInner>,
    thread_idx: usize,
    wrapped: bool,
}

impl SpawnRing {
    pub(super) fn new(inner: Arc<RuntimeInner>) -> Self {
        Self {
            inner,
            thread_idx: 0,
            wrapped: false,
        }
    }

    // Have we spawned a task on all threads already and
    // the position got reset to zero?
    pub fn has_wrapped(&self) -> bool {
        self.wrapped
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
        self.thread_idx += 1;

        if self.thread_idx >= self.inner.get_thread_count() {
            self.wrapped = true;
            self.thread_idx = 0;
        }
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
    spawn_with(|| Box::pin(func))
}

/// Spawns the task on the current thread
pub fn spawn_local<O: Sized + 'static, F: Future<Output = O> + 'static>(
    func: F,
) -> LocalJoinHandle<O> {
    ACTIVE_RUNTIME.with_borrow(|r| r.as_ref().expect("No active runtime").spawn_local(func))
}

/// Spawns the task on a random thread (non-send version)
pub fn spawn_with<O: Send + Sized + 'static, F: FutureWith<O>>(func: F) -> JoinHandle<O> {
    ACTIVE_RUNTIME.with_borrow(|r| r.as_ref().expect("No active runtime").spawn_with(func))
}

/// Spawns the task on the given thread index (non-send version)
pub fn spawn_with_at<O: Send + Sized + 'static, F: FutureWith<O>>(
    offset: usize,
    func: F,
) -> JoinHandle<O> {
    ACTIVE_RUNTIME.with_borrow(|r| {
        r.as_ref()
            .expect("No active runtime")
            .spawn_with_at(offset, func)
    })
}

/// Spawns the task on a specific thread
pub fn spawn_at<O: Send + Sized + 'static, F: Future<Output = O> + Send + 'static>(
    offset: usize,
    func: F,
) -> JoinHandle<O> {
    spawn_with_at(offset, || Box::pin(func))
}
