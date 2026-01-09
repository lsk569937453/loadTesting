use crate::protocol::models::{WorkerState, TestConfig, FinalReport};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Worker state machine
pub struct WorkerStateMgr {
    id: String,
    state: Arc<Mutex<InnerState>>,
}

struct InnerState {
    current_state: WorkerState,
    config: Option<TestConfig>,
    report: Option<FinalReport>,
}

impl WorkerStateMgr {
    pub fn new(id: String) -> Self {
        Self {
            id,
            state: Arc::new(Mutex::new(InnerState {
                current_state: WorkerState::Idle,
                config: None,
                report: None,
            })),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub async fn get_state(&self) -> WorkerState {
        self.state.lock().await.current_state
    }

    pub async fn set_config(&self, config: TestConfig) -> Result<(), anyhow::Error> {
        let mut inner = self.state.lock().await;
        if inner.current_state != WorkerState::Idle {
            return Err(anyhow::anyhow!("Worker is not idle"));
        }
        inner.config = Some(config);
        inner.current_state = WorkerState::Prepared;
        Ok(())
    }

    pub async fn get_config(&self) -> Option<TestConfig> {
        self.state.lock().await.config.clone()
    }

    pub async fn start(&self) -> Result<(), anyhow::Error> {
        let mut inner = self.state.lock().await;
        if inner.current_state != WorkerState::Prepared {
            return Err(anyhow::anyhow!("Worker is not prepared"));
        }
        inner.current_state = WorkerState::Running;
        Ok(())
    }

    pub async fn finish(&self, report: FinalReport) -> Result<(), anyhow::Error> {
        let mut inner = self.state.lock().await;
        if inner.current_state != WorkerState::Running {
            return Err(anyhow::anyhow!("Worker is not running"));
        }
        inner.report = Some(report);
        inner.current_state = WorkerState::Finished;
        Ok(())
    }

    pub async fn get_report(&self) -> Option<FinalReport> {
        self.state.lock().await.report.clone()
    }

    pub async fn reset(&self) {
        let mut inner = self.state.lock().await;
        inner.current_state = WorkerState::Idle;
        inner.config = None;
        inner.report = None;
    }
}
