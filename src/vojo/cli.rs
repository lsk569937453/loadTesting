use clap::{Parser, Subcommand};
use http::Uri;
use std::time::Duration;

/// A simple yet powerful HTTP stress testing tool.
#[derive(Parser, Clone, Debug)]
#[command(name = "kt")]
#[command(author, version, about, long_about)]
#[command(after_help = "
EXAMPLES:
  # Standalone mode
  kt run -c 100 -d 30s https://api.example.com

  # Worker mode
  kt worker --listen 0.0.0.0:50051 --id worker-1

  # Master mode with multiple workers
  kt master --workers 10.0.0.1:50051,10.0.0.2:50051 -c 1000 -d 2m https://api.example.com

  # With custom headers
  kt run -H \"Authorization: Bearer token\" https://api.example.com

  # POST request with body
  kt run -b '{\"key\":\"value\"}' https://api.example.com/api

MODES:
  run    Standalone mode (single machine load testing)
  worker Worker node (listens for master commands)
  master Master node (orchestrates multiple workers)

For more information, visit: https://github.com/your-repo/cargo-kt
")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum Commands {
    /// Run in standalone mode (original behavior)
    Run {
        /// The URL to benchmark, e.g., http://localhost:8080/
        #[arg(value_parser = parse_url)]
        url: Uri,

        /// Number of concurrent workers (threads) to run.
        #[arg(short = 'c', long, default_value_t = 50)]
        concurrency: u16,

        /// Duration of the test. Stops when the duration is reached.
        /// Mutually exclusive with --requests. e.g., 30s, 10m.
        #[arg(short = 'd', long, value_parser = parse_strict_duration, conflicts_with = "requests")]
        duration: Option<Duration>,

        /// Total number of requests to send.
        /// Mutually exclusive with --duration.
        #[arg(
            short = 'r',
            long,
            help = "Total number of requests to send. Mutually exclusive with --duration.",
            default_value = "500000",
            conflicts_with = "duration"
        )]
        requests: u64,

        /// Add a custom HTTP header to the request.
        /// This option can be used multiple times. Format: "Key:Value".
        #[arg(short = 'H', long = "header", value_parser = parse_key_val, name = "KEY_VALUE")]
        headers: Vec<(String, String)>,

        /// The HTTP request body data.
        /// If the value starts with '@', the rest is treated as a file path,
        /// and its content will be read as the body.
        #[arg(short = 'b', long = "body")]
        body: Option<String>,
    },
    /// Start as a worker node
    Worker {
        /// Listen address for the worker HTTP server
        #[arg(short = 'l', long, default_value = "0.0.0.0:50051")]
        listen: String,

        /// Worker ID for identification
        #[arg(short = 'i', long, default_value = "worker-1")]
        id: String,
    },
    /// Start as master node
    Master {
        /// Comma-separated list of worker addresses
        #[arg(short = 'w', long, value_delimiter = ',')]
        workers: Vec<String>,

        /// The URL to benchmark, e.g., http://localhost:8080/
        #[arg(value_parser = parse_url)]
        url: Uri,

        /// Number of concurrent workers (threads) to run.
        #[arg(short = 'c', long, default_value_t = 50)]
        concurrency: u16,

        /// Duration of the test. Stops when the duration is reached.
        /// Mutually exclusive with --requests. e.g., 30s, 10m.
        #[arg(short = 'd', long, value_parser = parse_strict_duration, conflicts_with = "requests")]
        duration: Option<Duration>,

        /// Total number of requests to send.
        /// Mutually exclusive with --duration.
        #[arg(
            short = 'r',
            long,
            help = "Total number of requests to send. Mutually exclusive with --duration.",
            default_value = "500000",
            conflicts_with = "duration"
        )]
        requests: u64,

        /// Add a custom HTTP header to the request.
        /// This option can be used multiple times. Format: "Key:Value".
        #[arg(short = 'H', long = "header", value_parser = parse_key_val, name = "KEY_VALUE")]
        headers: Vec<(String, String)>,

        /// The HTTP request body data.
        /// If the value starts with '@', the rest is treated as a file path,
        /// and its content will be read as the body.
        #[arg(short = 'b', long = "body")]
        body: Option<String>,
    },
}

/// A strict duration parser that only accepts s, ms, m, d.
fn parse_strict_duration(s: &str) -> Result<Duration, String> {
    let split_point = s.find(|c: char| !c.is_ascii_digit());

    let (num_str, unit_str) = match split_point {
        Some(idx) => s.split_at(idx),
        None => return Err("Invalid format. Must include a unit (e.g., 30s, 10m).".to_string()),
    };

    let value: u64 = num_str
        .parse()
        .map_err(|_| format!("Invalid number: '{num_str}'"))?;

    match unit_str {
        "s" => Ok(Duration::from_secs(value)),
        "ms" => Ok(Duration::from_millis(value)),
        "m" => Ok(Duration::from_secs(value * 60)),
        "d" => Ok(Duration::from_secs(value * 60 * 60 * 24)),
        _ => Err(format!(
            "Unsupported time unit: '{unit_str}'. Use 's', 'ms', 'm', or 'd'."
        )),
    }
}

fn parse_key_val(s: &str) -> Result<(String, String), String> {
    s.split_once(':')
        .map(|(key, val)| (key.trim().to_string(), val.trim().to_string()))
        .ok_or_else(|| "Header must be in 'Key:Value' format".to_string())
}

fn parse_url(s: &str) -> Result<Uri, String> {
    let uri: Uri = s.parse().map_err(|e| format!("Invalid URL format: {e}"))?;

    match uri.scheme_str() {
        Some("http") | Some("https") => (),
        Some(other) => {
            return Err(format!(
                "Unsupported scheme: '{other}'. Only 'http' or 'https' are supported."
            ))
        }
        None => return Err("URL must include a scheme (e.g., http:// or https://)".to_string()),
    }

    if uri.host().is_none() {
        return Err("URL must include a host (e.g., 'google.com')".to_string());
    }

    Ok(uri)
}
