use std::future::Future;
use std::pin::Pin;

use tokio::sync::mpsc;

trait Generator = (FnOnce() -> Pin<Box<dyn Future<Output = ()> + 'static>>) + Send;

pub(super) struct Task {
    pub(super) generator: Box<dyn Generator>,
}

/// Used to send tasks to the worker threads
/// The runtime sends `None`, if we are shutting down
pub(super) type TaskSender = mpsc::UnboundedSender<Option<Task>>;

pub(super) type TaskReceiver = mpsc::UnboundedReceiver<Option<Task>>;

pub(super) fn create_task_channel() -> (TaskSender, TaskReceiver) {
    mpsc::unbounded_channel()
}
