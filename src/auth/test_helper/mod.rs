//! Mocks of the authentication endpoints.

pub(crate) mod identity_api;
pub(crate) mod oauth_api;

use wiremock::MockServer;

use crate::client::Client;

pub(crate) const USER_AGENT: &str = "discogs-client-tests/1.0 +https://github.com/boehm-s/discogs-rs";

pub(crate) fn client(server: &MockServer) -> Client {
  Client::builder(USER_AGENT).base_url(server.uri()).build().unwrap()
}
