//! Fetch latest lightning observations (`/weather?api=lightning`).
//!
//! ```bash
//! cargo run --example weather_lightning
//! ```

mod common;

use nea_rs::NeaWeatherSubApi;
use satay_reqwest::{ReqwestActionExt, reqwest};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = reqwest::Client::new();
    let api = common::api_from_env();
    let response = api
        .weather_sub_api(NeaWeatherSubApi::Lightning)
        .send_with(&client)
        .await?;
    println!("{response:#?}");
    Ok(())
}
