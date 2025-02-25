use kioto_uring_executor as kioto;

use kioto::time::{sleep, Duration};
use std::sync::mpsc;

#[test]
fn block_on() {
    kioto::block_on(async {
        sleep(Duration::from_millis(10)).await;
        println!("Hello world");
    })
}

#[test]
fn num_threads() {
    kioto::block_on_runtime(async {
        assert!(kioto::get_runtime_thread_count() > 0);
        println!("Hello world");
    })
}

#[test]
fn block_on_spawn() {
    kioto::block_on(async {
        let fut = kioto::spawn_local(async {
            sleep(Duration::from_millis(10)).await;
            println!("Hello world");
        });

        fut.join().await
    })
}

#[test]
fn runtime_block_on() {
    let runtime = kioto::Runtime::new();
    runtime.block_on_with(|| {
        Box::pin(async {
            sleep(Duration::from_millis(10)).await;
            println!("Hello world");
        })
    })
}

#[test]
fn spawn() {
    let runtime = kioto::Runtime::new();
    let (sender, receiver) = mpsc::channel();

    runtime.spawn_with(|| {
        Box::pin(async move {
            sleep(Duration::from_millis(10)).await;
            let _ = sender.send("Hello world".to_string());
        })
    });

    println!("{}", receiver.recv().unwrap());
}

#[kioto::test]
async fn join() {
    let hdl = kioto::spawn_with(|| {
        Box::pin(async move {
            sleep(Duration::from_millis(10)).await;
            "Hello world".to_string()
        })
    });

    assert_eq!("Hello world".to_string(), hdl.join().await);
}

#[kioto::test]
async fn spawn_local() {
    let hdl = kioto::spawn_local(async move {
        sleep(Duration::from_millis(10)).await;
        "Hello world".to_string()
    });

    assert_eq!("Hello world".to_string(), hdl.join().await);
}

#[test]
fn spawn_with() {
    let runtime = kioto::Runtime::new();
    let (sender, receiver) = mpsc::channel();

    runtime.spawn_with(|| {
        Box::pin(async move {
            sleep(Duration::from_millis(10)).await;
            let _ = sender.send("Hello world".to_string());
        })
    });

    assert_eq!("Hello world".to_string(), receiver.recv().unwrap());
}

#[kioto::test]
async fn spawn_ring() {
    let mut ring = kioto::new_spawn_ring();
    let hdl = ring.spawn_with(|| {
        Box::pin(async move {
            sleep(Duration::from_millis(10)).await;
            println!("Hello world");
        })
    });

    hdl.join().await;
}

#[kioto::test]
async fn executor_macro() {
    sleep(Duration::from_millis(10)).await;
    println!("Hello world");
}
