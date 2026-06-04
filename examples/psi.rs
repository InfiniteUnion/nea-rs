//! Fetch latest PSI readings.
//!
//! ```bash
//! cargo run --example psi
//! # optional: X_API_KEY=... cargo run --example psi
//! ```

mod common;

use satay_reqwest::{ReqwestActionExt, reqwest};
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = reqwest::Client::new();
    let api = common::api_from_env();
    let response = api.psi().send_with(&client).await?;
    println!("{response:#?}");
    Ok(())
}
