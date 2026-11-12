use tracing::instrument;

use crate::auth::model::identity::Identity;
use crate::client::Client;
use crate::error::Result;

/// `GET /oauth/identity`
#[instrument(skip(client))]
pub(crate) async fn fetch_identity(client: &Client) -> Result<Identity> {
  client.get("/oauth/identity", &()).await
}

#[cfg(test)]
mod tests {
  use reqwest::StatusCode;
  use wiremock::MockServer;

  use super::fetch_identity;
  use crate::auth::model::identity::Identity;
  use crate::auth::test_helper::client;
  use crate::auth::test_helper::identity_api::mount_identity_api;
  use crate::error::DiscogsClientError;

  #[tokio::test]
  async fn fetch_identity_ok() {
    let server = MockServer::start().await;
    mount_identity_api(&server, 200, include_str!("../../../resources/auth/identity.json")).await;

    let identity = fetch_identity(&client(&server)).await.unwrap();

    assert_eq!(
      identity,
      serde_json::from_str::<Identity>(include_str!("../../../resources/auth/identity.json")).unwrap()
    );
  }

  /// Each recorded 401 response is turned into an API error with Discogs' message.
  #[tokio::test]
  async fn fetch_identity_unauthorized() {
    let cases = [
      (
        include_str!("../../../resources/auth/invalid_token.json"),
        "Invalid consumer token. Please register an app before making requests.",
      ),
      (
        include_str!("../../../resources/auth/invalid_consumer.json"),
        "Invalid consumer key/secret. Please register an app before making requests.",
      ),
      (
        include_str!("../../../resources/auth/unauthenticated.json"),
        "You must authenticate to access this resource.",
      ),
    ];

    for (response, expected) in cases {
      let server = MockServer::start().await;
      mount_identity_api(&server, 401, response).await;

      match fetch_identity(&client(&server)).await {
        Err(DiscogsClientError::Api { status, message }) => {
          assert_eq!(status, StatusCode::UNAUTHORIZED);
          assert_eq!(message.as_deref(), Some(expected));
        }
        other => panic!("expected an API error, got {other:?}"),
      }
    }
  }
}

#[cfg(test)]
#[cfg(feature = "live-tests")]
mod live_tests {
  use reqwest::StatusCode;

  use crate::auth::test_helper::USER_AGENT;
  use crate::client::Client;
  use crate::error::DiscogsClientError;

  #[tokio::test]
  async fn identity_with_personal_token() {
    let token = std::env::var("DISCOGS_TOKEN").expect("DISCOGS_TOKEN is required by the live tests");
    let client = Client::builder(USER_AGENT).personal_token(token).build().unwrap();

    let identity = client.identity().await.unwrap();

    assert!(!identity.username.is_empty());
  }

  #[tokio::test]
  async fn identity_with_invalid_token() {
    let client = Client::builder(USER_AGENT).personal_token("invalid").build().unwrap();

    match client.identity().await {
      Err(DiscogsClientError::Api { status, message }) => {
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(message.is_some());
      }
      other => panic!("expected an API error, got {other:?}"),
    }
  }
}
