#[cfg(feature = "monoio")]
pub(super) use monoio::task::JoinHandle as InnerJoinHandle;
#[cfg(feature = "tokio-uring")]
use tokio::task::JoinHandle as InnerJoinHandle;

pub(super) type JoinHandleReceiver<O> = tokio::sync::oneshot::Receiver<O>;

use crate::error::{Error, Result};

pub struct JoinHandle<O: Sized + Send + 'static> {
    /// will be `None` when spawning a task during shutdown
    receiver: Option<JoinHandleReceiver<O>>,
}

/// A local join handle is different from a cross-thread
/// JoinHandle in that it is not Send and can be aborted
pub struct LocalJoinHandle<O: Sized + 'static> {
    inner: InnerJoinHandle<O>,
}

impl<O: Sized> LocalJoinHandle<O> {
    pub(super) fn new(inner: InnerJoinHandle<O>) -> Self {
        Self { inner }
    }

    #[cfg(feature = "tokio-uring")]
    pub fn abort(self) {
        self.inner.abort()
    }

    pub async fn join(self) -> Result<O> {
        #[cfg(feature = "tokio-uring")]
        {
            self.inner.await.map_err(|err| Error::TaskFailed {
                message: err.to_string(),
            })
        }
        #[cfg(feature = "monoio")]
        {
            Ok(self.inner.await)
        }
    }
}

impl<O: Sized + Send> JoinHandle<O> {
    pub(super) fn new_aborted() -> Self {
        Self { receiver: None }
    }

    /// Constructor (only used by the Runtime internally)
    pub(super) fn new(receiver: JoinHandleReceiver<O>) -> Self {
        Self {
            receiver: Some(receiver),
        }
    }

    /// Block until the associated trorask finished
    pub async fn join(self) -> Result<O> {
        let Some(receiver) = self.receiver else {
            return Err(Error::ShuttingDown);
        };

        receiver.await.map_err(|err| Error::TaskFailed {
            message: err.to_string(),
        })
    }
}
