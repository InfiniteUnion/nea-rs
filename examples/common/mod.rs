//! Shared helpers for NEA realtime API examples.

use std::env;

pub fn api_from_env() -> nea_rs::Api<Box<str>> {
    let mut api = nea_rs::Api::default();
    if let Ok(key) = env::var("X_API_KEY") {
        api = api.x_api_key(key);
    }
    api
}
