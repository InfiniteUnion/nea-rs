# nea-rs

[![Crates.io](https://img.shields.io/crates/v/nea-rs)](https://crates.io/crates/nea-rs)
[![Crates.io Downloads](https://img.shields.io/crates/d/nea-rs)](https://crates.io/crates/nea-rs)
[![Docs.rs](https://img.shields.io/docsrs/nea-rs)](https://docs.rs/nea-rs)
[![License](https://img.shields.io/badge/license-Apache--2.0%2FMIT-blue)](#license)
[![MSRV](https://img.shields.io/badge/MSRV-1.85.1-orange)](https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/)
[![Rust Edition](https://img.shields.io/badge/Rust-2024-blue)](https://doc.rust-lang.org/edition-guide/rust-2024/)

A type-safe, sans-IO Rust client for Singapore NEA real-time weather & environmental APIs.

> [!NOTE]
> This is generated from the OpenAPI spec in [openapi](./openapi/) using [satay-rs](https://github.com/zeon256/satay-rs)

<p align="center">
  <img src="logo.png" alt="nea-rs logo" width="300">
</p>

## Installation

```bash
cargo add nea-rs
```

## Example

Fetch the latest air temperature readings with [reqwest](https://crates.io/crates/reqwest) and [satay-reqwest](https://crates.io/crates/satay-reqwest):

```rust
use nea_rs::Api;
use satay_reqwest::ReqwestActionExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = satay_reqwest::reqwest::Client::new();
    let mut api = Api::new();
    if let Ok(key) = std::env::var("X_API_KEY") {
        api = api.x_api_key(key);
    }
    let response = api.air_temperature().send_with(&client).await?;
    println!("{response:#?}");
    Ok(())
}
```

From this repo, run the same logic as a binary example:

```bash
cargo run --example air_temperature
```

Optional: set `X_API_KEY` to your [data.gov.sg](https://data.gov.sg/) API key. More runnable examples live under [`examples/`](./examples/).

## Security

Please see [SECURITY.md](SECURITY.md) for vulnerability reporting.

## License

Licensed under either of:

- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- **MIT license** ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
