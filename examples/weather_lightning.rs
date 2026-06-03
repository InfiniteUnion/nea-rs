//! Fetch latest lightning observations (`/weather?api=lightning`).
//!
//! ```bash
//! cargo run --example weather_lightning
//! ```

mod common;

use nea_rs::NeaWeatherSubApi;
use satay_reqwest::ReqwestActionExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = satay_reqwest::reqwest::Client::new();
    let api = common::api_from_env();
    let response = api
        .weather_sub_api(NeaWeatherSubApi::Lightning)
        .send_with(&client)
        .await?;
    println!("{response:#?}");
    Ok(())
}
