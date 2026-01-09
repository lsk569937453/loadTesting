/// Configuration for running a load test
#[derive(Clone, Debug)]
pub struct TestRunConfig {
    pub url: String,
    pub concurrency: u16,
    pub duration_secs: Option<u64>,
    pub total_requests: Option<u64>,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub start_at: Option<i64>, // Unix timestamp in milliseconds
}

/// Result of a load test
#[derive(Debug)]
pub struct TestResult {
    pub duration_ns: u128,
    pub responses: Vec<ResponseStat>,
    pub errors: Vec<ErrorStat>,
    pub total_bytes: u64,
}

use crate::protocol::models::{ResponseStat, ErrorStat};
