//! Shared helpers for NEA realtime API examples.

pub fn api_from_env() -> nea_rs::Api {
    let mut api = nea_rs::Api::new();
    if let Ok(key) = std::env::var("X_API_KEY") {
        api = api.x_api_key(key);
    }
    api
}
