use serde::{Deserialize, Serialize};

/// Worker state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkerState {
    Idle,
    Prepared,
    Running,
    Finished,
}

/// Ping response from worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingResponse {
    pub state: WorkerState,
    pub version: String,
}

/// Worker health info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerInfo {
    pub cpu_cores: usize,
    pub version: String,
}

/// Test configuration sent from master to worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub url: String,
    pub concurrency: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_requests: Option<u64>,
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    pub start_at: i64, // Unix timestamp in milliseconds
}

/// Prepare request sent to worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareRequest {
    pub config: TestConfig,
}

/// Prepare response from worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrepareResponse {
    pub ready: bool,
}

/// Start request sent to worker (only contains start time)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartRequest {
    pub start_at: i64, // Unix timestamp in milliseconds
}

/// Response statistics from a single request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseStat {
    pub time_cost_ns: u64,
    pub status_code: u16,
    pub content_length: u64,
}

/// Error statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorStat {
    pub message: String,
    pub count: u64,
}

/// Final report from worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalReport {
    pub worker_id: String,
    pub duration_ns: u128,
    pub total_requests: u64,
    pub total_bytes: u64,
    pub responses: Vec<ResponseStat>,
    pub errors: Vec<ErrorStat>,
}

/// Status code distribution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusCodeDistribution {
    pub code: u16,
    pub count: u64,
}
