use serde::{Deserialize, Serialize};

use crate::helpers::ids::ArtistId;
use crate::releases::model::status::DataQuality;
use crate::xml_exports::model::common::Reference;

/// Artist of the `artists` export.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExportArtist {
  pub id: ArtistId,
  pub name: String,
  pub real_name: Option<String>,
  pub profile: Option<String>,
  pub data_quality: Option<DataQuality>,
  pub urls: Vec<String>,
  /// Other spellings of the name, as credited on releases.
  pub name_variations: Vec<String>,
  /// Other names of the artist.
  pub aliases: Vec<Reference<ArtistId>>,
  /// Members of a group.
  pub members: Vec<Reference<ArtistId>>,
  /// Groups the artist is a member of.
  pub groups: Vec<Reference<ArtistId>>,
}

/// Artist credited on a master, a release or a track.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ArtistCredit {
  /// `None` for artists not in the database.
  pub id: Option<ArtistId>,
  pub name: String,
  /// Name variation used on this release
  pub anv: Option<String>,
  /// Text joining this artist to the next one (`&`, `feat.`…).
  pub join: Option<String>,
  /// Role of extra artists (`Producer`, `Written-By`…).
  pub role: Option<String>,
  /// Tracks of extra artists (`A1 to A3, B2`).
  pub tracks: Option<String>,
}
