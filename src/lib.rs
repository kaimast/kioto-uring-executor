#![feature(trait_alias)]

#[cfg(all(feature = "monoio", feature = "tokio-uring"))]
compile_error!("Cannot enable monoio and tokio-uring");

#[cfg(not(any(feature = "monoio", feature = "tokio-uring")))]
compile_error!("Must enable either the 'monoio' or 'tokio-uring' feature");

#[cfg(feature = "monoio")]
pub use monoio::{net, time};

#[cfg(feature = "tokio-uring")]
pub use tokio_uring::net;

#[cfg(feature = "tokio-uring")]
pub use tokio::time;

use std::future::Future;

#[cfg(feature = "macros")]
pub use kioto_uring_executor_macros::{main, test};

mod runtime;
pub use runtime::{FutureWith, JoinHandle, LocalJoinHandle, Runtime};

mod spawn;
pub use spawn::*;

use runtime::ACTIVE_RUNTIME;

#[cfg(feature = "monoio")]
fn generate_runtime() -> monoio::Runtime<monoio::time::TimeDriver<monoio::IoUringDriver>> {
    monoio::RuntimeBuilder::<monoio::IoUringDriver>::new()
        .enable_timer()
        .build()
        .expect("Failed to start monoio")
}

/// Emulates tokio's block_on call
///
/// This will spawn a new runtime on the current thread
pub fn block_on<T: 'static, F: Future<Output = T> + 'static>(task: F) -> T {
    Runtime::block_on_current_thread(task)
}

/// Get a handle to the currently active runtime (if any)
pub fn runtime_handle() -> Option<runtime::Handle> {
    ACTIVE_RUNTIME.with_borrow(|r| {
        r.as_ref().map(|inner| runtime::Handle {
            inner: inner.clone(),
        })
    })
}

/// Emulates tokio's block_on call
///
/// This will use an existing exeuctor
pub fn block_on_runtime<O: Send + 'static, F: Future<Output = O> + Send + 'static>(task: F) -> O {
    runtime_handle().expect("No active runtime").block_on(task)
}

pub fn get_runtime_thread_count() -> usize {
    runtime_handle()
        .expect("No active runtime")
        .get_thread_count()
}
