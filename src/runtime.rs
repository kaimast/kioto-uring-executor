use std::cell::RefCell;
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::Arc;

use parking_lot::RwLock;

use rand::Rng;

use tokio::sync::mpsc;

pub struct Task {
    future: Pin<Box<dyn Future<Output = ()> + 'static>>,
}

unsafe impl Send for Task {}

pub type TaskSender = mpsc::UnboundedSender<Task>;

thread_local! {
    pub(super) static ACTIVE_RUNTIME: RefCell<Option<Arc<RuntimeInner>>> = const { RefCell::new(None) };
}

pub(super) struct RuntimeInner {
    task_senders: RwLock<Vec<TaskSender>>,
}

pub struct Runtime {
    inner: Arc<RuntimeInner>,
}

pub struct SpawnRing {
    inner: Arc<RuntimeInner>,
    thread_idx: usize,
}

impl SpawnRing {
    pub fn get(&self) -> usize {
        self.thread_idx
    }

    pub fn advance(&mut self) {
        let num_worker_threads = self.inner.get_num_threads();
        self.thread_idx = (self.thread_idx + 1) % num_worker_threads;
    }
}

impl Runtime {
    pub fn new() -> Self {
        let thread_count = std::thread::available_parallelism().unwrap();
        Self::new_with_threads(thread_count)
    }

    pub fn new_with_threads(num_os_threads: NonZeroUsize) -> Self {
        let num_os_threads = num_os_threads.get();
        log::info!("Initialized tokio runtime with {num_os_threads} worker thread(s)");
        let inner = Arc::new(RuntimeInner {
            task_senders: Default::default(),
        });

        for _ in 0..num_os_threads {
            let (sender, mut receiver) = mpsc::unbounded_channel::<Task>();
            inner.task_senders.write().push(sender);
            let inner = inner.clone();

            std::thread::spawn(move || {
                ACTIVE_RUNTIME.with_borrow_mut(|r| {
                    *r = Some(inner);
                });
                tokio_uring::start(async {
                    while let Some(task) = receiver.recv().await {
                        tokio_uring::spawn(task.future);
                    }
                });
            });
        }

        Self { inner }
    }

    /// Create a primitive that lets you distribute tasks
    /// across worker threads in a round-robin fashion
    pub fn ring(&self) -> SpawnRing {
        SpawnRing {
            thread_idx: 0,
            inner: self.inner.clone(),
        }
    }

    /// Spawns the task on a random thread
    pub fn spawn<F: Future<Output = ()> + Send + 'static>(&self, task: F) {
        self.inner.spawn(task)
    }

    /// How many worker threads are there?
    pub fn get_num_threads(&self) -> usize {
        self.inner.get_num_threads()
    }

    /// Spawns the task on a specific thread
    pub fn spawn_at<F: Future<Output = ()> + Send + 'static>(&self, offset: usize, task: F) {
        self.inner.spawn_at(offset, task)
    }

    /// # Safety
    ///
    /// Make sure task is Send before polled for the first time
    /// (Can be not Send afterwards)
    pub unsafe fn unsafe_spawn_at<F: Future<Output = ()> + 'static>(&self, offset: usize, task: F) {
        self.inner.unsafe_spawn_at(offset, task)
    }

    /// # Safety
    ///
    /// Make sure task is Send before polled for the first time
    /// (Can be not Send afterwards)
    pub unsafe fn unsafe_spawn<F: Future<Output = ()> + 'static>(&self, task: F) {
        self.inner.unsafe_spawn(task)
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        *self.inner.task_senders.write() = vec![];
    }
}

impl RuntimeInner {
    pub fn spawn<F: Future<Output = ()> + Send + 'static>(&self, task: F) {
        let task = Task {
            future: Box::pin(task),
        };

        let senders = self.task_senders.read();
        if senders.is_empty() {
            panic!("Executor not set up yet!");
        }

        let idx = rand::thread_rng().gen_range(0..senders.len());
        if let Err(err) = senders[idx].send(task) {
            panic!("Failed to spawn task: {err}");
        }
    }

    /// Spawns the task on a specific thread
    pub fn spawn_at<F: Future<Output = ()> + Send + 'static>(&self, offset: usize, task: F) {
        let task = Task {
            future: Box::pin(task),
        };

        let senders = self.task_senders.read();
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
    pub unsafe fn unsafe_spawn_at<F: Future<Output = ()> + 'static>(&self, offset: usize, task: F) {
        let task = Task {
            future: Box::pin(task),
        };

        let senders = self.task_senders.read();
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
    pub unsafe fn unsafe_spawn<F: Future<Output = ()> + 'static>(&self, task: F) {
        let task = Task {
            future: Box::pin(task),
        };

        let senders = self.task_senders.read();
        if senders.is_empty() {
            panic!("Executor not set up yet!");
        }

        let idx = rand::thread_rng().gen_range(0..senders.len());
        if let Err(err) = senders[idx].send(task) {
            panic!("Failed to spawn task: {err}");
        }
    }

    pub fn get_num_threads(&self) -> usize {
        let senders = self.task_senders.read();
        if senders.is_empty() {
            panic!("No active kioto runtime")
        }

        senders.len()
    }
}
