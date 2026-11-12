use chrono::{DateTime, FixedOffset};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_with::{DisplayFromStr, NoneAsEmptyString, serde_as};

use crate::helpers::deserializers::{OutOfRangeAsNone, ZeroAsNone};
use crate::helpers::ids::{LabelId, MasterReleaseId, ReleaseId};
use crate::helpers::publication::Publication;
use crate::releases::model::artist::ArtistCredit;
use crate::releases::model::media::{Image, Video};
use crate::releases::model::rating::Rating;
use crate::releases::model::release_date::{NoDateAsNone, ReleaseDate};
use crate::releases::model::status::{DataQuality, ReleaseStatus};
use crate::releases::model::track::Track;

/// A particular edition of a record (e.g. the 1969 US LP on Atlantic).
#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Release {
  pub id: ReleaseId,
  pub title: String,
  pub status: ReleaseStatus,
  #[serde_as(deserialize_as = "ZeroAsNone")]
  #[serde(default)]
  pub year: Option<u32>,
  #[serde_as(deserialize_as = "NoDateAsNone")]
  #[serde(default)]
  pub released: Option<ReleaseDate>,
  #[serde_as(deserialize_as = "NoneAsEmptyString")]
  #[serde(default)]
  pub released_formatted: Option<String>,
  #[serde_as(deserialize_as = "NoneAsEmptyString")]
  #[serde(default)]
  pub country: Option<String>,
  pub resource_url: String,
  pub uri: String,
  #[serde(default)]
  pub artists: Vec<ArtistCredit>,
  #[serde_as(deserialize_as = "NoneAsEmptyString")]
  #[serde(default)]
  pub artists_sort: Option<String>,
  #[serde(default, rename = "extraartists")]
  pub extra_artists: Vec<ArtistCredit>,
  #[serde(default)]
  pub labels: Vec<Company>,
  #[serde(default)]
  pub series: Vec<Company>,
  #[serde(default)]
  pub companies: Vec<Company>,
  #[serde(default)]
  pub formats: Vec<Format>,
  #[serde_as(as = "OutOfRangeAsNone")]
  #[serde(default)]
  pub format_quantity: Option<u32>,
  pub data_quality: DataQuality,
  pub community: Option<Community>,
  pub date_added: Option<DateTime<FixedOffset>>,
  pub date_changed: Option<DateTime<FixedOffset>>,
  pub num_for_sale: Option<u32>,
  /// `None` when nothing is for sale.
  pub lowest_price: Option<Decimal>,
  pub master_id: Option<MasterReleaseId>,
  pub master_url: Option<String>,
  #[serde_as(deserialize_as = "NoneAsEmptyString")]
  #[serde(default)]
  pub notes: Option<String>,
  #[serde(default)]
  pub identifiers: Vec<Identifier>,
  #[serde(default)]
  pub videos: Vec<Video>,
  #[serde(default)]
  pub genres: Vec<String>,
  #[serde(default)]
  pub styles: Vec<String>,
  #[serde(default)]
  pub tracklist: Vec<Track>,
  #[serde(default)]
  pub images: Vec<Image>,
  #[serde_as(deserialize_as = "NoneAsEmptyString")]
  #[serde(default)]
  pub thumb: Option<String>,
  #[serde(rename = "estimated_weight")]
  pub estimated_weight_in_grams: Option<u32>,
  #[serde(default)]
  pub blocked_from_sale: bool,
  #[serde(default)]
  pub is_offensive: bool,
}

/// Label, series or company (pressing plant, studio…) credited on a release.
#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Company {
  pub name: String,
  #[serde(flatten)]
  pub publication: Publication<LabelId>,
  /// Catalog number
  #[serde_as(deserialize_as = "NoneAsEmptyString")]
  #[serde(default)]
  pub catno: Option<String>,
  pub entity_type: String,
  /// Role of the company (`Label`, `Pressed By`, `Phonographic Copyright (p)`…).
  pub entity_type_name: String,
  pub thumbnail_url: Option<String>,
}

#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Format {
  /// `Vinyl`, `CD`, `Cassette`…
  pub name: String,
  /// Number of items (2 for a double LP).
  #[serde_as(as = "OutOfRangeAsNone<DisplayFromStr>")]
  #[serde(rename = "qty")]
  pub quantity: Option<u32>,
  /// `LP`, `Album`, `45 RPM`…
  #[serde(default)]
  pub descriptions: Vec<String>,
  /// Free text (pressing details, color of the vinyl…).
  #[serde_as(deserialize_as = "NoneAsEmptyString")]
  #[serde(default)]
  pub text: Option<String>,
}

/// Barcode, matrix number, label code…
#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Identifier {
  #[serde(rename = "type")]
  pub kind: String,
  pub value: String,
  #[serde_as(deserialize_as = "NoneAsEmptyString")]
  #[serde(default)]
  pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Community {
  /// Number of users who have the release in their collection.
  pub have: u32,
  /// Number of users who have the release in their wantlist.
  pub want: u32,
  pub rating: Rating,
  pub submitter: Option<Contributor>,
  #[serde(default)]
  pub contributors: Vec<Contributor>,
  pub data_quality: DataQuality,
  pub status: ReleaseStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Contributor {
  pub username: String,
  pub resource_url: String,
}

#[cfg(test)]
mod tests {
  use super::Release;
  use crate::helpers::publication::Publication;
  use crate::releases::model::status::ReleaseStatus;

  #[test]
  fn deserialize_draft_release() {
    let release: Release =
      serde_json::from_str(include_str!("../../../resources/releases/draft_release.json")).unwrap();

    assert_eq!(release.status, ReleaseStatus::Draft);
    assert_eq!(release.artists[0].publication, Publication::Draft {});
    assert_eq!(release.labels[0].publication, Publication::Draft {});
    assert_eq!((release.year, release.released), (None, None));
  }

  #[test]
  fn roundtrips_through_serde() {
    let release: Release = serde_json::from_str(include_str!("../../../resources/releases/release.json")).unwrap();
    let json = serde_json::to_string(&release).unwrap();

    assert_eq!(serde_json::from_str::<Release>(&json).unwrap(), release);
  }
}
