<p align="center">
  <img src="logo.png" alt="nea-rs logo" width="300">
</p>

<h1 align="center">nea-rs</h1>

<p align="center">
  <a href="https://crates.io/crates/nea-rs"><img src="https://img.shields.io/crates/v/nea-rs" alt="Crates.io"></a>
  <a href="https://crates.io/crates/nea-rs"><img src="https://img.shields.io/crates/d/nea-rs" alt="Crates.io Downloads"></a>
  <a href="https://docs.rs/nea-rs"><img src="https://img.shields.io/docsrs/nea-rs" alt="Docs.rs"></a>
  <a href="#license"><img src="https://img.shields.io/badge/license-Apache--2.0%2FMIT-blue" alt="License"></a>
  <a href="https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/"><img src="https://img.shields.io/badge/MSRV-1.85.1-orange" alt="MSRV"></a>
  <a href="https://doc.rust-lang.org/edition-guide/rust-2024/"><img src="https://img.shields.io/badge/Rust-2024-blue" alt="Rust Edition"></a>
</p>

<p align="center">
  A type-safe, sans-IO Rust client for Singapore NEA real-time weather &amp; environmental APIs.
</p>

<p align="center">
  Generated from the OpenAPI spec in <a href="./openapi/">openapi</a> using <a href="https://github.com/zeon256/satay-rs">satay-rs</a>.
</p>

## Install

```bash
cargo add nea-rs
```

## Example

Fetch the latest air temperature readings with [reqwest](https://crates.io/crates/reqwest) and [satay-reqwest](https://crates.io/crates/satay-reqwest):

```rust
use nea_rs::Api;
use satay_reqwest::ReqwestActionExt;
use satay_reqwest::reqwest::Client;
use std::env;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = Client::new();
    let mut api = Api::new();
    if let Ok(key) = env::var("X_API_KEY") {
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

## Using other client backends

`nea-rs` is [sans-IO](https://fasterthanli.me/articles/the-case-for-sans-io): generated actions build `http::Request<Vec<u8>>` values and decode responses without performing IO. Pick the HTTP client that fits your application, or use the same boundary directly in tests, WASM, or custom transports.

See [satay-rs](https://github.com/zeon256/satay-rs) for more details


### ureq

```toml
[dependencies]
nea-rs = "0.1"
satay-ureq = "0.1"
ureq = "3"
```

```rust
use nea_rs::Api;
use satay_ureq::UreqActionExt;
use satay_ureq::ureq;
use std::env;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut api = Api::new();
    if let Ok(key) = env::var("X_API_KEY") {
        api = api.x_api_key(key);
    }

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into();

    let response = api.air_temperature().send_with(&agent)?;
    println!("{response:#?}");
    Ok(())
}
```

Configure the agent with `http_status_as_error(false)` so Satay can decode typed non-2xx responses instead of ureq treating them as transport errors.

### reqwest (blocking)

Enable the `blocking` feature on both crates when you want a synchronous reqwest client:

```toml
[dependencies]
nea-rs = "0.1"
satay-reqwest = { version = "0.1", features = ["blocking"] }
reqwest = { version = "0.13.3", features = ["blocking"] }
```

```rust
use nea_rs::Api;
use satay_reqwest::ReqwestBlockingActionExt;
use satay_reqwest::reqwest::blocking;
use std::env;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let mut api = Api::new();
    if let Ok(key) = env::var("X_API_KEY") {
        api = api.x_api_key(key);
    }
    let response = api
        .air_temperature()
        .send_with(&blocking::Client::new())?;
    println!("{response:#?}");
    Ok(())
}
```

For more transport patterns (including WebSocket and custom adapters), see the [Satay transport docs](https://github.com/zeon256/satay-rs/blob/main/docs/transports.md) and the examples under [`satay-rs/examples/`](https://github.com/zeon256/satay-rs/tree/main/examples).

## Security

Please see [SECURITY.md](SECURITY.md) for vulnerability reporting.

## License

Licensed under either of:

- **Apache License, Version 2.0** ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- **MIT license** ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
