use futures::{stream, StreamExt};
use http_body_util::Empty;
use hyper::Request;
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use std::str::FromStr;
use std::sync::atomic::AtomicI32;
use std::sync::Arc;
use tokio::signal::ctrl_c;
use tokio::sync::Mutex;
#[macro_use]
extern crate anyhow;
use clap::Parser;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper_rustls::ConfigBuilderExt;
use hyper_rustls::HttpsConnector;
use hyper_util::rt::TokioExecutor;
use rustls::RootCertStore;
use std::env;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::{sleep, Duration};

#[derive(Parser)]
#[command(author, version, about, long_about)]
struct Cli {
    /// The request url,like http://www.google.com
    url: String,
    /// The thread count.
    #[arg(short = 't', long, value_name = "Threads count", default_value_t = 20)]
    threads: u16,
    /// The thread count.
    #[arg(
        short = 's',
        long,
        value_name = "The running seconds",
        default_value_t = 3
    )]
    sleep_seconds: u64,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli: Cli = Cli::parse();
    let _ = do_request(cli.url, cli.threads, cli.sleep_seconds).await;
    Ok(())
}
async fn do_request(
    url: String,
    connections: u16,
    sleep_seconds: u64,
) -> Result<(), anyhow::Error> {
    let tls = rustls::ClientConfig::builder()
        .with_native_roots()?
        .with_no_client_auth();
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config(tls)
        .https_or_http()
        .enable_http1()
        .build();

    let timer = tokio::time::Instant::now();
    let client = Client::builder(hyper_util::rt::TokioExecutor::new()).build(https.clone());

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
    drop(client);
    let _ = sleep(Duration::from_secs(sleep_seconds)).await;

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
async fn submit_task(
    counter: Arc<AtomicI32>,
    client: Client<HttpsConnector<HttpConnector>, Full<Bytes>>,
    url: String,
) {
    let clone_client = client.clone();
    let clone_url: String = url.clone();
    loop {
        let cloned_client1 = clone_client.clone();
        let clone_url1 = clone_url.parse::<hyper::Uri>().unwrap();
        let result = cloned_client1
            .get(clone_url1)
            .await
            .map_err(|e| anyhow!("Terst!"));

        if let Ok(response) = result {
            if response.status().is_success() {
                tokio::spawn(statistic(counter.clone()));
            }
        }
    }
}
async fn statistic(counter: Arc<AtomicI32>) {
    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}
