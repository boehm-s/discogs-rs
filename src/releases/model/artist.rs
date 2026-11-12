use serde::{Deserialize, Serialize};
use serde_with::{NoneAsEmptyString, serde_as};

use crate::helpers::ids::ArtistId;
use crate::helpers::publication::Publication;

/// Artist credited on a release, a master or a track.
#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ArtistCredit {
  pub name: String,
  #[serde(flatten)]
  pub publication: Publication<ArtistId>,
  /// Artist name variation: the name as written on this release, if different.
  #[serde_as(deserialize_as = "NoneAsEmptyString")]
  #[serde(default)]
  pub anv: Option<String>,
  /// Text joining this artist to the next one (`&`, `feat.`…).
  #[serde_as(deserialize_as = "NoneAsEmptyString")]
  #[serde(default)]
  pub join: Option<String>,
  /// Role of extra artists (`Producer`, `Bass Guitar, Lead Vocals`…).
  #[serde_as(deserialize_as = "NoneAsEmptyString")]
  #[serde(default)]
  pub role: Option<String>,
  /// Tracks the credit is limited to (`A1 to A3`…), `None` when it applies to the whole release.
  #[serde_as(deserialize_as = "NoneAsEmptyString")]
  #[serde(default)]
  pub tracks: Option<String>,
  pub thumbnail_url: Option<String>,
}
