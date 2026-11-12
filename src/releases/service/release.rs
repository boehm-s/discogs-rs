use futures_util::future::try_join;
use tracing::instrument;

use crate::client::Client;
use crate::error::Result;
use crate::helpers::ids::ReleaseId;
use crate::releases::model::rating::{ReleaseRating, ReleaseRatingAndStats, ReleaseStats};
use crate::releases::model::release::Release;

/// `GET /releases/{release_id}`
#[instrument(skip(client))]
pub(crate) async fn fetch_release(client: &Client, release_id: ReleaseId) -> Result<Release> {
  client.get(&format!("/releases/{release_id}"), &()).await
}

/// `GET /releases/{release_id}/rating` and `GET /releases/{release_id}/stats`, called concurrently.
#[instrument(skip(client))]
pub(crate) async fn fetch_release_rating_and_stats(
  client: &Client,
  release_id: ReleaseId,
) -> Result<ReleaseRatingAndStats> {
  let (rating_path, stats_path) = (
    format!("/releases/{release_id}/rating"),
    format!("/releases/{release_id}/stats"),
  );
  let (rating, stats) = try_join(
    client.get::<ReleaseRating>(&rating_path, &()),
    client.get::<ReleaseStats>(&stats_path, &()),
  )
  .await?;
  Ok(ReleaseRatingAndStats { rating, stats })
}

#[cfg(test)]
mod tests {
  use std::time::Duration;

  use chrono::NaiveDate;
  use reqwest::StatusCode;
  use rust_decimal::Decimal;
  use wiremock::MockServer;

  use super::{fetch_release, fetch_release_rating_and_stats};
  use crate::auth::test_helper::client;
  use crate::error::DiscogsClientError;
  use crate::helpers::ids::{ArtistId, LabelId, MasterReleaseId, ReleaseId};
  use crate::helpers::publication::Publication;
  use crate::releases::model::release::Release;
  use crate::releases::model::release_date::ReleaseDate;
  use crate::releases::model::status::{DataQuality, ReleaseStatus};
  use crate::releases::model::track::TrackKind;
  use crate::releases::test_helper::release_api::{
    RELEASE, RELEASE_ID, mount_release_api, mount_release_rating_api, mount_release_stats_api,
  };

  #[tokio::test]
  async fn fetch_release_ok() {
    let server = MockServer::start().await;
    mount_release_api(&server, RELEASE_ID, 200, RELEASE).await;

    let release = fetch_release(&client(&server), RELEASE_ID).await.unwrap();

    assert_eq!(release, serde_json::from_str::<Release>(RELEASE).unwrap());
    assert_eq!(
      release.title,
      "In The Court Of The Crimson King (An Observation By King Crimson)"
    );
    assert_eq!(release.status, ReleaseStatus::Accepted);
    assert_eq!(release.data_quality, DataQuality::NeedsVote);
    assert_eq!(release.master_id, Some(MasterReleaseId::new(406_634)));
    assert_eq!(
      release.released,
      Some(ReleaseDate::Known(
        NaiveDate::from_ymd_opt(1969, 11, 1).unwrap()..=NaiveDate::from_ymd_opt(1969, 11, 30).unwrap()
      ))
    );
    assert_eq!(release.estimated_weight_in_grams, Some(230));
    assert_eq!(release.lowest_price, Some(Decimal::new(178, 1)));
    assert_eq!(release.formats[0].quantity, Some(1));
    assert_eq!(
      release.artists[0].publication,
      Publication::Published {
        id: ArtistId::new(70_828),
        resource_url: "https://api.discogs.com/artists/70828".to_owned(),
      }
    );
    assert_eq!(release.artists[0].anv, None);
    assert_eq!(release.labels[0].catno.as_deref(), Some("SD 8245"));
    assert!(matches!(
      release.labels[0].publication,
      Publication::Published { id, .. } if id == LabelId::new(681)
    ));
    assert_eq!(release.tracklist[0].kind, TrackKind::Track);
    assert_eq!(release.tracklist[0].duration, Some(Duration::from_secs(7 * 60 + 21)));
    assert_eq!(
      release.community.map(|community| community.rating.average),
      Some(Decimal::new(456, 2))
    );
  }

  #[tokio::test]
  async fn fetch_release_not_found() {
    let server = MockServer::start().await;
    let release_id = ReleaseId::new(0);
    mount_release_api(
      &server,
      release_id,
      404,
      include_str!("../../../resources/releases/release_not_found.json"),
    )
    .await;

    match fetch_release(&client(&server), release_id).await {
      Err(DiscogsClientError::Api { status, message }) => {
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(
          message.as_deref(),
          Some("That release does not exist or may have been deleted.")
        );
      }
      other => panic!("expected an API error, got {other:?}"),
    }
  }

  #[tokio::test]
  async fn fetch_release_rating_and_stats_ok() {
    let server = MockServer::start().await;
    mount_release_rating_api(&server, RELEASE_ID).await;
    mount_release_stats_api(&server, RELEASE_ID).await;

    let rating_and_stats = fetch_release_rating_and_stats(&client(&server), RELEASE_ID)
      .await
      .unwrap();

    assert_eq!(rating_and_stats.rating.release_id, RELEASE_ID);
    assert_eq!(rating_and_stats.rating.rating.count, 556);
    assert_eq!(rating_and_stats.rating.rating.average, Decimal::new(456, 2));
    assert!(!rating_and_stats.stats.is_offensive);
  }
}
