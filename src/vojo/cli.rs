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

    /// Duration of the test. The application will stop and exit when the duration is reached.
    /// Supported formats: "10s", "1m", "2h30m".
    #[arg(short = 'd', long, value_parser = humantime::parse_duration, default_value = "10s")]
    pub duration: Duration,

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

/// Parses a "Key:Value" string into a (String, String) tuple.
fn parse_key_val(s: &str) -> Result<(String, String), String> {
    s.split_once(':')
        .map(|(key, val)| (key.trim().to_string(), val.trim().to_string()))
        .ok_or_else(|| "Header must be in 'Key:Value' format".to_string())
}

/// Parses a string into a valid HTTP/HTTPS Uri.
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
