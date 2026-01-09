use crate::protocol::client::WorkerClient;
use crate::protocol::models::*;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct MasterConfig {
    pub url: String,
    pub total_concurrency: u16,
    pub duration_secs: Option<u64>,
    pub total_requests: Option<u64>,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

pub struct MasterOrchestrator {
    workers: Vec<String>,
}

impl MasterOrchestrator {
    pub fn new(workers: Vec<String>) -> Self {
        Self { workers }
    }

    pub async fn run(&self, config: MasterConfig) -> Result<Vec<FinalReport>, anyhow::Error> {
        let num_workers = self.workers.len() as u16;
        if num_workers == 0 {
            return Err(anyhow::anyhow!("No workers provided"));
        }

        // Step 1: Health check all workers
        tracing::info!("Checking worker health...");
        let mut clients = Vec::new();
        for worker_addr in &self.workers {
            let client = WorkerClient::new(worker_addr);
            let info = client.info().await?;
            tracing::info!("Worker {} OK: {} cores, version {}", worker_addr, info.cpu_cores, info.version);
            clients.push(client);
        }

        // Step 2: Calculate concurrency per worker
        let concurrency_per_worker = config.total_concurrency / num_workers;
        let remainder = config.total_concurrency % num_workers;

        // Step 3: Prepare all workers
        tracing::info!("Preparing workers...");
        let start_delay_secs = 5; // Start 5 seconds from now
        let start_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_millis() as i64 + (start_delay_secs * 1000) as i64;

        for (i, client) in clients.iter().enumerate() {
            let worker_concurrency = if (i as u16) < remainder {
                concurrency_per_worker + 1
            } else {
                concurrency_per_worker
            };

            let duration_secs = config.duration_secs;
            let total_requests: Option<u64> = if let Some(r) = config.total_requests {
                let base = r / num_workers as u64;
                let rem = r % num_workers as u64;
                Some(if (i as u64) < rem { base + 1 } else { base })
            } else {
                None
            };

            let test_config = TestConfig {
                url: config.url.clone(),
                concurrency: worker_concurrency,
                duration_secs,
                total_requests,
                headers: config.headers.clone(),
                body: config.body.clone(),
                start_at,
            };

            let prep_req = PrepareRequest { config: test_config };
            client.prepare(&prep_req).await?;
            tracing::info!("Worker {} prepared with {} concurrency", self.workers[i], worker_concurrency);
        }

        // Step 4: Start all workers simultaneously
        tracing::info!("Starting test in {} seconds...", start_delay_secs);
        let start_req = StartRequest { start_at };
        for client in &clients {
            client.start(&start_req).await?;
        }

        // Step 5: Wait for test to complete and collect reports
        tracing::info!("Waiting for test to complete...");
        let test_duration = config.duration_secs.map_or_else(
            || Duration::from_secs(300), // Default 5 min for request-based
            |d| Duration::from_secs(d),
        );

        // Add buffer for in-flight requests
        let wait_duration = test_duration + Duration::from_secs(10);

        let mut reports = Vec::new();
        let start = std::time::Instant::now();

        while start.elapsed() < wait_duration {
            let mut all_finished = true;
            for (i, client) in clients.iter().enumerate() {
                match client.status().await {
                    Ok(WorkerState::Finished) => {
                        // Get report if not already collected
                        if !reports.iter().any(|r: &FinalReport| r.worker_id == format!("worker-{}", i + 1)) {
                            match client.report().await {
                                Ok(report) => {
                                    tracing::info!("Worker {} finished: {} requests", self.workers[i], report.total_requests);
                                    reports.push(report);
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to get report from worker {}: {}", self.workers[i], e);
                                }
                            }
                        }
                    }
                    Ok(WorkerState::Running) => {
                        all_finished = false;
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("Failed to get status from worker {}: {}", self.workers[i], e);
                    }
                }
            }

            if all_finished && reports.len() == clients.len() {
                break;
            }

            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        // Try to get any remaining reports
        for client in &clients {
            if let Ok(report) = client.report().await {
                if !reports.iter().any(|r| r.worker_id == report.worker_id) {
                    reports.push(report);
                }
            }
        }

        Ok(reports)
    }
}
