use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_with::{NoneAsEmptyString, serde_as};

use crate::helpers::deserializers::track_duration;
use crate::releases::model::artist::ArtistCredit;

#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Track {
  /// `A1`, `B2`…, `None` for headings.
  #[serde_as(deserialize_as = "NoneAsEmptyString")]
  #[serde(default)]
  pub position: Option<String>,
  #[serde(rename = "type_")]
  pub kind: TrackKind,
  pub title: String,
  #[serde(default, with = "track_duration")]
  pub duration: Option<Duration>,
  /// Only when they differ from the release artists.
  #[serde(default)]
  pub artists: Vec<ArtistCredit>,
  #[serde(default, rename = "extraartists")]
  pub extra_artists: Vec<ArtistCredit>,
  /// Tracks of an `index` track (e.g. the movements of a classical work).
  #[serde(default)]
  pub sub_tracks: Vec<Track>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrackKind {
  Track,
  /// Title of a section of the tracklist, not a track.
  Heading,
  /// Group of `sub_tracks`.
  Index,
  #[serde(untagged)]
  Unknown(String),
}
