use kioto_uring_executor as kioto;

use kioto::time::{sleep, Duration};
use std::sync::mpsc;

#[test]
fn block_on() {
    let _ = env_logger::builder().is_test(true).try_init();

    kioto::block_on(async {
        sleep(Duration::from_millis(10)).await;
        println!("Hello world");
    })
}

#[test]
fn num_threads() {
    let _ = env_logger::builder().is_test(true).try_init();

    kioto::block_on(async {
        assert!(kioto::get_runtime_thread_count() > 0);
        println!("Hello world");
    })
}

#[kioto::test]
async fn num_threads_in_runtime() {
    let _ = env_logger::builder().is_test(true).try_init();

    // make sure ACTIVE_RUNTIME is set correctly
    assert!(kioto::get_runtime_thread_count() > 0);

    kioto::spawn(async {
        // ensure ACTIVE_RUNTIME is still set
        assert!(kioto::get_runtime_thread_count() > 0);
        println!("Hello world");
    })
    .join()
    .await
    .unwrap();
}

#[test]
fn block_on_spawn() {
    let _ = env_logger::builder().is_test(true).try_init();

    kioto::block_on(async {
        let fut = kioto::spawn_local(async {
            sleep(Duration::from_millis(10)).await;
            println!("Hello world");
        });

        fut.join().await.unwrap();
    })
}

#[test]
fn runtime_block_on() {
    let _ = env_logger::builder().is_test(true).try_init();

    let runtime = kioto::Runtime::new();
    runtime
        .block_on_with(|| {
            Box::pin(async {
                sleep(Duration::from_millis(10)).await;
                println!("Hello world");
            })
        })
        .unwrap();
}

#[test]
fn spawn() {
    let _ = env_logger::builder().is_test(true).try_init();

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
    let _ = env_logger::builder().is_test(true).try_init();

    let hdl = kioto::spawn_with(|| {
        Box::pin(async move {
            sleep(Duration::from_millis(10)).await;
            "Hello world".to_string()
        })
    });

    assert_eq!("Hello world".to_string(), hdl.join().await.unwrap());
}

#[kioto::test]
async fn spawn_local() {
    let _ = env_logger::builder().is_test(true).try_init();

    let hdl = kioto::spawn_local(async move {
        sleep(Duration::from_millis(10)).await;
        "Hello world".to_string()
    });

    assert_eq!("Hello world".to_string(), hdl.join().await.unwrap());
}

#[test]
fn spawn_with() {
    let _ = env_logger::builder().is_test(true).try_init();

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
    let _ = env_logger::builder().is_test(true).try_init();

    let mut ring = kioto::new_spawn_ring();
    let hdl = ring.spawn_with(|| {
        Box::pin(async move {
            sleep(Duration::from_millis(10)).await;
            println!("Hello world");
        })
    });

    hdl.join().await.unwrap();
}

#[kioto::test]
async fn executor_macro() {
    let _ = env_logger::builder().is_test(true).try_init();

    sleep(Duration::from_millis(10)).await;
    println!("Hello world");
}
