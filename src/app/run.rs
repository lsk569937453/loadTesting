use crate::engine::config::TestResult;
use crate::engine::config::TestRunConfig;
use crate::engine::runner::run_load_test;
use crate::master::aggregator::ResultAggregator;
use crate::master::orchestrator::{MasterConfig, MasterOrchestrator};
use crate::vojo::cli::Cli;
use crate::worker::server::WorkerServer;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing_subscriber::Layer;
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub async fn main_with_error() -> Result<(), anyhow::Error> {
    let console_layer = tracing_subscriber::fmt::Layer::new()
        .with_target(true)
        .with_ansi(true)
        .with_writer(std::io::stdout)
        .with_filter(tracing_subscriber::filter::LevelFilter::INFO);
    let _ = tracing_subscriber::registry()
        .with(console_layer)
        .with(tracing_subscriber::filter::LevelFilter::TRACE)
        .try_init();

    let cli: Cli = Cli::parse();

    match cli.command {
        crate::vojo::cli::Commands::Run {
            url,
            concurrency,
            duration,
            requests,
            headers,
            body,
        } => {
            run_standalone(url, concurrency, duration, requests, headers, body).await?;
        }
        crate::vojo::cli::Commands::Worker { listen, id } => {
            run_worker(listen, id).await?;
        }
        crate::vojo::cli::Commands::Master {
            workers,
            url,
            concurrency,
            duration,
            requests,
            headers,
            body,
        } => {
            run_master(workers, url, concurrency, duration, requests, headers, body).await?;
        }
    }

    Ok(())
}

async fn run_standalone(
    url: http::Uri,
    concurrency: u16,
    duration: Option<Duration>,
    requests: u64,
    headers: Vec<(String, String)>,
    body: Option<String>,
) -> Result<(), anyhow::Error> {
    tracing::info!("Starting standalone load test...");

    // Read body from file if needed
    let body_bytes = if let Some(b) = body {
        if b.starts_with('@') {
            tokio::fs::read(b.strip_prefix('@').unwrap_or_default())
                .await
                .ok()
        } else {
            Some(b.into_bytes())
        }
    } else {
        None
    };

    let config = TestRunConfig {
        url: url.to_string(),
        concurrency,
        duration_secs: duration.map(|d| d.as_secs()),
        total_requests: if duration.is_none() {
            Some(requests)
        } else {
            None
        },
        headers,
        body: body_bytes,
        start_at: None,
    };

    // Create progress bar
    let total_requests = config.total_requests.unwrap_or(500000);
    let progress = ProgressBar::new(if config.duration_secs.is_some() {
        config.duration_secs.unwrap_or_default()
    } else {
        total_requests
    });
    progress.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}",
            )?
            .progress_chars("##-"),
    );
    progress.set_message("Running load test...");

    // Create progress channel for request mode
    let (progress_tx, progress_rx) = mpsc::channel(100);

    // Spawn test in background
    let config_clone = config.clone();
    let test_handle =
        tokio::spawn(async move { run_load_test(config_clone, Some(progress_tx)).await });

    // Update progress
    if config.duration_secs.is_some() {
        // Duration mode: update progress by second
        let duration_secs = config.duration_secs.unwrap_or_default();
        for i in 0..=duration_secs {
            progress.set_position(i);
            sleep(Duration::from_secs(1)).await;
        }
    } else {
        // Request mode: track real-time progress from channel
        let mut rx = progress_rx;
        while let Some(completed) = rx.recv().await {
            progress.set_position(completed);
            if completed >= total_requests {
                break;
            }
        }
    }

    let result = test_handle.await??;

    progress.finish_with_message("Test completed!");

    // Print results
    print_results(&result, &url.to_string());

    Ok(())
}

async fn run_worker(listen: String, id: String) -> Result<(), anyhow::Error> {
    tracing::info!("Starting worker: {} on {}", id, listen);

    let server = WorkerServer::new(id);
    server.run(listen).await?;

    Ok(())
}

async fn run_master(
    workers: Vec<String>,
    url: http::Uri,
    concurrency: u16,
    duration: Option<Duration>,
    requests: u64,
    headers: Vec<(String, String)>,
    body: Option<String>,
) -> Result<(), anyhow::Error> {
    tracing::info!("Starting master mode with {} workers", workers.len());

    let config = MasterConfig {
        url: url.to_string(),
        total_concurrency: concurrency,
        duration_secs: duration.map(|d| d.as_secs()),
        total_requests: if duration.is_none() {
            Some(requests)
        } else {
            None
        },
        headers,
        body,
    };

    let orchestrator = MasterOrchestrator::new(workers);
    let reports = orchestrator.run(config).await?;

    if reports.is_empty() {
        tracing::error!("No reports received from workers");
        return Ok(());
    }

    // Aggregate results
    let aggregator = ResultAggregator::new(reports);
    if let Ok(summary) = aggregator.aggregate() {
        println!("\n{}", summary);
    }

    Ok(())
}

fn print_results(result: &TestResult, url: &str) {
    use hdrhistogram::Histogram;
    use std::collections::HashMap;

    if result.responses.is_empty() {
        println!("No responses were recorded.");
        return;
    }

    let histogram_res = Histogram::<u64>::new_with_bounds(1, 60_000_000_000, 3);
    let Ok(mut histogram) = histogram_res else {
        println!("histogram_res is error");
        return;
    };
    for resp in &result.responses {
        histogram.record(resp.time_cost_ns).unwrap_or_default();
    }

    let duration_secs = result.duration_ns as f64 / 1_000_000_000.0;
    let total_requests = result.responses.len() as u64;

    // Calculate status codes
    let mut status_codes: HashMap<u16, u64> = HashMap::new();
    for resp in &result.responses {
        *status_codes.entry(resp.status_code).or_insert(0) += 1;
    }

    // Helper function to format latency
    fn format_latency_ns(ns: u64) -> String {
        if ns >= 1_000_000_000 {
            format!("{:.2} s", ns as f64 / 1_000_000_000.0)
        } else if ns >= 1_000_000 {
            format!("{:.2} ms", ns as f64 / 1_000_000.0)
        } else if ns >= 1_000 {
            format!("{:.2} µs", ns as f64 / 1_000.0)
        } else {
            format!("{} ns", ns)
        }
    }

    // Helper function to get HTTP status description
    fn status_description(code: u16) -> &'static str {
        match code {
            200 => "OK",
            201 => "Created",
            204 => "No Content",
            301 => "Moved Permanently",
            302 => "Found",
            304 => "Not Modified",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            405 => "Method Not Allowed",
            500 => "Internal Server Error",
            502 => "Bad Gateway",
            503 => "Service Unavailable",
            504 => "Gateway Timeout",
            _ => "Unknown",
        }
    }

    println!("\n=== Load Test Results ===");
    println!("URL: {}", url);
    println!("Duration: {:.2}s", duration_secs);
    println!("Total Requests: {}", total_requests);
    println!("Requests/sec: {:.2}", total_requests as f64 / duration_secs);
    println!(
        "Transfer: {:.2} MB",
        result.total_bytes as f64 / (1024.0 * 1024.0)
    );
    println!("\nLatency:");
    println!("  Average: {}", format_latency_ns(histogram.mean() as u64));
    println!("  Min:     {}", format_latency_ns(histogram.min()));
    println!("  Max:     {}", format_latency_ns(histogram.max()));
    println!(
        "  P50:     {}",
        format_latency_ns(histogram.value_at_quantile(0.5))
    );
    println!(
        "  P90:     {}",
        format_latency_ns(histogram.value_at_quantile(0.9))
    );
    println!(
        "  P95:     {}",
        format_latency_ns(histogram.value_at_quantile(0.95))
    );
    println!(
        "  P99:     {}",
        format_latency_ns(histogram.value_at_quantile(0.99))
    );

    println!("\nStatus Codes:");
    let mut codes: Vec<_> = status_codes.iter().collect();
    codes.sort_by_key(|&(k, _)| k);
    for (code, count) in codes {
        let percent = (*count as f64 / total_requests as f64) * 100.0;
        println!(
            "  {} {} - {} ({:.1}%)",
            code,
            status_description(*code),
            count,
            percent
        );
    }

    if !result.errors.is_empty() {
        println!("\nErrors:");
        for err in &result.errors {
            println!("  [{}] {}", err.count, err.message);
        }
    }
}
