use std::cell::RefCell;
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;

use parking_lot::RwLock;

use rand::Rng;

use tokio::sync::mpsc;

pub struct Task {
    future: Pin<Box<dyn Future<Output = ()> + 'static>>,
}

unsafe impl Send for Task {}

type TaskSender = mpsc::UnboundedSender<Task>;

thread_local! {
    pub(super) static ACTIVE_RUNTIME: RefCell<Option<Arc<RuntimeInner>>> = const { RefCell::new(None) };
}

pub struct JoinHandle<O: Sized + Send + 'static> {
    receiver: tokio::sync::oneshot::Receiver<O>,
}

impl<O: Send> JoinHandle<O> {
    /// Block until the associated task finished
    pub async fn join(self) -> O {
        self.receiver.await.unwrap()
    }
}

pub(super) struct RuntimeInner {
    task_senders: RwLock<Vec<TaskSender>>,
}

pub struct Runtime {
    inner: Arc<RuntimeInner>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SpawnRing {
    inner: Arc<RuntimeInner>,
    thread_idx: usize,
}

impl SpawnRing {
    pub(super) fn new(inner: Arc<RuntimeInner>) -> Self {
        Self {
            inner,
            thread_idx: 0,
        }
    }

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

    /// Blocks the current thread until the runtime has finished th task
    pub fn block_on<T: Send + 'static, F: Future<Output = T> + Send + 'static>(
        &self,
        task: F,
    ) -> T {
        self.inner.block_on(task)
    }

    /// Blocks the current thread until the runtime has finished th task (unsafe version)
    ///
    /// # Safety
    /// Make sure task is Send before polled for the first time
    /// (Can be not Send afterwards)
    pub unsafe fn unsafe_block_on<T: Send + 'static, F: Future<Output = T> + 'static>(
        &self,
        task: F,
    ) -> T {
        self.inner.unsafe_block_on(task)
    }

    /// Spawns the task on a random thread
    pub fn spawn<O: Send + Sized, F: Future<Output = O> + Send + 'static>(
        &self,
        task: F,
    ) -> JoinHandle<O> {
        self.inner.spawn(task)
    }

    /// How many worker threads are there?
    pub fn get_num_threads(&self) -> usize {
        self.inner.get_num_threads()
    }

    /// Spawns the task on a specific thread
    pub fn spawn_at<O: Sized + Send + 'static, F: Future<Output = O> + Send + 'static>(
        &self,
        offset: usize,
        task: F,
    ) -> JoinHandle<O> {
        self.inner.spawn_at(offset, task)
    }

    /// # Safety
    ///
    /// Make sure task is Send before polled for the first time
    /// (Can be not Send afterwards)
    pub unsafe fn unsafe_spawn_at<O: Sized + Send + 'static, F: Future<Output = O> + 'static>(
        &self,
        offset: usize,
        task: F,
    ) -> JoinHandle<O> {
        self.inner.unsafe_spawn_at(offset, task)
    }

    /// # Safety
    ///
    /// Make sure task is Send before polled for the first time
    /// (Can be not Send afterwards)
    pub unsafe fn unsafe_spawn<O: Sized + Send + 'static, F: Future<Output = O> + 'static>(
        &self,
        task: F,
    ) -> JoinHandle<O> {
        self.inner.unsafe_spawn(task)
    }

    /// Create a primitive that lets you distribute tasks
    /// across worker threads in a round-robin fashion
    pub fn new_spawn_ring(&self) -> SpawnRing {
        SpawnRing::new(self.inner.clone())
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        *self.inner.task_senders.write() = vec![];
    }
}

impl RuntimeInner {
    fn wrap_function<O: Send + 'static, F: Future<Output = O> + Send + 'static>(
        func: F,
    ) -> (Task, JoinHandle<O>) {
        let (sender, receiver) = tokio::sync::oneshot::channel();

        let task = Task {
            future: Box::pin(async move {
                let result = func.await;
                let _ = sender.send(result);
            }),
        };

        let hdl = JoinHandle { receiver };

        (task, hdl)
    }

    unsafe fn unsafe_wrap_function<O: Send + Sized + 'static, F: Future<Output = O> + 'static>(
        func: F,
    ) -> (Task, JoinHandle<O>) {
        let (sender, receiver) = tokio::sync::oneshot::channel();

        let task = Task {
            future: Box::pin(async move {
                let result = func.await;
                let _ = sender.send(result);
            }),
        };

        let hdl = JoinHandle { receiver };

        (task, hdl)
    }

    pub fn spawn<O: Sized + Send + 'static, F: Future<Output = O> + Send + 'static>(
        &self,
        func: F,
    ) -> JoinHandle<O> {
        let (task, hdl) = Self::wrap_function(func);

        let senders = self.task_senders.read();
        if senders.is_empty() {
            panic!("Executor not set up yet!");
        }

        let idx = rand::thread_rng().gen_range(0..senders.len());
        if let Err(err) = senders[idx].send(task) {
            panic!("Failed to spawn task: {err}");
        }

        hdl
    }

    /// Spawns the task on a specific thread
    pub fn spawn_at<O: Send + 'static, F: Future<Output = O> + Send + 'static>(
        &self,
        offset: usize,
        func: F,
    ) -> JoinHandle<O> {
        let (task, hdl) = Self::wrap_function(func);

        let senders = self.task_senders.read();
        if senders.is_empty() {
            panic!("Executor not set up yet!");
        }

        let idx = offset % senders.len();
        if let Err(err) = senders[idx].send(task) {
            panic!("Failed to spawn task: {err}");
        }

        hdl
    }

    /// Blocks the current thread until the runtime has finished th task
    pub fn block_on<T: Send + 'static, F: Future<Output = T> + Send + 'static>(
        &self,
        task: F,
    ) -> T {
        let (sender, receiver) = std_mpsc::channel();

        self.spawn(async move {
            let res = task.await;
            sender.send(res).expect("Notification failed");
        });

        receiver.recv().expect("Failed to wait for task")
    }

    /// Blocks the current thread until the runtime has finished th task (unsafe version)
    ///
    /// # Safety
    /// Make sure task is Send before polled for the first time
    /// (Can be not Send afterwards)
    pub unsafe fn unsafe_block_on<T: Send + 'static, F: Future<Output = T> + 'static>(
        &self,
        task: F,
    ) -> T {
        let (sender, receiver) = std_mpsc::channel();

        self.unsafe_spawn(async move {
            let res = task.await;
            sender.send(res).expect("Notification failed");
        });

        receiver.recv().expect("Failed to wait for task")
    }

    /// # Safety
    ///
    /// Make sure task is Send before polled for the first time
    /// (Can be not Send afterwards)
    pub unsafe fn unsafe_spawn_at<O: Send + Sized + 'static, F: Future<Output = O> + 'static>(
        &self,
        offset: usize,
        func: F,
    ) -> JoinHandle<O> {
        let (task, hdl) = Self::unsafe_wrap_function(func);

        let senders = self.task_senders.read();
        if senders.is_empty() {
            panic!("Executor not set up yet!");
        }

        let idx = offset % senders.len();
        if let Err(err) = senders[idx].send(task) {
            panic!("Failed to spawn task: {err}");
        }

        hdl
    }

    /// # Safety
    ///
    /// Make sure task is Send before polled for the first time
    /// (Can be not Send afterwards)
    pub unsafe fn unsafe_spawn<O: Send + Sized + 'static, F: Future<Output = O> + 'static>(
        &self,
        func: F,
    ) -> JoinHandle<O> {
        let (task, hdl) = Self::unsafe_wrap_function(func);

        let senders = self.task_senders.read();
        if senders.is_empty() {
            panic!("Executor not set up yet!");
        }

        let idx = rand::thread_rng().gen_range(0..senders.len());
        if let Err(err) = senders[idx].send(task) {
            panic!("Failed to spawn task: {err}");
        }

        hdl
    }

    pub fn get_num_threads(&self) -> usize {
        let senders = self.task_senders.read();
        if senders.is_empty() {
            panic!("No active kioto runtime")
        }

        senders.len()
    }
}
