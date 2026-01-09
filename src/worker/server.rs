use crate::engine::config::TestRunConfig;
use crate::engine::runner::run_load_test;
use crate::protocol::models::*;
use crate::worker::state::WorkerStateMgr;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
pub struct WorkerServer {
    state: Arc<WorkerStateMgr>,
}

impl WorkerServer {
    pub fn new(id: String) -> Self {
        Self {
            state: Arc::new(WorkerStateMgr::new(id)),
        }
    }

    pub async fn run(self, addr: String) -> Result<(), anyhow::Error> {
        let app = Router::new()
            .route("/ping", get(ping_handler))
            .route("/info", get(info_handler))
            .route("/prepare", post(prepare_handler))
            .route("/start", post(start_handler))
            .route("/status", get(status_handler))
            .route("/report", get(report_handler))
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            )
            .with_state(self.state.clone());

        let socket_addr: SocketAddr = addr.parse()?;
        let listener = tokio::net::TcpListener::bind(socket_addr).await?;

        tracing::info!("Worker {} listening on {}", self.state.id(), addr);

        axum::serve(listener, app).await?;

        Ok(())
    }
}

async fn ping_handler(State(state): State<Arc<WorkerStateMgr>>) -> impl IntoResponse {
    let current = state.get_state().await;
    Json(PingResponse {
        state: current,
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

async fn info_handler(State(_state): State<Arc<WorkerStateMgr>>) -> impl IntoResponse {
    Json(WorkerInfo {
        cpu_cores: num_cpus::get(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

async fn prepare_handler(
    State(state): State<Arc<WorkerStateMgr>>,
    Json(req): Json<PrepareRequest>,
) -> impl IntoResponse {
    let ready = match state.set_config(req.config).await {
        Ok(_) => true,
        Err(e) => {
            tracing::error!("Prepare failed: {}", e);
            false
        }
    };
    Json(PrepareResponse { ready })
}

async fn start_handler(
    State(state): State<Arc<WorkerStateMgr>>,
    Json(_req): Json<StartRequest>,
) -> impl IntoResponse {
    // Start the test in background
    let state_clone = state.clone();
    tokio::spawn(async move {
        if let Err(e) = state_clone.start().await {
            tracing::error!("Start failed: {}", e);
            return;
        }

        let config = match state_clone.get_config().await {
            Some(c) => c,
            None => return,
        };

        let run_config = TestRunConfig {
            url: config.url.clone(),
            concurrency: config.concurrency,
            duration_secs: config.duration_secs,
            total_requests: config.total_requests,
            headers: config.headers.clone(),
            body: config.body.as_ref().and_then(|b| {
                if b.starts_with('@') {
                    // Read from file - not implemented for now
                    None
                } else {
                    Some(b.as_bytes().to_vec())
                }
            }),
            start_at: Some(config.start_at),
        };

        // Wait until start time
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let wait_ms = config.start_at.saturating_sub(now_ms);
        if wait_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(wait_ms as u64)).await;
        }

        // Run the test
        let test_result = run_load_test(run_config).await;
        let Ok(result) = test_result else {
            return;
        };

        // Convert to FinalReport
        let report = FinalReport {
            worker_id: state_clone.id().to_string(),
            duration_ns: result.duration_ns,
            total_requests: result.responses.len() as u64,
            total_bytes: result.total_bytes,
            responses: result.responses,
            errors: result.errors,
        };

        if let Err(e) = state_clone.finish(report).await {
            error!("Finish failed: {}", e);
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({"status": "starting"})),
    )
}

async fn status_handler(State(state): State<Arc<WorkerStateMgr>>) -> impl IntoResponse {
    Json(state.get_state().await)
}

async fn report_handler(
    State(state): State<Arc<WorkerStateMgr>>,
) -> (StatusCode, Json<serde_json::Value>) {
    if let Some(report) = state.get_report().await {
        let json_value = serde_json::to_value(&report)
            .unwrap_or(serde_json::json!({"error": "Serialization failed"}));
        (StatusCode::OK, Json(json_value))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "No report available"})),
        )
    }
}
