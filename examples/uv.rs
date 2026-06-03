//! Fetch latest UV index.
//!
//! ```bash
//! cargo run --example uv
//! ```

mod common;

use satay_reqwest::ReqwestActionExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = satay_reqwest::reqwest::Client::new();
    let api = common::api_from_env();
    let response = api.uv().send_with(&client).await?;
    println!("{response:#?}");
    Ok(())
}
