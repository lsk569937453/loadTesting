use hyper::body::Incoming;
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use output::report::ResponseStatistic;
use output::report::StatisticList;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::broadcast::Receiver;
use tokio::sync::Mutex;
mod output;
mod vojo;
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

use crate::vojo::cli::Cli;
use hyper::header::HeaderName;
use hyper::header::CONTENT_TYPE;
use hyper::HeaderMap;
use hyper::Request;
use std::str::FromStr;
use tokio::sync::broadcast;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio::time::Instant;
use tokio::time::{sleep, Duration};
use tracing::Level;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber).unwrap();
    let cli: Cli = Cli::parse();
    if let Err(e) = do_request(cli).await {
        println!("{}", e);
    }
    Ok(())
}
async fn do_request(cli: Cli) -> Result<(), anyhow::Error> {
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
    let mut method = String::from("GET");
    let mut content_type_option = None;
    if cli.body_option.is_some() {
        method = String::from("POST");
        content_type_option = Some(String::from("application/x-www-form-urlencoded"));
    }
    let mut req_builder = Request::builder()
        .method(method.as_str())
        .uri(cli.url.clone());
    let mut header_map = HeaderMap::new();
    if let Some(content_type) = content_type_option {
        header_map.insert(CONTENT_TYPE, HeaderValue::from_str(&content_type)?);
    }
    for x in cli.headers.clone() {
        let split: Vec<String> = x.splitn(2, ':').map(|s| s.to_string()).collect();
        let key = &split[0];
        let value = &split[1];
        header_map.insert(
            HeaderName::from_str(key.as_str())?,
            HeaderValue::from_str(value)?,
        );
    }
    for (key, val) in header_map {
        req_builder = req_builder.header(key.ok_or(anyhow!(""))?, val);
    }
    let req = req_builder.body(Full::new(Bytes::new()))?;

    let mut task_list = JoinSet::new();
    let shared_list: Arc<Mutex<StatisticList>> = Arc::new(Mutex::new(StatisticList {
        cli: cli.clone(),
        response_list: vec![],
    }));
    let (sender, _) = broadcast::channel(16);

    let now = Instant::now();
    for _ in 0..cli.threads {
        let rx2: Receiver<()> = sender.subscribe();

        let cloned_list = shared_list.clone();
        let cloned_req = req.clone();
        let clone_client: Client<HttpsConnector<HttpConnector>, Full<Bytes>> = client.clone();
        task_list.spawn(async move {
            submit_task(cloned_list.clone(), clone_client, cloned_req, rx2).await
        });
    }
    let _ = sleep(Duration::from_secs(cli.sleep_seconds)).await;
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
    request: Request<Full<Bytes>>,

    mut receiver: Receiver<()>,
) -> Result<(), anyhow::Error> {
    let clone_client = client.clone();

    loop {
        let now = Instant::now();
        let cloned_client1 = clone_client.clone();
        let result = timeout(
            Duration::from_secs(1),
            cloned_client1.request(request.clone()),
        )
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
                time_cost,
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
