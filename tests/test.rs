use kioto_uring_executor as executor;

use std::sync::mpsc;
use tokio::time::{sleep, Duration};

#[test]
fn block_on() {
    executor::block_on(async {
        sleep(Duration::from_millis(10)).await;
        println!("Hello world");
    })
}

#[test]
fn runtime_block_on() {
    let runtime = executor::Runtime::new();
    runtime.block_on(async {
        sleep(Duration::from_millis(10)).await;
        println!("Hello world");
    })
}

#[test]
fn spawn() {
    let runtime = executor::Runtime::new();
    let (sender, receiver) = mpsc::channel();

    runtime.spawn(async move {
        sleep(Duration::from_millis(10)).await;
        let _ = sender.send("Hello world".to_string());
    });

    println!("{}", receiver.recv().unwrap());
}
