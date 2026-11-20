use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::auth::test_helper::USER_AGENT;
use crate::client::Client;

/// `?prefix=data/`: years 2008 to 2026
pub(crate) const YEARS_PAGE: &str = include_str!("../../../resources/xml_exports/listing_years.html");
/// `?prefix=data/2026/`: exports of January to November 2026
pub(crate) const EXPORTS_2026_PAGE: &str = include_str!("../../../resources/xml_exports/listing_2026.html");
pub(crate) const CHECKSUMS: &str = include_str!("../../../resources/xml_exports/checksum.txt");

pub(crate) fn exports_client(server: &MockServer) -> Client {
  Client::builder(USER_AGENT)
    .personal_token("secret-token")
    .exports_url(server.uri())
    .build()
    .unwrap()
}

pub(crate) async fn mount_listing_api(server: &MockServer, prefix: &str, page: &str) {
  Mock::given(method("GET"))
    .and(path("/"))
    .and(query_param("prefix", prefix))
    .and(header("User-Agent", USER_AGENT))
    .respond_with(ResponseTemplate::new(200).set_body_raw(page, "text/html"))
    .expect(1)
    .mount(server)
    .await;
}

pub(crate) async fn mount_download_api(server: &MockServer, file: &str, response: ResponseTemplate) {
  Mock::given(method("GET"))
    .and(path("/"))
    .and(query_param("download", file))
    .and(header("User-Agent", USER_AGENT))
    .respond_with(response)
    .expect(1)
    .mount(server)
    .await;
}
