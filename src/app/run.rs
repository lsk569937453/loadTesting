use crate::engine::config::TestResult;
use crate::engine::config::TestRunConfig;
use crate::engine::runner::run_load_test;
use crate::master::aggregator::ResultAggregator;
use crate::master::orchestrator::MasterConfig;
use crate::master::orchestrator::MasterOrchestrator;
use crate::vojo::cli::Cli;
use crate::worker::server::WorkerServer;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;
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

    let (_duration_mode, _total_or_duration) = if let Some(d) = config.duration_secs {
        ("duration", format!("{}s", d))
    } else {
        (
            "requests",
            config.total_requests.unwrap_or_default().to_string(),
        )
    };

    // Create progress bar
    let progress = ProgressBar::new(if config.duration_secs.is_some() {
        config.duration_secs.unwrap_or_default()
    } else {
        config.total_requests.unwrap_or_default()
    });
    progress.set_style(ProgressStyle::default_bar().template(
        "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
    )?);
    progress.set_message("Running load test...");

    // Spawn test in background
    let config_clone = config.clone();
    let test_handle = tokio::spawn(async move { run_load_test(config_clone).await });

    // Update progress
    if config.duration_secs.is_some() {
        let duration_secs = config.duration_secs.unwrap_or_default();
        for i in 0..=duration_secs {
            progress.set_position(i);
            sleep(Duration::from_secs(1)).await;
        }
    } else {
        loop {
            progress
                .set_position(config.total_requests.unwrap_or_default() - progress.position() + 1);
            sleep(Duration::from_millis(100)).await;
            if test_handle.is_finished() {
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
    println!("  Average: {} ns", histogram.mean() as u64);
    println!("  Min: {} ns", histogram.min());
    println!("  Max: {} ns", histogram.max());
    println!("  P50: {} ns", histogram.value_at_quantile(0.5));
    println!("  P90: {} ns", histogram.value_at_quantile(0.9));
    println!("  P95: {} ns", histogram.value_at_quantile(0.95));
    println!("  P99: {} ns", histogram.value_at_quantile(0.99));

    println!("\nStatus Codes:");
    let mut codes: Vec<_> = status_codes.iter().collect();
    codes.sort_by_key(|&(k, _)| k);
    for (code, count) in codes {
        println!("  {}: {}", code, count);
    }

    if !result.errors.is_empty() {
        println!("\nErrors:");
        for err in &result.errors {
            println!("  {}: {}", err.message, err.count);
        }
    }
}
