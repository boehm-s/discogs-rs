use serde::{Deserialize, Serialize};
use serde_with::{NoneAsEmptyString, serde_as, skip_serializing_none};

use crate::helpers::ids::ReleaseId;
use crate::helpers::pagination::{ApiResource, SortOrder};
use crate::releases::model::release_date::{NoDateAsNone, ReleaseDate};
use crate::releases::model::status::ReleaseStatus;

impl ApiResource for MasterVersion {
  const COLLECTION_KEY: &'static str = "versions";
}

/// Summary of a release, as listed in the versions of a master.
#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct MasterVersion {
  pub id: ReleaseId,
  pub title: String,
  pub status: ReleaseStatus,
  #[serde_as(deserialize_as = "NoneAsEmptyString")]
  #[serde(default)]
  pub label: Option<String>,
  /// Catalog number
  #[serde_as(deserialize_as = "NoneAsEmptyString")]
  #[serde(default)]
  pub catno: Option<String>,
  #[serde_as(deserialize_as = "NoneAsEmptyString")]
  #[serde(default)]
  pub country: Option<String>,
  #[serde_as(deserialize_as = "NoDateAsNone")]
  #[serde(default)]
  pub released: Option<ReleaseDate>,
  /// Format and its descriptions in one string (`Vinyl, LP, Album, Stereo`).
  pub format: String,
  #[serde(default)]
  pub major_formats: Vec<String>,
  pub resource_url: String,
  #[serde_as(deserialize_as = "NoneAsEmptyString")]
  #[serde(default)]
  pub thumb: Option<String>,
  pub stats: VersionStats,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VersionStats {
  pub community: VersionCounts,
  /// Counts of the authenticated user, only sent to authenticated clients.
  #[serde(default)]
  pub user: Option<VersionCounts>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct VersionCounts {
  pub in_wantlist: u32,
  pub in_collection: u32,
}

/// Filters and sorting of [`MasterRelease::list_versions`](crate::releases::model::master_release::MasterRelease::list_versions).
/// Filter values are the ones displayed on Discogs (`Vinyl`, `UK`, `1969`…).
#[skip_serializing_none]
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct VersionFilters {
  pub format: Option<String>,
  pub label: Option<String>,
  pub released: Option<String>,
  pub country: Option<String>,
  pub sort: Option<VersionSort>,
  pub sort_order: Option<SortOrder>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VersionSort {
  Released,
  Title,
  Format,
  Label,
  Catno,
  Country,
}
