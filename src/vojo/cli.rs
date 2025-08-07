use clap::Parser;
use http::Uri;
use std::time::Duration;
/// A simple yet powerful HTTP stress testing tool.
#[derive(Parser, Clone, Debug)]
#[command(author, version, about, long_about)]
pub struct Cli {
    /// The URL to benchmark, e.g., http://localhost:8080/
    #[arg(value_parser = parse_url)]
    pub url: Uri,

    /// Number of concurrent workers (threads) to run.
    #[arg(short = 'c', long, default_value_t = 50)]
    pub concurrency: u16,

    /// Duration of the test. Stops when the duration is reached.
    /// Mutually exclusive with --requests. e.g., 30s, 10m.
    #[arg(short = 'd', long, value_parser = parse_strict_duration, conflicts_with = "requests")]
    pub duration: Option<Duration>,

    /// Total number of requests to send.
    /// Mutually exclusive with --duration.
    // Use `default_value` to set the default.
    // Clap will correctly report a conflict if -d is used with this default.
    // Also, requests must now be required or have a default. We give it a default.
    #[arg(
        short = 'r',
        long,
        help = "Total number of requests to send. Mutually exclusive with --duration.",
        default_value = "500000",
        conflicts_with = "duration"
    )]
    pub requests: u64,
    /// Add a custom HTTP header to the request.
    /// This option can be used multiple times. Format: "Key:Value".
    #[arg(short = 'H', long = "header", value_parser = parse_key_val, name = "KEY_VALUE")]
    pub headers: Vec<(String, String)>,

    /// The HTTP request body data.
    /// If the value starts with '@', the rest is treated as a file path,
    /// and its content will be read as the body.
    #[arg(short = 'b', long = "body")]
    pub body: Option<String>,
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
        Some("http") | Some("https") => (), // Scheme is valid, continue.
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
