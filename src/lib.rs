#![feature(trait_alias)]

use std::future::Future;

#[cfg(feature = "macros")]
pub use kioto_uring_executor_macros::{main, test};

mod runtime;
pub use runtime::{FutureWith, JoinHandle, Runtime};

mod spawn;
pub use spawn::*;

use runtime::ACTIVE_RUNTIME;

/// Emulates tokio's block_on call
///
/// This will spawn a new tokio runtime on the current thread
pub fn block_on<T: 'static, F: Future<Output = T> + 'static>(task: F) -> T {
    tokio_uring::start(task)
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
#[deprecated]
pub unsafe fn unsafe_block_on_runtime<T: 'static, F: Future<Output = T> + 'static>(task: F) -> T {
    let (sender, receiver) = std::sync::mpsc::channel();

    #[allow(deprecated)]
    unsafe_spawn(async move {
        let res = task.await;
        sender.send(res).expect("Notification failed");
    });

    receiver.recv().expect("Failed to wait for task")
}
