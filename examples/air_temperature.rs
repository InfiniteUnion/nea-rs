//! Fetch latest air temperature readings.
//!
//! ```bash
//! cargo run --example air_temperature
//! ```

mod common;

use satay_reqwest::{ReqwestActionExt, reqwest};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = reqwest::Client::new();
    let api = common::api_from_env();
    let response = api.air_temperature().send_with(&client).await?;
    println!("{response:#?}");
    Ok(())
}
