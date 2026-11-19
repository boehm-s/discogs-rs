//! Records of the exports, and the exports themselves.
//!
//! Records are `non_exhaustive`: Discogs may add fields to its exports, and adding them here must
//! not be a breaking change.

pub mod artist;
pub mod common;
pub mod export;
pub mod label;
pub mod master;
pub mod release;

use crate::xml_exports::model::export::ExportKind;
use crate::xml_exports::parser::sealed;

/// Record of an export: [`ExportArtist`](artist::ExportArtist),
/// [`ExportLabel`](label::ExportLabel), [`ExportMaster`](master::ExportMaster) or
/// [`ExportRelease`](release::ExportRelease).
pub trait ExportRecord: Clone + std::fmt::Debug + sealed::Parse {
  const KIND: ExportKind;
}
