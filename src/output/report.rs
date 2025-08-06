use crate::vojo::cli::Cli;
use hdrhistogram::Histogram;
use itertools::Itertools;
use std::collections::HashMap;
use std::fmt::Display;
use std::fmt::Formatter;
use std::time::Duration;
pub struct StatisticList {
    pub response_list: Vec<Result<ResponseStatistic, anyhow::Error>>,
    pub cli: Cli,
}
pub struct ResponseStatistic {
    pub time_cost_ns: u64,
    pub status_code: u16,
    pub content_length: u64,
}
// 【优化】用于存放所有计算后结果的结构体
#[derive(Debug)]
pub struct BenchmarkSummary {
    // 会话信息
    url: String, // 新增：测试的目标 URL
    concurrency: u64,
    actual_duration: Duration, // 改为实际测试时长

    // 吞吐量
    requests_per_sec: f64,
    data_transfer_rate_mbps: f64,

    // 延迟 (使用 Duration 类型)
    average_latency: Duration,
    latency_std_dev: Duration,
    slowest: Duration,
    fastest: Duration,

    // 延迟百分位数
    p50: Duration,
    p90: Duration,
    p95: Duration,
    p99: Duration,
    p99_9: Duration,

    // 数据传输
    total_data: u64,
    avg_size_per_request: f64,

    // 结果分布
    total_requests: usize,
    successful_requests: usize,
    status_code_dist: HashMap<u16, usize>,
    error_dist: HashMap<String, usize>,
}

impl StatisticList {
    /// 分析压测结果。
    /// 【重要】传入实际的测试总耗时，以获得最精确的 RPS 计算。
    pub fn analyze(&self, actual_duration: Duration) -> Option<BenchmarkSummary> {
        if self.response_list.is_empty() {
            return None;
        }

        let mut status_code_dist = HashMap::new();
        let mut error_dist = HashMap::new();
        let mut hist = Histogram::<u64>::new(3).unwrap();

        let mut total_data = 0;
        let mut successful_times_ns = Vec::new();

        for result in &self.response_list {
            match result {
                Ok(item) => {
                    successful_times_ns.push(item.time_cost_ns);
                    hist.record(item.time_cost_ns).unwrap();
                    total_data += item.content_length;
                    *status_code_dist.entry(item.status_code).or_insert(0) += 1;
                }
                Err(e) => {
                    *error_dist.entry(e.to_string()).or_insert(0) += 1;
                }
            }
        }

        let successful_requests = successful_times_ns.len();
        if successful_requests == 0 {
            // 所有请求都失败的场景
            return Some(self.build_error_summary(actual_duration, error_dist));
        }

        // --- 开始计算 ---
        let total_duration_sec = actual_duration.as_secs_f64();
        let requests_per_sec = self.response_list.len() as f64 / total_duration_sec;
        let data_transfer_rate_mbps = (total_data as f64 / (1024.0 * 1024.0)) / total_duration_sec;

        let total_time_cost_ns: u64 = successful_times_ns.iter().sum();
        let average_ns = total_time_cost_ns as f64 / successful_requests as f64;

        let variance = successful_times_ns
            .iter()
            .map(|&time| {
                let diff = time as f64 - average_ns;
                diff * diff
            })
            .sum::<f64>()
            / successful_requests as f64;
        let std_dev_ns = variance.sqrt();

        let avg_size_per_request = total_data as f64 / successful_requests as f64;

        Some(BenchmarkSummary {
            url: self.cli.url.to_string().clone(),
            concurrency: self.cli.concurrency as u64, // 类型转换 u16 -> u64
            actual_duration,
            requests_per_sec,
            data_transfer_rate_mbps,
            average_latency: Duration::from_nanos(average_ns as u64),
            latency_std_dev: Duration::from_nanos(std_dev_ns as u64),
            slowest: Duration::from_nanos(*successful_times_ns.iter().max().unwrap_or(&0)),
            fastest: Duration::from_nanos(*successful_times_ns.iter().min().unwrap_or(&0)),
            p50: Duration::from_nanos(hist.value_at_quantile(0.50)),
            p90: Duration::from_nanos(hist.value_at_quantile(0.90)),
            p95: Duration::from_nanos(hist.value_at_quantile(0.95)),
            p99: Duration::from_nanos(hist.value_at_quantile(0.99)),
            p99_9: Duration::from_nanos(hist.value_at_quantile(0.999)),
            total_data,
            avg_size_per_request,
            total_requests: self.response_list.len(),
            successful_requests,
            status_code_dist,
            error_dist,
        })
    }

    // 辅助函数，用于构建只有错误的摘要
    fn build_error_summary(
        &self,
        actual_duration: Duration,
        error_dist: HashMap<String, usize>,
    ) -> BenchmarkSummary {
        BenchmarkSummary {
            url: self.cli.url.to_string().clone(),
            concurrency: self.cli.concurrency as u64,
            actual_duration,
            requests_per_sec: self.response_list.len() as f64 / actual_duration.as_secs_f64(),
            data_transfer_rate_mbps: 0.0,
            average_latency: Duration::default(),
            latency_std_dev: Duration::default(),
            slowest: Duration::default(),
            fastest: Duration::default(),
            p50: Duration::default(),
            p90: Duration::default(),
            p95: Duration::default(),
            p99: Duration::default(),
            p99_9: Duration::default(),
            total_data: 0,
            avg_size_per_request: 0.0,
            total_requests: self.response_list.len(),
            successful_requests: 0,
            status_code_dist: HashMap::new(),
            error_dist,
        }
    }
}

// 为 BenchmarkSummary 实现 Display trait，专门用于格式化输出
impl Display for BenchmarkSummary {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        // --- 辅助函数，用于格式化 ---
        fn format_duration(d: Duration) -> String {
            if d.as_secs_f64() >= 1.0 {
                format!("{:.2} s", d.as_secs_f64())
            } else if d.as_millis() >= 1 {
                format!("{} ms", d.as_millis())
            } else if d.as_micros() >= 1 {
                format!("{} µs", d.as_micros())
            } else {
                format!("{} ns", d.as_nanos())
            }
        }

        fn format_bytes(b: u64) -> String {
            byte_unit::Byte::from_u64(b)
                .get_appropriate_unit(byte_unit::UnitType::Binary)
                .to_string()
        }

        writeln!(f, "Http Stress Test Summary")?;
        writeln!(f, "====================================")?;

        // --- Session Section ---
        writeln!(f, "\n[Session]")?;
        writeln!(f, "  URL:              {}", self.url)?;
        writeln!(f, "  Concurrency:      {} threads", self.concurrency)?;
        writeln!(
            f,
            "  Test Duration:    {:.2} s",
            self.actual_duration.as_secs_f64()
        )?;

        // --- Throughput Section ---
        writeln!(f, "\n[Throughput]")?;
        writeln!(f, "  Requests/sec:     {:.2}", self.requests_per_sec)?;
        writeln!(
            f,
            "  Transfer Rate:    {:.2} MB/s",
            self.data_transfer_rate_mbps
        )?;

        // --- Latency & Data Sections (only if there were successful requests) ---
        if self.successful_requests > 0 {
            writeln!(f, "\n[Latency]")?;
            writeln!(
                f,
                "  Average:          {}",
                format_duration(self.average_latency)
            )?;
            writeln!(
                f,
                "  StdDev:           {}",
                format_duration(self.latency_std_dev)
            )?;
            writeln!(f, "  Slowest:          {}", format_duration(self.slowest))?;
            writeln!(f, "  Fastest:          {}", format_duration(self.fastest))?;

            writeln!(f, "\n[Latency Percentiles]")?;
            writeln!(f, "  P50 (Median):     {}", format_duration(self.p50))?;
            writeln!(f, "  P90:              {}", format_duration(self.p90))?;
            writeln!(f, "  P95:              {}", format_duration(self.p95))?;
            writeln!(f, "  P99:              {}", format_duration(self.p99))?;
            writeln!(f, "  P99.9:            {}", format_duration(self.p99_9))?;

            writeln!(f, "\n[Data Transfer]")?;
            writeln!(f, "  Total Data:       {}", format_bytes(self.total_data))?;
            writeln!(
                f,
                "  Size/request:     {:.2} bytes",
                self.avg_size_per_request
            )?;
        }

        // --- Results Section ---
        writeln!(f, "\n[Status Code Distribution]")?;
        if self.status_code_dist.is_empty() {
            writeln!(f, "  (No successful requests)")?;
        } else {
            for (code, count) in self.status_code_dist.iter().sorted_by_key(|&(&c, _)| c) {
                let percent = (*count as f64 / self.total_requests as f64) * 100.0;
                writeln!(f, "  [{code}] {count} responses ({percent:.2}%)")?;
            }
        }

        writeln!(f, "\n[Error Distribution]")?;
        if self.error_dist.is_empty() {
            writeln!(f, "  (No errors)")?;
        } else {
            for (error, count) in &self.error_dist {
                writeln!(f, "  - \"{error}\": {count} occurrences")?;
            }
        }

        Ok(())
    }
}
