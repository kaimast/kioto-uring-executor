#![feature(trait_alias)]

use std::future::Future;
use std::sync::mpsc as std_mpsc;

#[cfg(feature = "macros")]
pub use kioto_uring_executor_macros::{main, test};

mod runtime;
pub use runtime::{FutureWith, JoinHandle, Runtime, SpawnRing};

use runtime::ACTIVE_RUNTIME;

/// Emulates tokio's block_on call
///
/// This will spawn a new tokio runtime on the current thread
pub fn block_on<T: 'static, F: Future<Output = T> + 'static>(task: F) -> T {
    tokio_uring::start(task)
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

/// Emulates tokio's block_on call
///
/// This will use an existing exeuctor
pub fn block_on_runtime<T: Send + 'static, F: Future<Output = T> + Send + 'static>(task: F) -> T {
    ACTIVE_RUNTIME.with_borrow(|r| r.as_ref().expect("No active runtime").block_on(task))
}

/// Emulates tokio's block_on call
///
/// # Safety
/// Make sure task is Send before polled for the first time
/// (Can be not Send afterwards)
pub unsafe fn unsafe_block_on_runtime<T: 'static, F: Future<Output = T> + 'static>(task: F) -> T {
    let (sender, receiver) = std_mpsc::channel();

    unsafe_spawn(async move {
        let res = task.await;
        sender.send(res).expect("Notification failed");
    });

    receiver.recv().expect("Failed to wait for task")
}

/// # Safety
///
/// Make sure task is Send before polled for the first time
/// (Can be not Send afterwards)
pub unsafe fn unsafe_spawn<O: Send + 'static, F: Future<Output = O> + 'static>(
    task: F,
) -> JoinHandle<O> {
    ACTIVE_RUNTIME.with_borrow(|r| r.as_ref().expect("No active runtime").unsafe_spawn(task))
}

/// # Safety
///
/// Make sure task is Send before polled for the first time
/// (Can be not Send afterwards)
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
