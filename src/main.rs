use hyper::body::Incoming;
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use output::report::ResponseStatistic;
use output::report::StatisticList;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
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
use hyper_rustls::HttpsConnector;

use rustls::crypto::ring::default_provider;
use rustls::crypto::ring::DEFAULT_CIPHER_SUITES;
use rustls::crypto::CryptoProvider;
use rustls::ClientConfig;
use rustls::RootCertStore;

use tokio::sync::broadcast;
use tokio::time::timeout;

use tokio::task::JoinSet;
use tokio::time::Instant;
use tokio::time::{sleep, Duration};
use tracing;
use tracing::Level;

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
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber).unwrap();
    let cli: Cli = Cli::parse();
    if let Err(e) = do_request(cli.url, cli.threads, cli.sleep_seconds).await {
        println!("{}", e);
    }
    Ok(())
}
async fn do_request(
    url: String,
    connections: u16,
    sleep_seconds: u64,
) -> Result<(), anyhow::Error> {
    let mut root_store = RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let versions = rustls::DEFAULT_VERSIONS.to_vec();
    let tls_config = ClientConfig::builder_with_provider(
        CryptoProvider {
            cipher_suites: DEFAULT_CIPHER_SUITES.to_vec(),
            ..default_provider()
        }
        .into(),
    )
    .with_protocol_versions(&versions)?
    .with_root_certificates(root_store)
    .with_no_client_auth();
    let https = hyper_rustls::HttpsConnectorBuilder::new()
        .with_tls_config(tls_config)
        .https_or_http()
        .enable_http1()
        .build();

    let client = Client::builder(hyper_util::rt::TokioExecutor::new()).build(https.clone());

    let mut task_list = JoinSet::new();
    let shared_list: Arc<Mutex<StatisticList>> = Arc::new(Mutex::new(StatisticList {
        response_list: vec![],
    }));
    let (sender, _) = broadcast::channel(16);

    let now = Instant::now();
    for _ in 0..connections {
        let rx2: Receiver<()> = sender.subscribe();

        let cloned_list = shared_list.clone();
        let clone_url = url.clone();
        let clone_client = client.clone();
        task_list.spawn(async move {
            submit_task(cloned_list.clone(), clone_client, clone_url, rx2).await
        });
    }
    let _ = sleep(Duration::from_secs(sleep_seconds)).await;
    sender.send(())?;

    let total_cost = now.elapsed().as_millis();
    while let Some(r) = task_list.join_next().await {
        if let Ok(Ok(_)) = r {
        } else {
            println!("cause errppr");
        }
    }
    drop(client);

    let list = shared_list.lock().await;
    list.print(total_cost);
    Ok(())
}
async fn submit_task(
    shared_list: Arc<Mutex<StatisticList>>,
    client: Client<HttpsConnector<HttpConnector>, Full<Bytes>>,
    url: String,
    mut receiver: Receiver<()>,
) -> Result<(), anyhow::Error> {
    let clone_client = client.clone();
    let clone_url: String = url.clone();
    let clone_url1 = clone_url.parse::<hyper::Uri>().unwrap();

    loop {
        let now = Instant::now();
        let cloned_client1 = clone_client.clone();
        let current = clone_url1.clone();
        let result = timeout(Duration::from_secs(1), cloned_client1.get(current))
            .await?
            .map_err(|e| {
                if let Some(err) = e.source() {
                    anyhow!("{}", err)
                } else {
                    anyhow!(e)
                }
            });
        let elapsed = now.elapsed().as_millis();
        match result {
            Ok(res) => {
                tokio::spawn(statistic(shared_list.clone(), elapsed, Ok(res)));
            }
            Err(e) => {
                tokio::spawn(statistic(shared_list.clone(), elapsed, Err(anyhow!(e))));
            }
        }
        tokio::select! {
            biased;
            _ = receiver.recv() => {
                return Ok(());
            }
            _=async{}=>{}
        }
    }

    Ok(())
}
async fn statistic(
    shared_list: Arc<Mutex<StatisticList>>,
    time_cost: u128,
    result: Result<Response<Incoming>, anyhow::Error>,
) {
    match result {
        Ok(res) => {
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
            list.response_list.push(Ok(response_statistic));
        }
        Err(e) => {
            let mut list = shared_list.lock().await;

            list.response_list.push(Err(anyhow!("{}", e)));
        }
    };
}
