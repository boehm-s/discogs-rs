use serde::{Deserialize, Serialize};

use crate::helpers::ids::{MasterReleaseId, ReleaseId};
use crate::releases::model::media::Video;
use crate::releases::model::status::DataQuality;
use crate::xml_exports::model::artist::ArtistCredit;

/// Master release of the `masters` export.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExportMaster {
  pub id: MasterReleaseId,
  /// Release representing the master, usually the original one.
  pub main_release: Option<ReleaseId>,
  pub title: String,
  pub year: Option<u32>,
  pub artists: Vec<ArtistCredit>,
  pub genres: Vec<String>,
  pub styles: Vec<String>,
  pub notes: Option<String>,
  pub data_quality: Option<DataQuality>,
  pub videos: Vec<Video>,
}
