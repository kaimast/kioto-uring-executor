use std::future::Future;
use std::sync::mpsc as std_mpsc;

pub use kioto_uring_executor_macros::test;

mod runtime;
pub use runtime::Runtime;

use runtime::ACTIVE_RUNTIME;

/// Emulates tokio's block_on call
///
/// This will spawn a new tokio runtime on the current thread
pub fn block_on<T: 'static, F: Future<Output = T> + 'static>(task: F) -> T {
    tokio_uring::start(task)
}

/// Spawns the task on a random thread
pub fn spawn<F: Future<Output = ()> + Send + 'static>(task: F) {
    ACTIVE_RUNTIME.with_borrow(|r| r.as_ref().expect("No active runtime").spawn(task))
}

/// Spawns the task on a specific thread
pub fn spawn_at<F: Future<Output = ()> + Send + 'static>(offset: usize, task: F) {
    ACTIVE_RUNTIME.with_borrow(|r| {
        r.as_ref()
            .expect("No active runtime")
            .spawn_at(offset, task)
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
pub unsafe fn unsafe_block_on_runtime<T: Send + 'static, F: Future<Output = T> + 'static>(
    task: F,
) -> T {
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
pub unsafe fn unsafe_spawn<F: Future<Output = ()> + 'static>(task: F) {
    ACTIVE_RUNTIME.with_borrow(|r| r.as_ref().expect("No active runtime").unsafe_spawn(task))
}

/// # Safety
///
/// Make sure task is Send before polled for the first time
/// (Can be not Send afterwards)
pub unsafe fn unsafe_spawn_at<F: Future<Output = ()> + Send + 'static>(offset: usize, task: F) {
    ACTIVE_RUNTIME.with_borrow(|r| {
        r.as_ref()
            .expect("No active runtime")
            .unsafe_spawn_at(offset, task)
    })
}
