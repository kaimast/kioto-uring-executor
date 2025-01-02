/// Manages multiple tokio-uring runtimes for multi-threading
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::mpsc as std_mpsc;

use parking_lot::RwLock;

use rand::Rng;

use tokio::sync::mpsc;

pub use kioto_uring_executor_macros::test;

pub struct Task {
    future: Pin<Box<dyn Future<Output = ()> + 'static>>,
}

unsafe impl Send for Task {}

pub type TaskSender = mpsc::UnboundedSender<Task>;

static TASK_SENDERS: RwLock<Vec<TaskSender>> = const { RwLock::new(vec![]) };

pub struct SpawnPos {
    thread_idx: usize,
}

impl Default for SpawnPos {
    fn default() -> Self {
        Self::new()
    }
}

impl SpawnPos {
    pub fn new() -> Self {
        Self { thread_idx: 0 }
    }

    pub fn get(&self) -> usize {
        self.thread_idx
    }

    pub fn advance(&mut self) {
        let num_worker_threads = get_num_threads();
        self.thread_idx = (self.thread_idx + 1) % num_worker_threads;
    }
}

pub fn get_num_threads() -> usize {
    let senders = TASK_SENDERS.read();
    if senders.is_empty() {
        panic!("No active kioto runtime")
    }

    senders.len()
}

pub fn initialize() {
    let thread_count = std::thread::available_parallelism().unwrap();
    initialize_with_threads(thread_count)
}

pub fn initialize_with_threads(num_os_threads: NonZeroUsize) {
    let num_os_threads = num_os_threads.get();
    let mut task_senders = TASK_SENDERS.write();
    if !task_senders.is_empty() {
        panic!("Tokio runtime already set up!");
    }

    log::info!("Initialized tokio runtime with {num_os_threads} worker thread(s)");

    for _ in 0..num_os_threads {
        let (sender, mut receiver) = mpsc::unbounded_channel::<Task>();
        task_senders.push(sender);

        std::thread::spawn(move || {
            tokio_uring::start(async {
                while let Some(task) = receiver.recv().await {
                    tokio_uring::spawn(task.future);
                }
            });
        });
    }
}

/// Emulates tokio's block_on call
///
/// This will spawn a new tokio runtime on the current thread
pub fn block_on<T: 'static, F: Future<Output = T> + 'static>(task: F) -> T {
    tokio_uring::start(task)
}

/// Emulates tokio's block_on call
///
/// This will use an existing exeuctor
pub fn block_on_executor<T: Send + 'static, F: Future<Output = T> + Send + 'static>(task: F) -> T {
    let (sender, receiver) = std_mpsc::channel();

    spawn(async move {
        let res = task.await;
        sender.send(res).expect("Notification failed");
    });

    receiver.recv().expect("Failed to wait for task")
}

/// Emulates tokio's block_on call
///
/// # Safety
/// Make sure task is Send before polled for the first time
/// (Can be not Send afterwards)
pub unsafe fn unsafe_block_on_executor<T: Send + 'static, F: Future<Output = T> + 'static>(
    task: F,
) -> T {
    let (sender, receiver) = std_mpsc::channel();

    unsafe_spawn(async move {
        let res = task.await;
        sender.send(res).expect("Notification failed");
    });

    receiver.recv().expect("Failed to wait for task")
}

/// Spawns the task on a random thread
pub fn spawn<F: Future<Output = ()> + Send + 'static>(task: F) {
    let task = Task {
        future: Box::pin(task),
    };

    let senders = TASK_SENDERS.read();
    if senders.is_empty() {
        panic!("Executor not set up yet!");
    }

    let idx = rand::thread_rng().gen_range(0..senders.len());
    if let Err(err) = senders[idx].send(task) {
        panic!("Failed to spawn task: {err}");
    }
}

/// Spawns the task on a specific thread
pub fn spawn_at<F: Future<Output = ()> + Send + 'static>(offset: usize, task: F) {
    let task = Task {
        future: Box::pin(task),
    };

    let senders = TASK_SENDERS.read();
    if senders.is_empty() {
        panic!("Executor not set up yet!");
    }

    let idx = offset % senders.len();
    if let Err(err) = senders[idx].send(task) {
        panic!("Failed to spawn task: {err}");
    }
}

/// # Safety
///
/// Make sure task is Send before polled for the first time
/// (Can be not Send afterwards)
pub unsafe fn unsafe_spawn_at<F: Future<Output = ()> + 'static>(offset: usize, task: F) {
    let task = Task {
        future: Box::pin(task),
    };

    let senders = TASK_SENDERS.read();
    if senders.is_empty() {
        panic!("Executor not set up yet!");
    }

    let idx = offset % senders.len();
    if let Err(err) = senders[idx].send(task) {
        panic!("Failed to spawn task: {err}");
    }
}

/// # Safety
///
/// Make sure task is Send before polled for the first time
/// (Can be not Send afterwards)
pub unsafe fn unsafe_spawn<F: Future<Output = ()> + 'static>(task: F) {
    let task = Task {
        future: Box::pin(task),
    };

    let senders = TASK_SENDERS.read();
    if senders.is_empty() {
        panic!("Executor not set up yet!");
    }

    let idx = rand::thread_rng().gen_range(0..senders.len());
    if let Err(err) = senders[idx].send(task) {
        panic!("Failed to spawn task: {err}");
    }
}
