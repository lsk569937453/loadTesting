use http_body_util::Full;
use hyper::body::Bytes;
use hyper::body::Incoming;
use hyper::header::{CONTENT_LENGTH, CONTENT_TYPE, HeaderName, HeaderValue};
use hyper::{HeaderMap, Request, Response};
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use tokio::sync::Mutex;
use tokio::sync::broadcast::Receiver;
use tokio::task::JoinSet;
use tokio::time::{Duration, Instant, sleep, timeout};

use crate::engine::config::TestResult;
use crate::engine::config::TestRunConfig;
use crate::protocol::models::{ErrorStat, ResponseStat};
type HttpClient = Client<HttpsConnector<HttpConnector>, Full<Bytes>>;

/// Run the load test and return results
pub async fn run_load_test(config: TestRunConfig) -> Result<TestResult, anyhow::Error> {
    let client = super::client::create_http_client()?;

    // Build request
    let mut method = String::from("GET");
    let mut content_type_option = None;
    if config.body.is_some() {
        method = String::from("POST");
        content_type_option = Some(String::from("application/x-www-form-urlencoded"));
    }

    let mut req_builder = Request::builder()
        .method(method.as_str())
        .uri(config.url.clone());

    let mut header_map = HeaderMap::new();
    if let Some(content_type) = content_type_option {
        header_map.insert(CONTENT_TYPE, HeaderValue::from_str(&content_type)?);
    }
    for (key, value) in &config.headers {
        header_map.insert(
            HeaderName::from_str(key.as_str())?,
            HeaderValue::from_str(value)?,
        );
    }
    for (key, val) in header_map {
        if let Some(key) = key {
            req_builder = req_builder.header(key, val);
        }
    }

    let body_bytes = config.body.clone().unwrap_or_default();
    let req = req_builder.body(Full::new(Bytes::from(body_bytes)))?;

    // Wait for start time if specified
    if let Some(start_at_ms) = config.start_at {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis() as i64;
        let wait_ms = start_at_ms.saturating_sub(now_ms);
        if wait_ms > 0 {
            sleep(Duration::from_millis(wait_ms as u64)).await;
        }
    }

    let start_time = Instant::now();
    let responses = Arc::new(Mutex::new(Vec::new()));
    let errors = Arc::new(Mutex::new(Vec::new()));
    let total_bytes = Arc::new(AtomicI64::new(0));

    let mut task_list = JoinSet::new();

    if let Some(duration) = config.duration_secs {
        // Duration-based test
        let (sender, _) = tokio::sync::broadcast::channel(16);
        for _ in 0..config.concurrency {
            let rx2 = sender.subscribe();
            let cloned_responses = responses.clone();
            let cloned_errors = errors.clone();
            let cloned_bytes = total_bytes.clone();
            let cloned_req = req.clone();
            let clone_client = client.clone();
            task_list.spawn(async move {
                submit_task_duration(
                    cloned_responses,
                    cloned_errors,
                    cloned_bytes,
                    clone_client,
                    cloned_req,
                    rx2,
                )
                .await
            });
        }
        sleep(Duration::from_secs(duration)).await;
        sender.send(())?;
    } else {
        // Request-count based test
        let total_requests = config
            .total_requests
            .ok_or(anyhow!("total_requests run error"))?;
        let requests_counter = Arc::new(AtomicI64::new(total_requests as i64));
        for _ in 0..config.concurrency {
            let counter_clone = requests_counter.clone();
            let cloned_responses = responses.clone();
            let cloned_errors = errors.clone();
            let cloned_bytes = total_bytes.clone();
            let cloned_req = req.clone();
            let clone_client = client.clone();
            task_list.spawn(async move {
                submit_task_requests(
                    cloned_responses,
                    cloned_errors,
                    cloned_bytes,
                    clone_client,
                    cloned_req,
                    counter_clone,
                )
                .await
            });
        }
    }

    while let Some(_) = task_list.join_next().await {}

    let duration_ns = start_time.elapsed().as_nanos();

    Ok(TestResult {
        duration_ns,
        responses: Arc::try_unwrap(responses)
            .map_err(|e| anyhow!(""))?
            .into_inner(),
        errors: Arc::try_unwrap(errors)
            .map_err(|e| anyhow!(""))?
            .into_inner(),
        total_bytes: total_bytes.load(Ordering::Relaxed) as u64,
    })
}

async fn submit_task_duration(
    responses: Arc<Mutex<Vec<ResponseStat>>>,
    errors: Arc<Mutex<Vec<ErrorStat>>>,
    total_bytes: Arc<AtomicI64>,
    client: HttpClient,
    request: Request<Full<Bytes>>,
    mut receiver: Receiver<()>,
) {
    loop {
        let now = Instant::now();
        let result = timeout(Duration::from_millis(500), client.request(request.clone())).await;
        let elapsed = now.elapsed().as_nanos() as u64;
        match result {
            Ok(Ok(res)) => {
                let content_len = get_content_length(&res);
                total_bytes.fetch_add(content_len as i64, Ordering::Relaxed);
                let mut resp = responses.lock().await;
                resp.push(ResponseStat {
                    time_cost_ns: elapsed,
                    status_code: res.status().as_u16(),
                    content_length: content_len,
                });
            }
            Ok(Err(e)) => {
                let mut err = errors.lock().await;
                add_error(&mut err, &format!("{}", e));
            }
            Err(_) => {
                let mut err = errors.lock().await;
                add_error(&mut err, "Request timeout");
            }
        }
        tokio::select! {
            biased;
            _ = receiver.recv() => {
                return;
            }
            _=async{}=>{}
        }
    }
}

async fn submit_task_requests(
    responses: Arc<Mutex<Vec<ResponseStat>>>,
    errors: Arc<Mutex<Vec<ErrorStat>>>,
    total_bytes: Arc<AtomicI64>,
    client: HttpClient,
    request: Request<Full<Bytes>>,
    requests_counter: Arc<AtomicI64>,
) {
    while requests_counter.fetch_sub(1, Ordering::Relaxed) > 0 {
        let now = Instant::now();
        let result = timeout(Duration::from_millis(500), client.request(request.clone())).await;
        let elapsed = now.elapsed().as_nanos() as u64;
        match result {
            Ok(Ok(res)) => {
                let content_len = get_content_length(&res);
                total_bytes.fetch_add(content_len as i64, Ordering::Relaxed);
                let mut resp = responses.lock().await;
                resp.push(ResponseStat {
                    time_cost_ns: elapsed,
                    status_code: res.status().as_u16(),
                    content_length: content_len,
                });
            }
            Ok(Err(e)) => {
                let mut err = errors.lock().await;
                add_error(&mut err, &format!("{}", e));
            }
            Err(_) => {
                let mut err = errors.lock().await;
                add_error(&mut err, "Request timeout");
            }
        }
    }
}

fn get_content_length(res: &Response<Incoming>) -> u64 {
    let default_content_length = HeaderValue::from_static("0");
    let content_len_header = res
        .headers()
        .get(CONTENT_LENGTH)
        .unwrap_or(&default_content_length);
    content_len_header
        .to_str()
        .unwrap_or("0")
        .parse::<u64>()
        .unwrap_or(0)
}

fn add_error(errors: &mut Vec<ErrorStat>, message: &str) {
    if let Some(existing) = errors.iter_mut().find(|e| e.message == message) {
        existing.count += 1;
    } else {
        errors.push(ErrorStat {
            message: message.to_string(),
            count: 1,
        });
    }
}
