#![feature(trait_alias)]

#[cfg(all(feature = "monoio", feature = "tokio-uring"))]
compile_error!("Cannot enable monoio and tokio-uring");

#[cfg(not(any(feature = "monoio", feature = "tokio-uring")))]
compile_error!("Must enable either the 'monoio' or 'tokio-uring' feature");

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
    cfg_if::cfg_if! {
    if #[cfg(feature="tokio-uring")] {
        tokio_uring::start(task)
    } else {
        monoio::start::<monoio::IoUringDriver, _>(task)
    }
    }
}

/// Emulates tokio's block_on call
///
/// This will use an existing exeuctor
pub fn block_on_runtime<O: Send + 'static, F: Future<Output = O> + Send + 'static>(task: F) -> O {
    ACTIVE_RUNTIME.with_borrow(|r| r.as_ref().expect("No active runtime").block_on(task))
}

pub fn get_runtime_thread_count() -> usize {
    ACTIVE_RUNTIME.with_borrow(|r| r.as_ref().expect("No active runtime").get_thread_count())
}
