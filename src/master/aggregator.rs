use crate::output::report::BenchmarkSummary;
use crate::protocol::models::FinalReport;
use hdrhistogram::Histogram;
use std::collections::HashMap;
use std::time::Duration;

pub struct ResultAggregator {
    reports: Vec<FinalReport>,
}

impl ResultAggregator {
    pub fn new(reports: Vec<FinalReport>) -> Self {
        Self { reports }
    }

    pub fn aggregate(&self) -> Result<BenchmarkSummary, anyhow::Error> {
        if self.reports.is_empty() {
            return Err(anyhow::anyhow!("No reports to aggregate"));
        }

        let mut all_times_ns = Vec::new();
        let mut total_data = 0;
        let mut status_code_dist: HashMap<u16, usize> = HashMap::new();
        let mut error_dist: HashMap<String, usize> = HashMap::new();

        // Merge all worker results
        for report in &self.reports {
            total_data += report.total_bytes;

            for resp in &report.responses {
                all_times_ns.push(resp.time_cost_ns);
                *status_code_dist.entry(resp.status_code).or_insert(0) += 1;
            }

            for err in &report.errors {
                *error_dist.entry(err.message.clone()).or_insert(0) += err.count as usize;
            }
        }

        if all_times_ns.is_empty() {
            return Err(anyhow::anyhow!("No responses recorded"));
        }

        // Calculate duration (use the max duration from all workers)
        let max_duration_ns = self.reports.iter()
            .map(|r| r.duration_ns)
            .max()
            .unwrap_or(0);
        let actual_duration = Duration::from_nanos(max_duration_ns as u64);

        let total_requests = all_times_ns.len();

        // Build histogram
        let mut hist = match Histogram::<u64>::new(3) {
            Ok(h) => h,
            Err(_) => return Err(anyhow::anyhow!("Failed to create histogram")),
        };
        for &time_ns in &all_times_ns {
            hist.record(time_ns)?;
        }

        // Calculate stats
        let total_time_cost_ns: u64 = all_times_ns.iter().sum();
        let average_ns = total_time_cost_ns as f64 / total_requests as f64;

        let variance = all_times_ns
            .iter()
            .map(|&time| {
                let diff = time as f64 - average_ns;
                diff * diff
            })
            .sum::<f64>()
            / total_requests as f64;
        let std_dev_ns = variance.sqrt();

        let total_duration_sec = actual_duration.as_secs_f64();
        let requests_per_sec = total_requests as f64 / total_duration_sec;
        let data_transfer_rate_mbps = (total_data as f64 / (1024.0 * 1024.0)) / total_duration_sec;

        let avg_size_per_request = total_data as f64 / total_requests as f64;

        let url = self.reports[0].responses.get(0)
            .map(|_| "Load test".to_string())
            .unwrap_or_else(|| "Load test".to_string());

        let total_concurrency = self.reports.len() as u64;

        // Create BenchmarkSummary
        let summary = BenchmarkSummary {
            url,
            concurrency: total_concurrency,
            actual_duration,
            requests_per_sec,
            data_transfer_rate_mbps,
            average_latency: Duration::from_nanos(average_ns as u64),
            latency_std_dev: Duration::from_nanos(std_dev_ns as u64),
            slowest: Duration::from_nanos(*all_times_ns.iter().max().unwrap_or(&0)),
            fastest: Duration::from_nanos(*all_times_ns.iter().min().unwrap_or(&0)),
            p50: Duration::from_nanos(hist.value_at_quantile(0.50)),
            p90: Duration::from_nanos(hist.value_at_quantile(0.90)),
            p95: Duration::from_nanos(hist.value_at_quantile(0.95)),
            p99: Duration::from_nanos(hist.value_at_quantile(0.99)),
            p99_9: Duration::from_nanos(hist.value_at_quantile(0.999)),
            total_data,
            avg_size_per_request,
            total_requests,
            successful_requests: all_times_ns.len(),
            status_code_dist,
            error_dist,
        };

        Ok(summary)
    }
}
