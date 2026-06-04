//! Fetch latest wind direction readings.
//!
//! ```bash
//! cargo run --example wind_direction
//! ```

mod common;

use satay_reqwest::{ReqwestActionExt, reqwest};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = reqwest::Client::new();
    let api = common::api_from_env();
    let response = api.wind_direction().send_with(&client).await?;
    println!("{response:#?}");
    Ok(())
}
