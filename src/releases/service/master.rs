use serde::Serialize;
use tracing::instrument;

use crate::client::Client;
use crate::error::Result;
use crate::helpers::ids::MasterReleaseId;
use crate::helpers::pagination::{PaginatedResponse, Pagination};
use crate::releases::model::master_release::MasterRelease;
use crate::releases::model::master_version::{MasterVersion, VersionFilters};

#[derive(Serialize)]
struct VersionsQuery<'a> {
  #[serde(flatten)]
  pagination: Pagination,
  #[serde(flatten)]
  filters: &'a VersionFilters,
}

/// `GET /masters/{master_id}`
#[instrument(skip(client))]
pub(crate) async fn fetch_master_release(client: &Client, master_id: MasterReleaseId) -> Result<MasterRelease> {
  client.get(&format!("/masters/{master_id}"), &()).await
}

/// `GET /masters/{master_id}/versions{?page,per_page,format,label,released,country,sort,sort_order}`
#[instrument(skip(client))]
pub(crate) async fn fetch_master_versions(
  client: &Client,
  master_id: MasterReleaseId,
  filters: &VersionFilters,
  pagination: Pagination,
) -> Result<PaginatedResponse<MasterVersion>> {
  client
    .get(
      &format!("/masters/{master_id}/versions"),
      &VersionsQuery { pagination, filters },
    )
    .await
}

#[cfg(test)]
mod tests {
  use reqwest::StatusCode;
  use wiremock::MockServer;

  use super::{fetch_master_release, fetch_master_versions};
  use crate::auth::test_helper::client;
  use crate::error::DiscogsClientError;
  use crate::helpers::ids::{MasterReleaseId, ReleaseId};
  use crate::helpers::pagination::{Pagination, SortOrder};
  use crate::releases::model::master_release::MasterRelease;
  use crate::releases::model::master_version::{VersionFilters, VersionSort};
  use crate::releases::model::status::ReleaseStatus;
  use crate::releases::test_helper::master_api::{
    MASTER_ID, MASTER_RELEASE, MASTER_VERSIONS, MASTER_VERSIONS_UNAUTHENTICATED, mount_master_release_api,
    mount_master_versions_api,
  };

  #[tokio::test]
  async fn fetch_master_release_ok() {
    let server = MockServer::start().await;
    mount_master_release_api(&server, MASTER_ID, 200, MASTER_RELEASE).await;

    let master = fetch_master_release(&client(&server), MASTER_ID).await.unwrap();

    assert_eq!(master, serde_json::from_str::<MasterRelease>(MASTER_RELEASE).unwrap());
    assert_eq!(master.main_release, ReleaseId::new(2_287_669));
    assert_eq!(master.tracklist.len(), 5);
  }

  #[tokio::test]
  async fn fetch_master_release_not_found() {
    let server = MockServer::start().await;
    let master_id = MasterReleaseId::new(0);
    mount_master_release_api(
      &server,
      master_id,
      404,
      include_str!("../../../resources/releases/master_not_found.json"),
    )
    .await;

    match fetch_master_release(&client(&server), master_id).await {
      Err(DiscogsClientError::Api { status, message }) => {
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(message.as_deref(), Some("Master Release not found."));
      }
      other => panic!("expected an API error, got {other:?}"),
    }
  }

  #[tokio::test]
  async fn fetch_master_versions_sends_filters_and_pagination() {
    let server = MockServer::start().await;
    mount_master_versions_api(
      &server,
      MASTER_ID,
      &[
        ("page", "1"),
        ("per_page", "3"),
        ("format", "Vinyl"),
        ("country", "UK"),
        ("sort", "released"),
        ("sort_order", "desc"),
      ],
      MASTER_VERSIONS,
    )
    .await;
    let filters = VersionFilters {
      format: Some("Vinyl".to_owned()),
      country: Some("UK".to_owned()),
      sort: Some(VersionSort::Released),
      sort_order: Some(SortOrder::Desc),
      ..VersionFilters::default()
    };

    let versions = fetch_master_versions(
      &client(&server),
      MASTER_ID,
      &filters,
      Pagination {
        page: 1,
        number_of_items_per_page: 3,
      },
    )
    .await
    .unwrap();

    let pagination = versions.pagination;
    assert_eq!(
      (
        pagination.page,
        pagination.pages_count,
        pagination.number_of_items_per_page,
        pagination.items_count
      ),
      (1, 159, 3, 475)
    );
    let version = &versions.results[0];
    assert_eq!(version.id, ReleaseId::new(5_564_019));
    assert_eq!(version.status, ReleaseStatus::Accepted);
    assert_eq!(version.catno.as_deref(), Some("SD 8245"));
    assert_eq!(version.stats.community.in_collection, 526);
    assert!(version.stats.user.is_some());
  }

  #[tokio::test]
  async fn fetch_master_versions_unauthenticated() {
    let server = MockServer::start().await;
    mount_master_versions_api(&server, MASTER_ID, &[], MASTER_VERSIONS_UNAUTHENTICATED).await;

    let versions = fetch_master_versions(
      &client(&server),
      MASTER_ID,
      &VersionFilters::default(),
      Pagination::default(),
    )
    .await
    .unwrap();

    assert_eq!(versions.results[0].stats.user, None);
  }
}
