use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::helpers::ids::ReleaseId;

/// Community rating, out of 5.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Rating {
  pub count: u32,
  pub average: Decimal,
}

/// `GET /releases/{release_id}/rating`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReleaseRating {
  pub release_id: ReleaseId,
  pub rating: Rating,
}

/// `GET /releases/{release_id}/stats`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReleaseStats {
  pub is_offensive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ReleaseRatingAndStats {
  pub rating: ReleaseRating,
  pub stats: ReleaseStats,
}
