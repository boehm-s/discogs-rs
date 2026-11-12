//! Authentication endpoints added to [`Client`], and the OAuth flow.

use tracing::instrument;

use crate::auth::model::credentials::{AccessToken, ConsumerCredentials, RequestToken};
use crate::auth::model::identity::Identity;
use crate::auth::service::identity::fetch_identity;
use crate::auth::service::oauth::{fetch_access_token, fetch_request_token};
use crate::client::Client;
use crate::error::Result;

impl Client {
  /// Returns the user the client is authenticated as (`GET /oauth/identity`).
  ///
  /// Works with a personal access token or OAuth, not with app-only credentials.
  ///
  /// # Errors
  ///
  /// [`DiscogsClientError::Api`](crate::error::DiscogsClientError::Api) with status 401 if the client is not authenticated as a user.
  #[instrument(skip_all)]
  pub async fn identity(&self) -> Result<Identity> {
    fetch_identity(self).await
  }
}

/// Runs the OAuth flow to get an [`AccessToken`]:
///
/// 1. [`request_token`](Self::request_token), then send the user to
///    [`RequestToken::authorize_url`];
/// 2. once the user authorized the application, Discogs redirects to the callback URL with an
///    `oauth_verifier` parameter (or displays a code to copy when there is no callback);
/// 3. [`access_token`](Self::access_token) with that verifier.
#[derive(Clone, Debug)]
pub struct OAuthFlow {
  client: Client,
  consumer: ConsumerCredentials,
}

impl OAuthFlow {
  /// # Errors
  ///
  /// Same as [`ClientBuilder::build`](crate::client::ClientBuilder::build).
  pub fn new(user_agent: impl Into<String>, consumer: ConsumerCredentials) -> Result<Self> {
    Ok(Self::with_client(Client::builder(user_agent).build()?, consumer))
  }

  /// Creates the flow on top of an existing client (custom HTTP client, base URL…).
  /// The client's own authentication is not used.
  pub fn with_client(client: Client, consumer: ConsumerCredentials) -> Self {
    Self { client, consumer }
  }

  /// Gets a request token (step 1).
  ///
  /// `callback` is the URL Discogs redirects the user to after authorization. Without callback,
  /// Discogs displays a verification code the user has to copy into the application.
  ///
  /// # Errors
  ///
  /// [`DiscogsClientError::Api`](crate::error::DiscogsClientError::Api) if Discogs rejects the consumer credentials,
  /// [`DiscogsClientError::OAuth`](crate::error::DiscogsClientError::OAuth) if the response does not contain a token.
  #[instrument(skip_all)]
  pub async fn request_token(&self, callback: Option<&str>) -> Result<RequestToken> {
    fetch_request_token(&self.client, &self.consumer, callback).await
  }

  /// Exchanges the authorized request token and the verifier for an access token (step 3).
  ///
  /// # Errors
  ///
  /// [`DiscogsClientError::Api`](crate::error::DiscogsClientError::Api) if the verifier or the request token is rejected,
  /// [`DiscogsClientError::OAuth`](crate::error::DiscogsClientError::OAuth) if the response does not contain a token.
  #[instrument(skip_all)]
  pub async fn access_token(&self, request_token: &RequestToken, verifier: &str) -> Result<AccessToken> {
    fetch_access_token(&self.client, &self.consumer, request_token, verifier).await
  }
}
