use std::cell::RefCell;
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;

use cfg_if::cfg_if;

use parking_lot::RwLock;

use rand::Rng;

use tokio::sync::mpsc;

use crate::spawn::SpawnRing;

#[cfg(feature = "tokio-uring")]
use tokio::task::JoinHandle as InnerJoinHandle;
#[cfg(feature = "tokio-uring")]
use tokio_uring::spawn as backend_spawn;

#[cfg(feature = "monoio")]
use monoio::{spawn as backend_spawn, task::JoinHandle as InnerJoinHandle};

/// A wrapper for a future that can be send to another thread
/// Once it arrives at the other thread, the resulting
/// future can be not send
pub trait FutureWith<O: Send + 'static> =
    (FnOnce() -> Pin<Box<dyn Future<Output = O> + 'static>>) + Send + 'static;

trait Generator = (FnOnce() -> Pin<Box<dyn Future<Output = ()> + 'static>>) + Send;

pub struct Task {
    generator: Box<dyn Generator>,
}

type TaskSender = mpsc::UnboundedSender<Task>;

thread_local! {
    pub(super) static ACTIVE_RUNTIME: RefCell<Option<Arc<RuntimeInner>>> = const { RefCell::new(None) };
}

pub struct JoinHandle<O: Sized + Send + 'static> {
    receiver: tokio::sync::oneshot::Receiver<O>,
}

/// A local join handle is different from a cross-thread
/// JoinHandle in that it is not Send and can be aborted
pub struct LocalJoinHandle<O: Sized + 'static> {
    inner: InnerJoinHandle<O>,
}

impl<O: Sized> LocalJoinHandle<O> {
    #[cfg(feature = "tokio-uring")]
    pub fn abort(self) {
        self.inner.abort()
    }

    pub async fn join(self) -> O {
        #[cfg(feature = "tokio-uring")]
        {
            self.inner.await.unwrap()
        }
        #[cfg(feature = "monoio")]
        {
            self.inner.await
        }
    }
}

impl<O: Sized + Send> JoinHandle<O> {
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
    threads: Vec<std::thread::JoinHandle<()>>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Runtime {
    pub(crate) fn block_on_current_thread<T: 'static, F: Future<Output = T> + 'static>(
        fut: F,
    ) -> T {
        let (sender, mut receiver) = mpsc::unbounded_channel::<Task>();
        let inner = Arc::new(RuntimeInner {
            task_senders: RwLock::new(vec![sender]),
        });

        Self::wrap_in_runtime(inner, async move {
            // The future might spawn other tasks;
            // wait for them here
            crate::spawn_local(async move {
                while let Some(task) = receiver.recv().await {
                    let future = (task.generator)();
                    backend_spawn(future);
                }
            });

            fut.await
        })()
    }

    fn wrap_in_runtime<O>(
        inner: Arc<RuntimeInner>,
        fut: impl Future<Output = O>,
    ) -> impl FnOnce() -> O {
        move || {
            ACTIVE_RUNTIME.with_borrow_mut(|r| {
                *r = Some(inner);
            });

            cfg_if! {
                if #[cfg(feature="tokio-uring")] {
                    let output = tokio_uring::start(fut);
                } else {
                    let output = crate::generate_runtime().block_on(fut);
                }
            }

            ACTIVE_RUNTIME.with_borrow_mut(|r| {
                *r = None;
            });

            output
        }
    }

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

        let threads = (0..num_os_threads)
            .map(|idx| {
                let (sender, mut receiver) = mpsc::unbounded_channel::<Task>();
                inner.task_senders.write().push(sender);

                std::thread::spawn(Self::wrap_in_runtime(inner.clone(), async move {
                    log::debug!("Worker threads #{idx} started");
                    while let Some(task) = receiver.recv().await {
                        let future = (task.generator)();
                        backend_spawn(future);
                    }
                    log::debug!("Worker thread #{idx} done");
                }))
            })
            .collect();

        Self { inner, threads }
    }

    /// Blocks the current thread until the runtime has finished th task
    pub fn block_on<T: Send + 'static, F: Future<Output = T> + Send + 'static>(
        &self,
        task: F,
    ) -> T {
        self.inner.block_on(task)
    }

    /// Blocks the current thread until the runtime has finished th task
    pub fn block_on_with<T: Send + 'static, F: FutureWith<T> + 'static>(&self, fut: F) -> T {
        self.inner.block_on_with(fut)
    }

    /// Spawns the task on a random thread
    pub fn spawn<O: Send + Sized, F: Future<Output = O> + Send + 'static>(
        &self,
        func: F,
    ) -> JoinHandle<O> {
        self.inner.spawn(func)
    }

    /// Spawns the task on the current thread
    pub fn spawn_local<O: Sized, F: Future<Output = O> + 'static>(
        &self,
        func: F,
    ) -> LocalJoinHandle<O> {
        self.inner.spawn_local(func)
    }

    /// Spawns the task on a random thread
    ///
    /// This allows to send some data even if the resulting
    /// future is not Sync
    pub fn spawn_with<O: Send + Sized + 'static, F: FutureWith<O>>(
        &self,
        func: F,
    ) -> JoinHandle<O> {
        self.inner.spawn_with(func)
    }

    /// How many worker threads are there?
    pub fn get_thread_count(&self) -> usize {
        self.inner.get_thread_count()
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

        for t in self.threads.drain(..) {
            t.join().expect("Failed to join worker thread");
        }
    }
}

impl RuntimeInner {
    fn wrap_function<O: Send + 'static, F: FutureWith<O>>(func: F) -> (Task, JoinHandle<O>) {
        let (sender, receiver) = tokio::sync::oneshot::channel();

        let task = Task {
            generator: Box::new(move || {
                let func = func();
                Box::pin(async move {
                    let result = func.await;
                    let _ = sender.send(result);
                })
            }),
        };

        let hdl = JoinHandle { receiver };

        (task, hdl)
    }

    pub fn spawn_with<O: Send + Sized + 'static, F: FutureWith<O>>(
        &self,
        func: F,
    ) -> JoinHandle<O> {
        let (task, hdl) = Self::wrap_function(func);

        let senders = self.task_senders.read();
        if senders.is_empty() {
            panic!("Executor not set up yet!");
        }

        let idx = rand::rng().random_range(0..senders.len());
        if let Err(err) = senders[idx].send(task) {
            panic!("Failed to spawn task: {err}");
        }

        hdl
    }

    pub fn spawn_with_at<O: Send + Sized + 'static, F: FutureWith<O>>(
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

    pub fn spawn_local<O: Sized + 'static, F: Future<Output = O> + 'static>(
        &self,
        func: F,
    ) -> LocalJoinHandle<O> {
        cfg_if! {
            if #[cfg(feature="tokio-uring")] {
                let inner = tokio_uring::spawn(func);
            } else {
                let inner = monoio::spawn(func);
            }
        }

        LocalJoinHandle { inner }
    }

    pub fn spawn<O: Sized + Send + 'static, F: Future<Output = O> + Send + 'static>(
        &self,
        func: F,
    ) -> JoinHandle<O> {
        self.spawn_with(move || Box::pin(func))
    }

    /// Spawns the task on a specific thread
    pub fn spawn_at<O: Send + 'static, F: Future<Output = O> + Send + 'static>(
        &self,
        offset: usize,
        func: F,
    ) -> JoinHandle<O> {
        self.spawn_with_at(offset, || Box::pin(func))
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

    pub fn block_on_with<O: Send + Sized + 'static, F: FutureWith<O>>(&self, func: F) -> O {
        let (sender, receiver) = std_mpsc::channel();

        let task = Task {
            generator: Box::new(move || {
                let func = func();
                Box::pin(async move {
                    let result = func.await;
                    let _ = sender.send(result);
                })
            }),
        };

        let senders = self.task_senders.read();
        if senders.is_empty() {
            panic!("Executor not set up yet!");
        }

        let idx = rand::rng().random_range(0..senders.len());
        if let Err(err) = senders[idx].send(task) {
            panic!("Failed to spawn task: {err}");
        }

        receiver.recv().expect("Failed to wait for task")
    }

    pub fn get_thread_count(&self) -> usize {
        let senders = self.task_senders.read();
        if senders.is_empty() {
            panic!("No active kioto runtime")
        }

        senders.len()
    }
}
