use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::helpers::ids::{LabelId, MasterReleaseId, ReleaseId};
use crate::releases::model::media::Video;
use crate::releases::model::release::{Format, Identifier};
use crate::releases::model::release_date::ReleaseDate;
use crate::releases::model::status::{DataQuality, ReleaseStatus};
use crate::xml_exports::model::artist::ArtistCredit;

/// Release of the `releases` export.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExportRelease {
  pub id: ReleaseId,
  pub status: Option<ReleaseStatus>,
  pub title: String,
  pub artists: Vec<ArtistCredit>,
  /// Producers, musicians, engineers…
  pub extra_artists: Vec<ArtistCredit>,
  pub labels: Vec<CatalogEntry>,
  pub series: Vec<CatalogEntry>,
  /// Pressing plants, studios, publishers…
  pub companies: Vec<Company>,
  pub formats: Vec<Format>,
  pub genres: Vec<String>,
  pub styles: Vec<String>,
  pub country: Option<String>,
  pub released: Option<ReleaseDate>,
  pub notes: Option<String>,
  pub data_quality: Option<DataQuality>,
  pub master_id: Option<MasterReleaseId>,
  /// Whether this release is the main release of its master.
  pub is_main_release: bool,
  pub tracklist: Vec<Track>,
  /// Barcodes, matrix numbers, label codes…
  pub identifiers: Vec<Identifier>,
  pub videos: Vec<Video>,
}

/// Label or series of a release, with the catalog number of the release.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct CatalogEntry {
  /// `None` for labels not in the database.
  pub id: Option<LabelId>,
  pub name: String,
  pub catno: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Company {
  pub id: Option<LabelId>,
  pub name: String,
  pub catno: Option<String>,
  pub entity_type: Option<u32>,
  /// Role of the company (`Pressed By`, `Recorded At`, `Phonographic Copyright (p)`…).
  pub entity_type_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Track {
  /// `A1`, `B2`…, `None` for headings.
  pub position: Option<String>,
  pub title: String,
  pub duration: Option<Duration>,
  /// Only when they differ from the release artists.
  pub artists: Vec<ArtistCredit>,
  pub extra_artists: Vec<ArtistCredit>,
  /// Tracks of an index track (e.g. the movements of a classical work).
  pub sub_tracks: Vec<Track>,
}
