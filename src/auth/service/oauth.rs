use oauth1_request::Credentials;
use reqwest::header::CONTENT_TYPE;
use reqwest::{Method, RequestBuilder};
use serde::Deserialize;
use tracing::instrument;

use crate::auth::model::credentials::{AccessToken, ConsumerCredentials, RequestToken};
use crate::auth::service::authorization::oauth_builder;
use crate::client::Client;
use crate::error::{DiscogsClientError, Result};

/// `GET /oauth/request_token`, signed with the consumer credentials only.
#[instrument(skip(client, consumer))]
pub(crate) async fn fetch_request_token(
  client: &Client,
  consumer: &ConsumerCredentials,
  callback: Option<&str>,
) -> Result<RequestToken> {
  let request = client.request(Method::GET, "/oauth/request_token", &[], |method, url, params| {
    let mut builder = oauth_builder(consumer);
    builder.callback(callback);
    Some(builder.authorize(method, url, params))
  });
  let (token, secret) = send_token_request(request).await?;
  Ok(RequestToken::new(token, secret))
}

/// `POST /oauth/access_token`, signed with the authorized request token and the verifier.
#[instrument(skip_all)]
pub(crate) async fn fetch_access_token(
  client: &Client,
  consumer: &ConsumerCredentials,
  request_token: &RequestToken,
  verifier: &str,
) -> Result<AccessToken> {
  let request = client.request(Method::POST, "/oauth/access_token", &[], |method, url, params| {
    let mut builder = oauth_builder(consumer);
    builder
      .token(Credentials::new(request_token.token(), request_token.secret()))
      .verifier(verifier);
    Some(builder.authorize(method, url, params))
  });
  let (token, secret) = send_token_request(request).await?;
  Ok(AccessToken::new(token, secret))
}

/// Sends a token request and parses the `x-www-form-urlencoded` response.
async fn send_token_request(request: RequestBuilder) -> Result<(String, String)> {
  #[derive(Deserialize)]
  struct TokenResponse {
    oauth_token: String,
    oauth_token_secret: String,
  }

  // Discogs documents this content type for the token endpoints
  let request = request.header(CONTENT_TYPE, mime::APPLICATION_WWW_FORM_URLENCODED.as_ref());
  let response = Client::execute(request).await?;
  let status = response.status();
  let body = response.text().await?;
  // Unlike the REST API, OAuth endpoints answer errors in plain text
  if !status.is_success() {
    return Err(DiscogsClientError::Api {
      status,
      message: Some(body.trim().to_owned()),
    });
  }
  // The raw body is kept out of the error: it may contain `oauth_token_secret`
  let tokens =
    serde_urlencoded::from_str::<TokenResponse>(&body).map_err(|error| DiscogsClientError::OAuth(error.to_string()))?;
  Ok((tokens.oauth_token, tokens.oauth_token_secret))
}

#[cfg(test)]
mod tests {
  use reqwest::StatusCode;
  use wiremock::MockServer;

  use super::{fetch_access_token, fetch_request_token};
  use crate::auth::model::credentials::ConsumerCredentials;
  use crate::auth::test_helper::client;
  use crate::auth::test_helper::oauth_api::{
    ACCESS_TOKEN_RESPONSE, INVALID_CONSUMER_RESPONSE, REQUEST_TOKEN_RESPONSE, mount_access_token_api,
    mount_request_token_api,
  };
  use crate::error::DiscogsClientError;

  fn consumer() -> ConsumerCredentials {
    ConsumerCredentials::new("consumer-key", "consumer-secret")
  }

  #[tokio::test]
  async fn full_flow() {
    let server = MockServer::start().await;
    let callback = "https://example.com/callback";
    mount_request_token_api(&server, Some(callback), 200, REQUEST_TOKEN_RESPONSE).await;
    mount_access_token_api(&server, "request-token", "verifier-code", ACCESS_TOKEN_RESPONSE).await;
    let client = client(&server);

    let request_token = fetch_request_token(&client, &consumer(), Some(callback)).await.unwrap();
    assert_eq!(request_token.token(), "request-token");

    let access_token = fetch_access_token(&client, &consumer(), &request_token, "verifier-code")
      .await
      .unwrap();
    assert_eq!(
      (access_token.token(), access_token.secret()),
      ("access-token", "access-secret")
    );
  }

  #[tokio::test]
  async fn request_token_without_callback() {
    let server = MockServer::start().await;
    mount_request_token_api(&server, None, 200, REQUEST_TOKEN_RESPONSE).await;

    fetch_request_token(&client(&server), &consumer(), None).await.unwrap();
  }

  #[tokio::test]
  async fn invalid_consumer() {
    let server = MockServer::start().await;
    mount_request_token_api(&server, None, 401, INVALID_CONSUMER_RESPONSE).await;

    match fetch_request_token(&client(&server), &consumer(), None).await {
      Err(DiscogsClientError::Api { status, message }) => {
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(message.as_deref(), Some("Invalid consumer."));
      }
      other => panic!("expected an API error, got {other:?}"),
    }
  }

  #[tokio::test]
  async fn response_without_tokens() {
    let server = MockServer::start().await;
    mount_request_token_api(&server, None, 200, "oauth_token=only-the-token").await;

    assert!(matches!(
      fetch_request_token(&client(&server), &consumer(), None).await,
      Err(DiscogsClientError::OAuth(_))
    ));
  }
}
