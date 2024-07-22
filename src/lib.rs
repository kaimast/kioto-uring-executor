/// Manages multiple tokio-uring runtimes for multi-threading

use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;

use rand::Rng;

use tokio::sync::mpsc;

use std::sync::mpsc as std_mpsc;

pub use kioto_executor_macros::test;

pub struct Task {
    future: Pin<Box<dyn Future<Output = ()> + 'static>>,
}

unsafe impl Send for Task {}

pub type TaskSender = mpsc::UnboundedSender<Task>;

const MIN_EXECUTOR_THREADS: usize = 8;
static mut TASK_SENDERS: Vec<TaskSender> = vec![];

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
        let num_os_threads = std::thread::available_parallelism()
            .unwrap()
            .get()
            .max(MIN_EXECUTOR_THREADS);
        self.thread_idx = (self.thread_idx + 1) % num_os_threads;
    }
}

pub fn get_num_threads() -> usize {
    unsafe { TASK_SENDERS.len() }
}

pub fn initialize() {
    let thread_count = std::thread::available_parallelism().unwrap();
    initialize_with_threads(thread_count)
}

pub fn initialize_with_threads(num_os_threads: NonZeroUsize) {
    let num_os_threads = num_os_threads.get().max(MIN_EXECUTOR_THREADS);

    log::info!("Initialized tokio runtime with {num_os_threads} worker thread(s)");

    let mut task_senders = Vec::with_capacity(num_os_threads);

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

    unsafe {
        TASK_SENDERS = task_senders;
    }
}

/// Emulates tokio's block_on call
pub fn block_on<F: Future<Output = ()> + Send + 'static>(task: F) {
    let (sender, receiver) = std_mpsc::channel();

    spawn(async move {
        task.await;
        sender.send(()).expect("Notification failed");
    });

    receiver.recv().expect("Failed to wait for task");
}

/// Emulates tokio's block_on call
///
/// # Safety
/// Make sure task is Send before polled for the first time
/// (Can be not Send afterwards)

pub unsafe fn unsafe_block_on<F: Future<Output = ()> + 'static>(task: F) {
    let (sender, receiver) = std_mpsc::channel();

    unsafe_spawn(async move {
        task.await;
        sender.send(()).expect("Notification failed");
    });

    receiver.recv().expect("Failed to wait for task");
}

/// Spawns the task on a random thread
pub fn spawn<F: Future<Output = ()> + Send + 'static>(task: F) {
    let task = Task {
        future: Box::pin(task),
    };

    unsafe {
        if TASK_SENDERS.is_empty() {
            panic!("Executor not set up yet!");
        }

        let idx = rand::thread_rng().gen_range(0..TASK_SENDERS.len());
        if let Err(err) = TASK_SENDERS[idx].send(task) {
            panic!("Failed to spawn task: {err}");
        }
    }
}

/// Spawns the task on a specific thread
pub fn spawn_at<F: Future<Output = ()> + Send + 'static>(offset: usize, task: F) {
    let task = Task {
        future: Box::pin(task),
    };

    unsafe {
        if TASK_SENDERS.is_empty() {
            panic!("Executor not set up yet!");
        }

        let idx = offset % TASK_SENDERS.len();
        if let Err(err) = TASK_SENDERS[idx].send(task) {
            panic!("Failed to spawn task: {err}");
        }
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

    unsafe {
        if TASK_SENDERS.is_empty() {
            panic!("Executor not set up yet!");
        }

        let idx = offset % TASK_SENDERS.len();
        if let Err(err) = TASK_SENDERS[idx].send(task) {
            panic!("Failed to spawn task: {err}");
        }
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

    unsafe {
        if TASK_SENDERS.is_empty() {
            panic!("Executor not set up yet!");
        }

        let idx = rand::thread_rng().gen_range(0..TASK_SENDERS.len());
        if let Err(err) = TASK_SENDERS[idx].send(task) {
            panic!("Failed to spawn task: {err}");
        }
    }
}
