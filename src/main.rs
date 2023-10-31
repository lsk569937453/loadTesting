use futures::{stream, StreamExt};
use hyper::client::HttpConnector;
use hyper::{body::HttpBody as _, Client, Uri};
use std::str::FromStr;
use std::sync::atomic::AtomicI32;
use std::sync::Arc;
use tokio::signal::windows::ctrl_c;
use tokio::sync::Mutex;

#[macro_use]
extern crate anyhow;
use std::env;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let url = String::from("http://localhost:8080");
    let connections: u16 = 10;
    do_request(url.clone(), connections).await;
    Ok(())
}
async fn do_request(url: String, connections: u16) -> Result<(), anyhow::Error> {
    let client = Client::new();
    let timer = tokio::time::Instant::now();

    let counter = Arc::new(AtomicI32::new(0));
    let mut task_list = vec![];
    for _ in 0..connections {
        let clone_url = url.clone();
        let clone_counter = counter.clone();
        let clone_client = client.clone();
        let task =
            tokio::spawn(async move { submit_task(clone_counter, clone_client, clone_url).await });
        task_list.push(task);
    }
    let mut signal = ctrl_c()?;
    signal.recv().await;

    println!("task has been canceled!");
    task_list.iter().for_each(|item| item.abort());
    let success_count = counter.load(std::sync::atomic::Ordering::Relaxed).clone();

    let time_cost: u128 = timer.elapsed().as_millis();

    let base: i32 = 10;

    let rps = base.pow(3) * success_count / (time_cost as i32);

    println!(
        "Actual time {:.2} million second, RPS {}/s,count is {}",
        time_cost, rps, success_count
    );
    Ok(())
}
async fn submit_task(counter: Arc<AtomicI32>, client: Client<HttpConnector>, url: String) {
    let clone_client = client.clone();
    let clone_url: String = url.clone();
    loop {
        let cloned_client1 = clone_client.clone();
        let clone_url1 = clone_url.parse::<hyper::Uri>().unwrap();
        let result = cloned_client1
            .get(clone_url1)
            .await
            .map_err(|e| anyhow!("Request error ,the error is {},", e));

        if let Ok(response) = result {
            if response.status().is_success() {
                counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }
}
