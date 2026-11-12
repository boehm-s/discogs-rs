use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::helpers::deserializers::ZeroAsNone;
use crate::helpers::ids::{MasterReleaseId, ReleaseId};
use crate::releases::model::artist::ArtistCredit;
use crate::releases::model::media::{Image, Video};
use crate::releases::model::status::DataQuality;
use crate::releases::model::track::Track;

/// Group of all the versions of a record (all its pressings, reissues, formats…).
#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MasterRelease {
  pub id: MasterReleaseId,
  pub title: String,
  #[serde_as(deserialize_as = "ZeroAsNone")]
  #[serde(default)]
  pub year: Option<u32>,
  /// Release representing the master, usually the original one.
  pub main_release: ReleaseId,
  pub main_release_url: String,
  pub most_recent_release: Option<ReleaseId>,
  pub most_recent_release_url: Option<String>,
  pub resource_url: String,
  pub uri: String,
  pub versions_url: String,
  #[serde(default)]
  pub artists: Vec<ArtistCredit>,
  #[serde(default)]
  pub genres: Vec<String>,
  #[serde(default)]
  pub styles: Vec<String>,
  #[serde(default)]
  pub tracklist: Vec<Track>,
  #[serde(default)]
  pub images: Vec<Image>,
  #[serde(default)]
  pub videos: Vec<Video>,
  pub num_for_sale: Option<u32>,
  /// `None` when nothing is for sale.
  pub lowest_price: Option<Decimal>,
  pub data_quality: DataQuality,
}
