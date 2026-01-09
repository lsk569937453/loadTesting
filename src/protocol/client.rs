use crate::protocol::models::*;
use anyhow::Result;

/// HTTP client for communicating with workers
pub struct WorkerClient {
    base_url: String,
    client: reqwest::Client,
}

impl WorkerClient {
    pub fn new(addr: &str) -> Self {
        let base_url = if addr.starts_with("http://") || addr.starts_with("https://") {
            addr.to_string()
        } else {
            format!("http://{}", addr)
        };

        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }

    /// Ping worker to check if it's alive
    pub async fn ping(&self) -> Result<PingResponse> {
        let resp = self.client.get(&format!("{}/ping", self.base_url))
            .send()
            .await?;
        Ok(resp.json().await?)
    }

    /// Get worker info
    pub async fn info(&self) -> Result<WorkerInfo> {
        let resp = self.client.get(&format!("{}/info", self.base_url))
            .send()
            .await?;
        Ok(resp.json().await?)
    }

    /// Prepare worker for test
    pub async fn prepare(&self, req: &PrepareRequest) -> Result<PrepareResponse> {
        let resp = self.client.post(&format!("{}/prepare", self.base_url))
            .json(req)
            .send()
            .await?;
        Ok(resp.json().await?)
    }

    /// Start the test on worker
    pub async fn start(&self, req: &StartRequest) -> Result<()> {
        self.client.post(&format!("{}/start", self.base_url))
            .json(req)
            .send()
            .await?;
        Ok(())
    }

    /// Get current status from worker
    pub async fn status(&self) -> Result<WorkerState> {
        let resp = self.client.get(&format!("{}/status", self.base_url))
            .send()
            .await?;
        Ok(resp.json().await?)
    }

    /// Get final report from worker
    pub async fn report(&self) -> Result<FinalReport> {
        let resp = self.client.get(&format!("{}/report", self.base_url))
            .send()
            .await?;
        Ok(resp.json().await?)
    }
}
