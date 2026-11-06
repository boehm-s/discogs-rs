//! Rust client for the [Discogs API](https://www.discogs.com/developers).

/// Base URL of the Discogs API.
pub const API_BASE_URL: &str = "https://api.discogs.com";

#[cfg(test)]
mod tests {
  use super::API_BASE_URL;

  #[test]
  fn api_base_url_uses_https() {
    assert!(API_BASE_URL.starts_with("https://"));
  }
}
