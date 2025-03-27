use clap::Parser;

#[derive(Parser, Clone)]
#[command(author, version, about, long_about)]
pub struct Cli {
    /// The request url,like http://www.google.com
    pub url: String,
    /// Number of workers to run concurrently. Total number of requests cannot
    ///  be smaller than the concurrency level. Default is 50..
    #[arg(
        short = 'c',
        long,
        value_name = "Number of workers",
        default_value_t = 50
    )]
    pub threads: u16,
    /// Duration of application to send requests. When duration is reached,application stops and exits.
    #[arg(
        short = 'z',
        long,
        value_name = "Duration of application to send requests",
        default_value_t = 5
    )]
    pub sleep_seconds: u64,
    /// The http headers.
    #[arg(short = 'H', long = "header", value_name = "header/@file")]
    pub headers: Vec<String>,
    /// HTTP POST data.
    #[arg(short = 'd', long = "data", value_name = "data")]
    pub body_option: Option<String>,
}
