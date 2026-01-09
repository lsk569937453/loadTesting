use http_body_util::Full;
use hyper::body::Bytes;
use hyper::body::Incoming;
use hyper::header::{CONTENT_LENGTH, CONTENT_TYPE, HeaderName, HeaderValue};
use hyper::{HeaderMap, Request, Response};
use hyper_rustls::HttpsConnector;
use hyper_util::client::legacy::Client;
use hyper_util::client::legacy::connect::HttpConnector;
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::{Duration, Instant, sleep, timeout};
use tokio_util::sync::CancellationToken;

use crate::engine::config::TestResult;
use crate::engine::config::TestRunConfig;
use crate::protocol::models::{ErrorStat, ResponseStat};

type HttpClient = Client<HttpsConnector<HttpConnector>, Full<Bytes>>;

/// Result sent from worker tasks
enum WorkerResult {
    Response { stat: ResponseStat, bytes: u64 },
    Error(String),
}

/// Progress tracker for real-time updates
#[derive(Clone)]
pub struct ProgressTracker {
    completed: Arc<AtomicU64>,
}

impl ProgressTracker {
    pub fn new() -> Self {
        Self {
            completed: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn get(&self) -> u64 {
        self.completed.load(Ordering::Relaxed)
    }

    fn increment(&self) {
        self.completed.fetch_add(1, Ordering::Relaxed);
    }
}

/// Run the load test and return results
pub async fn run_load_test(
    config: TestRunConfig,
    progress_tx: Option<mpsc::Sender<u64>>,
) -> Result<TestResult, anyhow::Error> {
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

    // Create progress tracker
    let progress_tracker = ProgressTracker::new();
    let progress_tracker_for_ui = progress_tracker.clone();
    let total_requests = config.total_requests;

    // Spawn progress reporter if channel provided
    if let Some(tx) = progress_tx {
        tokio::spawn(async move {
            let mut last_reported = 0u64;
            loop {
                let current = progress_tracker_for_ui.get();
                if current > last_reported {
                    if tx.send(current).await.is_err() {
                        break;
                    }
                    last_reported = current;
                }
                if let Some(total) = total_requests {
                    if current >= total {
                        break;
                    }
                }
                sleep(Duration::from_millis(50)).await;
            }
        });
    }

    // Create channel for collecting results (buffered for performance)
    let (result_tx, mut result_rx) = mpsc::channel(config.concurrency as usize * 16);

    // Spawn result collector task
    let collector_handle = tokio::spawn(async move {
        let mut responses = Vec::new();
        let mut errors = Vec::new();
        let mut total_bytes = 0u64;

        while let Some(result) = result_rx.recv().await {
            match result {
                WorkerResult::Response { stat, bytes } => {
                    total_bytes += bytes;
                    responses.push(stat);
                }
                WorkerResult::Error(msg) => add_error(&mut errors, &msg),
            }
        }

        (responses, errors, total_bytes)
    });

    let mut task_list = JoinSet::new();

    if let Some(duration) = config.duration_secs {
        // Duration-based test using CancellationToken
        let cancel_token = CancellationToken::new();
        let token_for_tasks = cancel_token.clone();

        for _ in 0..config.concurrency {
            let tx = result_tx.clone();
            let cloned_req = req.clone();
            let clone_client = client.clone();
            let token = token_for_tasks.clone();

            task_list.spawn(async move {
                submit_task_duration(tx, clone_client, cloned_req, token).await;
            });
        }

        // Wait for test duration
        sleep(Duration::from_secs(duration)).await;

        // Cancel all tasks
        cancel_token.cancel();
    } else {
        // Request-count based test
        let total_requests = config
            .total_requests
            .ok_or(anyhow!("total_requests run error"))?;

        for _ in 0..config.concurrency {
            let tx = result_tx.clone();
            let cloned_req = req.clone();
            let clone_client = client.clone();
            let tracker = progress_tracker.clone();
            let requests_per_worker = (total_requests / config.concurrency as u64)
                + if total_requests % config.concurrency as u64 > 0 {
                    1
                } else {
                    0
                };

            task_list.spawn(async move {
                submit_task_requests(tx, clone_client, cloned_req, requests_per_worker, tracker).await;
            });
        }
    }

    // Drop our sender so the collector task knows we're done
    drop(result_tx);

    // Wait for all worker tasks to complete
    while let Some(_) = task_list.join_next().await {}

    let duration_ns = start_time.elapsed().as_nanos();

    // Wait for collector to finish
    let (responses, errors, total_bytes) = collector_handle.await?;

    Ok(TestResult {
        duration_ns,
        responses,
        errors,
        total_bytes,
    })
}

async fn submit_task_duration(
    tx: mpsc::Sender<WorkerResult>,
    client: HttpClient,
    request: Request<Full<Bytes>>,
    cancel_token: CancellationToken,
) {
    loop {
        // Check if cancelled
        if cancel_token.is_cancelled() {
            return;
        }

        let now = Instant::now();
        let result = timeout(Duration::from_millis(500), client.request(request.clone())).await;
        let elapsed = now.elapsed().as_nanos() as u64;

        match result {
            Ok(Ok(res)) => {
                let content_len = get_content_length(&res);
                let _ = tx
                    .send(WorkerResult::Response {
                        stat: ResponseStat {
                            time_cost_ns: elapsed,
                            status_code: res.status().as_u16(),
                            content_length: content_len,
                        },
                        bytes: content_len,
                    })
                    .await;
            }
            Ok(Err(e)) => {
                let _ = tx.send(WorkerResult::Error(format!("{}", e))).await;
            }
            Err(_) => {
                let _ = tx
                    .send(WorkerResult::Error("Request timeout".to_string()))
                    .await;
            }
        }

        // Check again after request
        if cancel_token.is_cancelled() {
            return;
        }
    }
}

async fn submit_task_requests(
    tx: mpsc::Sender<WorkerResult>,
    client: HttpClient,
    request: Request<Full<Bytes>>,
    total_requests: u64,
    progress: ProgressTracker,
) {
    for _ in 0..total_requests {
        let now = Instant::now();
        let result = timeout(Duration::from_millis(500), client.request(request.clone())).await;
        let elapsed = now.elapsed().as_nanos() as u64;

        match result {
            Ok(Ok(res)) => {
                let content_len = get_content_length(&res);
                let _ = tx
                    .send(WorkerResult::Response {
                        stat: ResponseStat {
                            time_cost_ns: elapsed,
                            status_code: res.status().as_u16(),
                            content_length: content_len,
                        },
                        bytes: content_len,
                    })
                    .await;
            }
            Ok(Err(e)) => {
                let _ = tx.send(WorkerResult::Error(format!("{}", e))).await;
            }
            Err(_) => {
                let _ = tx
                    .send(WorkerResult::Error("Request timeout".to_string()))
                    .await;
            }
        }

        // Update progress counter
        progress.increment();
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
