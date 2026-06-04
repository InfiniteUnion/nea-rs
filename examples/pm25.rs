//! Fetch latest PM2.5 readings.
//!
//! ```bash
//! cargo run --example pm25
//! ```

mod common;

use satay_reqwest::{ReqwestActionExt, reqwest};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = reqwest::Client::new();
    let api = common::api_from_env();
    let response = api.pm25().send_with(&client).await?;
    println!("{response:#?}");
    Ok(())
}
