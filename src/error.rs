//! Errors returned by the client.

use std::time::Duration;

use reqwest::StatusCode;
use serde::Deserialize;

pub type Result<T, E = DiscogsClientError> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DiscogsClientError {
  #[error("HTTP error: {0}")]
  Http(#[from] reqwest::Error),

  #[error("Discogs API error ({status}): {}", message.as_deref().unwrap_or("no message"))]
  Api {
    status: StatusCode,
    message: Option<String>,
  },

  /// Too many requests: Discogs asks to wait `retry_after` before the next one.
  #[error("rate limited by Discogs, retry after {retry_after:?}")]
  RateLimited { retry_after: Option<Duration> },

  #[error("invalid query: {0}")]
  InvalidQuery(String),

  #[error("invalid OAuth response: {0}")]
  OAuth(String),

  #[error("invalid user agent: Discogs requires a non-empty, printable User-Agent")]
  InvalidUserAgent,

  #[cfg(feature = "xml-exports")]
  #[error(transparent)]
  Export(#[from] crate::xml_exports::error::ExportError),
}

/// Error body of the REST API
#[derive(Debug, Deserialize)]
pub(crate) struct ApiErrorBody {
  pub(crate) message: Option<String>,
}
