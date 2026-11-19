//! Errors of the XML exports, wrapped in [`DiscogsClientError::Export`].

use crate::error::DiscogsClientError;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ExportError {
  #[error("I/O error: {0}")]
  Io(#[from] std::io::Error),

  /// Malformed XML: the export cannot be read further.
  #[error("invalid XML: {0}")]
  Xml(#[from] quick_xml::Error),

  /// Malformed export: it cannot be read further.
  #[error("invalid export: {0}")]
  InvalidExport(String),

  /// Invalid record: the next records can still be read.
  #[error("invalid export record: {0}")]
  InvalidRecord(String),

  #[error("invalid options: {0}")]
  InvalidOptions(String),

  #[error("no export found: {0}")]
  NotFound(String),

  /// The pages or the checksum file of data.discogs.com changed format.
  #[error("unexpected listing of the exports: {0}")]
  InvalidListing(String),

  /// The downloaded file differs from the one published by Discogs.
  #[error("checksum mismatch for {file}: expected {expected}, got {actual}")]
  ChecksumMismatch {
    file: String,
    expected: String,
    actual: String,
  },
}

/// Export errors of the input thread travel as I/O errors: they are restored here.
impl From<std::io::Error> for DiscogsClientError {
  fn from(error: std::io::Error) -> Self {
    match error.downcast::<ExportError>() {
      Ok(error) => error.into(),
      Err(error) => ExportError::Io(error).into(),
    }
  }
}

impl From<quick_xml::Error> for DiscogsClientError {
  fn from(error: quick_xml::Error) -> Self {
    ExportError::Xml(error).into()
  }
}

impl From<quick_xml::encoding::EncodingError> for DiscogsClientError {
  fn from(error: quick_xml::encoding::EncodingError) -> Self {
    ExportError::Xml(error.into()).into()
  }
}
