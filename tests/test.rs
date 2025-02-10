use kioto_uring_executor as executor;

use executor::time::{sleep, Duration};
use std::sync::mpsc;

#[test]
fn block_on() {
    executor::block_on(async {
        sleep(Duration::from_millis(10)).await;
        println!("Hello world");
    })
}

#[test]
fn block_on_spawn() {
    executor::block_on(async {
        let fut = executor::spawn_local(async {
            sleep(Duration::from_millis(10)).await;
            println!("Hello world");
        });

        fut.join().await
    })
}

#[test]
fn runtime_block_on() {
    let runtime = executor::Runtime::new();
    runtime.block_on_with(|| {
        Box::pin(async {
            sleep(Duration::from_millis(10)).await;
            println!("Hello world");
        })
    })
}

#[test]
fn spawn() {
    let runtime = executor::Runtime::new();
    let (sender, receiver) = mpsc::channel();

    runtime.spawn_with(|| {
        Box::pin(async move {
            sleep(Duration::from_millis(10)).await;
            let _ = sender.send("Hello world".to_string());
        })
    });

    println!("{}", receiver.recv().unwrap());
}

#[kioto_uring_executor::test]
async fn join() {
    let hdl = kioto_uring_executor::spawn_with(|| {
        Box::pin(async move {
            sleep(Duration::from_millis(10)).await;
            "Hello world".to_string()
        })
    });

    assert_eq!("Hello world".to_string(), hdl.join().await);
}

#[kioto_uring_executor::test]
async fn spawn_local() {
    let hdl = kioto_uring_executor::spawn_local(async move {
        sleep(Duration::from_millis(10)).await;
        "Hello world".to_string()
    });

    assert_eq!("Hello world".to_string(), hdl.join().await);
}

#[test]
fn spawn_with() {
    let runtime = executor::Runtime::new();
    let (sender, receiver) = mpsc::channel();

    runtime.spawn_with(|| {
        Box::pin(async move {
            sleep(Duration::from_millis(10)).await;
            let _ = sender.send("Hello world".to_string());
        })
    });

    assert_eq!("Hello world".to_string(), receiver.recv().unwrap());
}

#[kioto_uring_executor::test]
async fn spawn_ring() {
    let mut ring = kioto_uring_executor::new_spawn_ring();
    let hdl = ring.spawn_with(|| {
        Box::pin(async move {
            sleep(Duration::from_millis(10)).await;
            println!("Hello world");
        })
    });

    hdl.join().await;
}

#[kioto_uring_executor::test]
async fn executor_macro() {
    sleep(Duration::from_millis(10)).await;
    println!("Hello world");
}
