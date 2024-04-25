use futures::{stream, StreamExt};
use http_body_util::BodyExt;
use http_body_util::Empty;
use hyper::body::Incoming;
use hyper::Request;
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use output::report::ResponseStatistic;
use output::report::StatisticList;
use std::str::FromStr;
use std::sync::atomic::AtomicI32;
use std::sync::Arc;
use tokio::signal::ctrl_c;
use tokio::sync::Mutex;
mod output;
#[macro_use]
extern crate anyhow;
use clap::Parser;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::header::HeaderValue;
use hyper::header::CONTENT_LENGTH;
use hyper::Response;
use hyper_rustls::ConfigBuilderExt;
use hyper_rustls::HttpsConnector;
use hyper_util::rt::TokioExecutor;
use rustls::RootCertStore;
use std::env;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::Instant;
use tokio::time::{sleep, Duration};
#[derive(Parser)]
#[command(author, version, about, long_about)]
struct Cli {
    /// The request url,like http://www.google.com
    url: String,
    /// Number of workers to run concurrently. Total number of requests cannot
    ///  be smaller than the concurrency level. Default is 50..
    #[arg(
        short = 'c',
        long,
        value_name = "Number of workers",
        default_value_t = 50
    )]
    threads: u16,
    /// Duration of application to send requests. When duration is reached,application stops and exits.
    #[arg(
        short = 'z',
        long,
        value_name = "Duration of application to send requests",
        default_value_t = 5
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

    let client = Client::builder(hyper_util::rt::TokioExecutor::new()).build(https.clone());

    let mut task_list = vec![];
    let shared_list: Arc<Mutex<StatisticList>> = Arc::new(Mutex::new(StatisticList {
        response_list: vec![],
    }));
    let now = Instant::now();
    for _ in 0..connections {
        let cloned_list = shared_list.clone();
        let clone_url = url.clone();
        let clone_client = client.clone();
        let task =
            tokio::spawn(
                async move { submit_task(cloned_list.clone(), clone_client, clone_url).await },
            );
        task_list.push(task);
    }
    drop(client);
    let _ = sleep(Duration::from_secs(sleep_seconds)).await;
    let total_cost = now.elapsed().as_millis();
    task_list.iter().for_each(|item| item.abort());
    let list = shared_list.lock().await;
    list.print(total_cost);
    Ok(())
}
async fn submit_task(
    shared_list: Arc<Mutex<StatisticList>>,
    client: Client<HttpsConnector<HttpConnector>, Full<Bytes>>,
    url: String,
) {
    let clone_client = client.clone();
    let clone_url: String = url.clone();
    loop {
        let now = Instant::now();

        let cloned_client1 = clone_client.clone();
        let clone_url1 = clone_url.parse::<hyper::Uri>().unwrap();
        let result = cloned_client1
            .get(clone_url1)
            .await
            .map_err(|e| anyhow!("Terst!"));
        let elapsed = now.elapsed().as_millis();
        if let Ok(r) = result {
            tokio::spawn(statistic(shared_list.clone(), elapsed, r));
        }
    }
}
async fn statistic(
    shared_list: Arc<Mutex<StatisticList>>,
    time_cost: u128,
    res: Response<Incoming>,
) {
    let default_content_length = HeaderValue::from_static("0");
    let content_len_header = res
        .headers()
        .get(CONTENT_LENGTH)
        .unwrap_or(&default_content_length);
    let content_len = content_len_header
        .to_str()
        .unwrap_or("0")
        .parse::<u64>()
        .unwrap_or(0);
    let mut list = shared_list.lock().await;
    let response_statistic = ResponseStatistic {
        time_cost: time_cost,
        staus_code: res.status().as_u16(),
        content_length: content_len,
    };
    list.response_list.push(response_statistic);
}
