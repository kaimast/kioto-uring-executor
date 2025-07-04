use std::cell::RefCell;
use std::future::Future;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;

use cfg_if::cfg_if;

use parking_lot::RwLock;

use rand::Rng;

use crate::error::Error;
use crate::spawn::SpawnRing;

mod task;
use task::{create_task_channel, Task, TaskReceiver, TaskSender};

mod handle;
pub use handle::Handle;

mod join;
pub use join::{JoinHandle, LocalJoinHandle};

#[cfg(feature = "tokio-uring")]
use tokio_uring::spawn as backend_spawn;

#[cfg(feature = "monoio")]
use monoio::spawn as backend_spawn;

/// A wrapper for a future that can be send to another thread
/// Once it arrives at the other thread, the resulting
/// future can be not send
pub trait FutureWith<O: Send + 'static> =
    (FnOnce() -> Pin<Box<dyn Future<Output = O> + 'static>>) + Send + 'static;

thread_local! {
    pub(super) static ACTIVE_RUNTIME: RefCell<Option<Arc<RuntimeInner>>> = const { RefCell::new(None) };
}

#[derive(PartialEq, Eq)]
enum State {
    Running,
    ShuttingDown,
}

pub(super) struct RuntimeInner {
    state: RwLock<State>,
    task_senders: RwLock<Vec<TaskSender>>,
}

/// The kioto runtime
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
    pub fn handle(&self) -> Handle {
        Handle::new(self.inner.clone())
    }

    pub(crate) fn block_on_current_thread<T: 'static, F: Future<Output = T> + 'static>(
        fut: F,
    ) -> T {
        ACTIVE_RUNTIME.with_borrow(|r| {
            if r.is_some() {
                panic!("You cannot block on the current thread from within a runtime");
            }
        });

        let (sender, mut receiver) = create_task_channel();
        let inner = Arc::new(RuntimeInner::new(vec![sender]));

        Self::wrap_in_runtime(inner, async move {
            // The future might spawn other tasks;
            // wait for them here
            crate::spawn_local(async move {
                while let Some(Some(task)) = receiver.recv().await {
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
            log::trace!("kioto runtime thread started");
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

            log::trace!("kioto runtime thread finished");
            output
        }
    }

    /// Spawns a runtime with one thread per core.
    /// If there a less then four cores, it will still spawn at least four threads.
    pub fn new() -> Self {
        let min_threads = NonZeroUsize::new(4).unwrap();
        let thread_count = std::thread::available_parallelism()
            .unwrap()
            .max(min_threads);
        Self::new_with_threads(thread_count)
    }

    pub fn new_with_threads(num_os_threads: NonZeroUsize) -> Self {
        let num_os_threads = num_os_threads.get();
        log::trace!("Spawning {num_os_threads} worker threads");

        let mut senders = vec![];
        let mut receivers = vec![];
        for _ in 0..num_os_threads {
            let (sender, receiver) = create_task_channel();
            senders.push(sender);
            receivers.push(receiver);
        }

        let inner = Arc::new(RuntimeInner::new(senders));

        let threads = receivers
            .into_iter()
            .enumerate()
            .map(|(idx, receiver)| {
                std::thread::spawn(Self::wrap_in_runtime(inner.clone(), async move {
                    RuntimeInner::worker_thread(idx, receiver).await
                }))
            })
            .collect();

        log::info!("Initialized tokio runtime with {num_os_threads} worker thread(s)");
        Self { inner, threads }
    }

    /// Blocks the current thread until the runtime has finished th task
    pub fn block_on<T: Send + 'static, F: Future<Output = T> + Send + 'static>(
        &self,
        task: F,
    ) -> Result<T, Error> {
        self.inner.block_on(task)
    }

    /// Blocks the current thread until the runtime has finished th task
    pub fn block_on_with<T: Send + 'static, F: FutureWith<T> + 'static>(
        &self,
        fut: F,
    ) -> Result<T, Error> {
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
        log::debug!("Shutting down kioto runtime");

        *self.inner.state.write() = State::ShuttingDown;

        // Tell threads to shut down
        for sender in self.inner.task_senders.write().iter() {
            let result = sender.send(None);

            if let Err(err) = result {
                log::error!("Failed to shut down worker thread gracefully: {err}");
            }
        }

        // Wait for threads to shut down before we close the task sender
        // This ensure no new tasks are spawned during shutdown
        for t in self.threads.drain(..) {
            t.join().expect("Failed to join worker thread");
        }

        log::debug!("All kioto threads terminated");

        *self.inner.task_senders.write() = vec![];
    }
}

impl RuntimeInner {
    fn new(task_senders: Vec<TaskSender>) -> Self {
        Self {
            state: RwLock::new(State::Running),
            task_senders: RwLock::new(task_senders),
        }
    }

    /// The loop for each worker thread
    async fn worker_thread(idx: usize, mut receiver: TaskReceiver) {
        log::debug!("Worker thread #{idx} started");
        while let Some(Some(task)) = receiver.recv().await {
            let future = (task.generator)();
            backend_spawn(future);
        }
        log::debug!("Worker thread #{idx} finished");
    }

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

        let hdl = JoinHandle::new(receiver);

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
        log::trace!("Spawning new task on worker thread #{idx}");

        if let Err(err) = senders[idx].send(Some(task)) {
            if *self.state.read() == State::ShuttingDown {
                JoinHandle::new_aborted()
            } else {
                panic!("Failed to spawn task: {err}");
            }
        } else {
            hdl
        }
    }

    pub fn spawn_with_at<O: Send + Sized + 'static, F: FutureWith<O>>(
        &self,
        offset: usize,
        func: F,
    ) -> JoinHandle<O> {
        let (task, hdl) = Self::wrap_function(func);

        let senders = self.task_senders.read();
        if senders.is_empty() {
            // This should never happen. Panic here.
            panic!("Executor not set up yet!");
        }

        let idx = offset % senders.len();
        if let Err(err) = senders[idx].send(Some(task)) {
            if *self.state.read() == State::ShuttingDown {
                JoinHandle::new_aborted()
            } else {
                // This should also never happen.
                panic!("Failed to spawn task: {err}");
            }
        } else {
            hdl
        }
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

        LocalJoinHandle::new(inner)
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
    ) -> Result<T, Error> {
        let (sender, receiver) = std_mpsc::channel();

        self.spawn(async move {
            let res = task.await;
            sender.send(res).expect("Notification failed");
        });

        log::trace!("Waiting for block_on call to complete");
        receiver.recv().map_err(|err| Error::TaskFailed {
            message: err.to_string(),
        })
    }

    pub fn block_on_with<O: Send + Sized + 'static, F: FutureWith<O>>(
        &self,
        func: F,
    ) -> Result<O, Error> {
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

        // Drop read lock after sending task
        {
            let senders = self.task_senders.read();
            if senders.is_empty() {
                panic!("Executor not set up yet!");
            }

            let idx = rand::rng().random_range(0..senders.len());
            if let Err(err) = senders[idx].send(Some(task)) {
                panic!("Failed to spawn task: {err}");
            }
        }

        receiver.recv().map_err(|err| Error::TaskFailed {
            message: err.to_string(),
        })
    }

    pub fn get_thread_count(&self) -> usize {
        let senders = self.task_senders.read();
        if senders.is_empty() {
            panic!("No active kioto runtime")
        }

        senders.len()
    }
}
