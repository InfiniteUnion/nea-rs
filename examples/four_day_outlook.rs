//! Fetch latest four-day weather outlook.
//!
//! ```bash
//! cargo run --example four_day_outlook
//! ```

mod common;

use satay_reqwest::{ReqwestActionExt, reqwest};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = reqwest::Client::new();
    let api = common::api_from_env();
    let response = api.four_day_outlook().send_with(&client).await?;
    println!("{response:#?}");
    Ok(())
}
