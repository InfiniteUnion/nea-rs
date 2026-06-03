//! Fetch latest twenty-four-hour forecast.
//!
//! ```bash
//! cargo run --example twenty_four_hr_forecast
//! ```

mod common;

use satay_reqwest::ReqwestActionExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = satay_reqwest::reqwest::Client::new();
    let api = common::api_from_env();
    let response = api.twenty_four_hr_forecast().send_with(&client).await?;
    println!("{response:#?}");
    Ok(())
}
