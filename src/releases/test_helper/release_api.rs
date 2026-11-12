use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::auth::test_helper::USER_AGENT;
use crate::helpers::ids::ReleaseId;

/// King Crimson - In The Court Of The Crimson King, US LP (1969)
pub(crate) const RELEASE_ID: ReleaseId = ReleaseId::new(878_534);

pub(crate) const RELEASE: &str = include_str!("../../../resources/releases/release.json");

fn json(status: u16, response: &str) -> ResponseTemplate {
  ResponseTemplate::new(status).set_body_raw(response, mime::APPLICATION_JSON.as_ref())
}

pub(crate) async fn mount_release_api(server: &MockServer, release_id: ReleaseId, status: u16, response: &str) {
  Mock::given(method("GET"))
    .and(path(format!("/releases/{release_id}")))
    .and(header("User-Agent", USER_AGENT))
    .respond_with(json(status, response))
    .expect(1)
    .mount(server)
    .await;
}

pub(crate) async fn mount_release_rating_api(server: &MockServer, release_id: ReleaseId) {
  Mock::given(method("GET"))
    .and(path(format!("/releases/{release_id}/rating")))
    .respond_with(json(
      200,
      include_str!("../../../resources/releases/release_rating.json"),
    ))
    .expect(1)
    .mount(server)
    .await;
}

pub(crate) async fn mount_release_stats_api(server: &MockServer, release_id: ReleaseId) {
  Mock::given(method("GET"))
    .and(path(format!("/releases/{release_id}/stats")))
    .respond_with(json(
      200,
      include_str!("../../../resources/releases/release_stats.json"),
    ))
    .expect(1)
    .mount(server)
    .await;
}
