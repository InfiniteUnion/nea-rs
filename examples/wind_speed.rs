//! Fetch latest wind speed readings.
//!
//! ```bash
//! cargo run --example wind_speed
//! ```

mod common;

use satay_reqwest::ReqwestActionExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = satay_reqwest::reqwest::Client::new();
    let api = common::api_from_env();
    let response = api.wind_speed().send_with(&client).await?;
    println!("{response:#?}");
    Ok(())
}
