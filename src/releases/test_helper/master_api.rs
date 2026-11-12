use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::auth::test_helper::USER_AGENT;
use crate::helpers::ids::MasterReleaseId;

/// Master of the recorded release
pub(crate) const MASTER_ID: MasterReleaseId = MasterReleaseId::new(406_634);

pub(crate) const MASTER_RELEASE: &str = include_str!("../../../resources/releases/master_release.json");

/// First page, 3 versions per page, recorded with a personal token
pub(crate) const MASTER_VERSIONS: &str = include_str!("../../../resources/releases/master_versions.json");

/// Same page, recorded without authentication
pub(crate) const MASTER_VERSIONS_UNAUTHENTICATED: &str =
  include_str!("../../../resources/releases/master_versions_unauthenticated.json");

fn json(status: u16, response: &str) -> ResponseTemplate {
  ResponseTemplate::new(status).set_body_raw(response, mime::APPLICATION_JSON.as_ref())
}

pub(crate) async fn mount_master_release_api(
  server: &MockServer,
  master_id: MasterReleaseId,
  status: u16,
  response: &str,
) {
  Mock::given(method("GET"))
    .and(path(format!("/masters/{master_id}")))
    .and(header("User-Agent", USER_AGENT))
    .respond_with(json(status, response))
    .expect(1)
    .mount(server)
    .await;
}

/// Answers `response` when every `query` parameter is sent.
pub(crate) async fn mount_master_versions_api(
  server: &MockServer,
  master_id: MasterReleaseId,
  query: &[(&str, &str)],
  response: &str,
) {
  let mut mock = Mock::given(method("GET")).and(path(format!("/masters/{master_id}/versions")));
  for (name, value) in query {
    mock = mock.and(query_param(*name, *value));
  }
  mock.respond_with(json(200, response)).expect(1).mount(server).await;
}
