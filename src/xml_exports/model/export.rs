use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportKind {
  Artists,
  Labels,
  Masters,
  Releases,
}

impl ExportKind {
  pub const ALL: [Self; 4] = [Self::Artists, Self::Labels, Self::Masters, Self::Releases];

  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Artists => "artists",
      Self::Labels => "labels",
      Self::Masters => "masters",
      Self::Releases => "releases",
    }
  }
}

impl Display for ExportKind {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    f.write_str(self.as_str())
  }
}

/// Monthly export of the Discogs database, as listed on data.discogs.com: one gzipped XML file
/// per kind of entity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Export {
  pub(crate) date: NaiveDate,
  pub(crate) files: Vec<PublishedFile>,
  /// SHA-256 of the files (`discogs_20261101_CHECKSUM.txt`), once published.
  pub(crate) checksum_file: Option<String>,
}

impl Export {
  pub(crate) fn file(&self, kind: ExportKind) -> Option<&PublishedFile> {
    self.files.iter().find(|file| file.kind == kind)
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PublishedFile {
  pub(crate) kind: ExportKind,
  /// `discogs_20261101_releases.xml.gz`
  pub(crate) name: String,
}

/// SHA-256 of the files of an export, in hexadecimal, by file name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ExportChecksums(pub(crate) HashMap<String, String>);

impl ExportChecksums {
  pub(crate) fn get(&self, file: &PublishedFile) -> Option<&str> {
    self.0.get(&file.name).map(String::as_str)
  }
}
