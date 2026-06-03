//! Fetch latest PM2.5 readings.
//!
//! ```bash
//! cargo run --example pm25
//! ```

mod common;

use satay_reqwest::ReqwestActionExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = satay_reqwest::reqwest::Client::new();
    let api = common::api_from_env();
    let response = api.pm25().send_with(&client).await?;
    println!("{response:#?}");
    Ok(())
}
