//! Releases endpoints added to [`Client`], and links between the release models.

use tracing::instrument;

use crate::client::Client;
use crate::error::Result;
use crate::helpers::ids::{MasterReleaseId, ReleaseId};
use crate::helpers::pagination::{PaginatedResponse, Pagination};
use crate::releases::model::master_release::MasterRelease;
use crate::releases::model::master_version::{MasterVersion, VersionFilters};
use crate::releases::model::rating::ReleaseRatingAndStats;
use crate::releases::model::release::Release;
use crate::releases::service::master::{fetch_master_release, fetch_master_versions};
use crate::releases::service::release::{fetch_release, fetch_release_rating_and_stats};

impl Client {
  #[instrument(skip_all)]
  pub async fn get_release(&self, release_id: ReleaseId) -> Result<Release> {
    fetch_release(self, release_id).await
  }

  #[instrument(skip_all)]
  pub async fn get_release_rating_and_stats(&self, release_id: ReleaseId) -> Result<ReleaseRatingAndStats> {
    fetch_release_rating_and_stats(self, release_id).await
  }

  #[instrument(skip_all)]
  pub async fn get_master_release(&self, master_id: MasterReleaseId) -> Result<MasterRelease> {
    fetch_master_release(self, master_id).await
  }

  #[instrument(skip_all)]
  pub async fn list_master_versions(
    &self,
    master_id: MasterReleaseId,
    filters: &VersionFilters,
    pagination: Pagination,
  ) -> Result<PaginatedResponse<MasterVersion>> {
    fetch_master_versions(self, master_id, filters, pagination).await
  }
}

impl Release {
  /// `None` when the release does not belong to a master.
  #[instrument(skip_all)]
  pub async fn get_master_release(&self, client: &Client) -> Result<Option<MasterRelease>> {
    match self.master_id {
      Some(master_id) => Ok(Some(fetch_master_release(client, master_id).await?)),
      None => Ok(None),
    }
  }

  #[instrument(skip_all)]
  pub async fn get_rating_and_stats(&self, client: &Client) -> Result<ReleaseRatingAndStats> {
    fetch_release_rating_and_stats(client, self.id).await
  }
}

impl MasterRelease {
  #[instrument(skip_all)]
  pub async fn list_versions(
    &self,
    client: &Client,
    filters: &VersionFilters,
    pagination: Pagination,
  ) -> Result<PaginatedResponse<MasterVersion>> {
    fetch_master_versions(client, self.id, filters, pagination).await
  }

  #[instrument(skip_all)]
  pub async fn get_main_release(&self, client: &Client) -> Result<Release> {
    fetch_release(client, self.main_release).await
  }
}

impl MasterVersion {
  #[instrument(skip_all)]
  pub async fn get_release(&self, client: &Client) -> Result<Release> {
    fetch_release(client, self.id).await
  }
}

#[cfg(test)]
mod tests {
  use wiremock::MockServer;

  use crate::auth::test_helper::client;
  use crate::helpers::pagination::Pagination;
  use crate::releases::model::master_release::MasterRelease;
  use crate::releases::model::master_version::VersionFilters;
  use crate::releases::model::release::Release;
  use crate::releases::test_helper::master_api::{
    MASTER_ID, MASTER_RELEASE, MASTER_VERSIONS, mount_master_release_api, mount_master_versions_api,
  };
  use crate::releases::test_helper::release_api::{RELEASE, RELEASE_ID, mount_release_api};

  fn release() -> Release {
    serde_json::from_str(RELEASE).unwrap()
  }

  fn master() -> MasterRelease {
    serde_json::from_str(MASTER_RELEASE).unwrap()
  }

  #[tokio::test]
  async fn release_to_master_release() {
    let server = MockServer::start().await;
    mount_master_release_api(&server, MASTER_ID, 200, MASTER_RELEASE).await;

    let master = release().get_master_release(&client(&server)).await.unwrap();

    assert_eq!(master.map(|master| master.id), Some(MASTER_ID));
  }

  #[tokio::test]
  async fn release_without_master() {
    // No mock: any request would fail
    let server = MockServer::start().await;
    let mut release = release();
    release.master_id = None;

    assert_eq!(release.get_master_release(&client(&server)).await.unwrap(), None);
  }

  #[tokio::test]
  async fn master_release_to_versions_and_back_to_a_release() {
    let server = MockServer::start().await;
    mount_master_versions_api(&server, MASTER_ID, &[("page", "1"), ("per_page", "3")], MASTER_VERSIONS).await;
    let client = client(&server);

    let versions = master()
      .list_versions(
        &client,
        &VersionFilters::default(),
        Pagination {
          page: 1,
          number_of_items_per_page: 3,
        },
      )
      .await
      .unwrap();
    // The mock answers the recorded release whatever the version
    mount_release_api(&server, versions.results[0].id, 200, RELEASE).await;
    versions.results[0].get_release(&client).await.unwrap();
  }

  #[tokio::test]
  async fn master_release_to_main_release() {
    let server = MockServer::start().await;
    let master = master();
    mount_release_api(&server, master.main_release, 200, RELEASE).await;

    let release = master.get_main_release(&client(&server)).await.unwrap();

    assert_eq!(release.id, RELEASE_ID);
  }
}

#[cfg(test)]
#[cfg(feature = "live-tests")]
mod live_tests {
  use crate::auth::test_helper::USER_AGENT;
  use crate::client::Client;
  use crate::helpers::pagination::Pagination;
  use crate::releases::model::master_version::VersionFilters;
  use crate::releases::test_helper::release_api::RELEASE_ID;

  #[tokio::test]
  async fn browse_release_master_and_versions() {
    let token = std::env::var("DISCOGS_TOKEN").expect("DISCOGS_TOKEN is required by the live tests");
    let client = Client::builder(USER_AGENT).personal_token(token).build().unwrap();

    let release = client.get_release(RELEASE_ID).await.unwrap();
    let rating_and_stats = release.get_rating_and_stats(&client).await.unwrap();
    assert_eq!(rating_and_stats.rating.release_id, RELEASE_ID);

    let master = release.get_master_release(&client).await.unwrap().unwrap();
    let versions = master
      .list_versions(
        &client,
        &VersionFilters::default(),
        Pagination {
          page: 1,
          number_of_items_per_page: 2,
        },
      )
      .await
      .unwrap();
    assert_eq!(versions.results.len(), 2);
    assert!(versions.results[0].stats.user.is_some());
  }
}
