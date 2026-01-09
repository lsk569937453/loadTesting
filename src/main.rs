mod app;
mod engine;
mod master;
mod output;
mod protocol;
mod vojo;
mod worker;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate tracing;
use crate::app::run::main_with_error;
#[tokio::main]
async fn main() {
    if let Err(e) = main_with_error().await {
        eprintln!("{}", e);
    }
}
