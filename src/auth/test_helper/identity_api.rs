use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::USER_AGENT;

pub(crate) async fn mount_identity_api(server: &MockServer, status: u16, response: &str) {
  Mock::given(method("GET"))
    .and(path("/oauth/identity"))
    .and(header("User-Agent", USER_AGENT))
    .respond_with(ResponseTemplate::new(status).set_body_raw(response, mime::APPLICATION_JSON.as_ref()))
    .expect(1)
    .mount(server)
    .await;
}
