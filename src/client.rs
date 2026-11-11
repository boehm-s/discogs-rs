//! Client of the Discogs API, shared by all the domains (`auth`, …): each domain adds its endpoints
//! to [`Client`] in its own `client.rs`.

use std::sync::Arc;

use oauth1_request::ParameterList;
use reqwest::header::{AUTHORIZATION, HeaderValue, USER_AGENT};
use reqwest::{Method, RequestBuilder, Response};
use serde::de::DeserializeOwned;
use tracing::{Span, instrument};

use crate::API_BASE_URL;
use crate::auth::model::credentials::{AccessToken, ConsumerCredentials};
use crate::auth::service::authorization::{Authentication, OAuthCredentials};
use crate::error::{ApiErrorBody, DiscogsClientError, Result};

/// Created with [`Client::builder`]. Cloning is cheap: clones share the same connection pool.
#[derive(Clone, Debug)]
pub struct Client {
  inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
  http: reqwest::Client,
  user_agent: HeaderValue,
  /// Without trailing slash
  base_url: String,
  auth: Authentication,
}

impl Client {
  /// Discogs requires every application to send a unique `User-Agent`, ideally with a version
  /// and a URL, e.g. `MyCollectionApp/1.0 +https://example.com`.
  pub fn builder(user_agent: impl Into<String>) -> ClientBuilder {
    ClientBuilder {
      user_agent: user_agent.into(),
      auth: Authentication::None,
      base_url: API_BASE_URL.to_owned(),
      http: None,
    }
  }

  pub(crate) async fn get<T: DeserializeOwned>(&self, path: &str, query: &[(&str, &str)]) -> Result<T> {
    let auth = &self.inner.auth;
    let request = self.request(Method::GET, path, query, |method, url, params| {
      auth.authorization_header(method, url, params)
    });
    Ok(Self::send(request).await?.json().await?)
  }

  /// `authorization` receives the method, the URL without query and the query parameters (all
  /// needed by OAuth signatures) and returns the `Authorization` header, if any.
  pub(crate) fn request(
    &self,
    method: Method,
    path: &str,
    query: &[(&str, &str)],
    authorization: impl FnOnce(&str, &str, &ParameterList<&str, &str>) -> Option<String>,
  ) -> RequestBuilder {
    let url = format!("{}{path}", self.inner.base_url);
    let params = ParameterList::new(query.to_vec());
    let authorization = authorization(method.as_str(), &url, &params);
    // Same percent-encoding as the one used for the OAuth signature
    let url = oauth1_request::to_query(url, &params);

    let request = self
      .inner
      .http
      .request(method, url)
      .header(USER_AGENT, self.inner.user_agent.clone());
    match authorization {
      Some(authorization) => request.header(AUTHORIZATION, authorization),
      None => request,
    }
  }

  /// Sends a REST request, turning error statuses into [`DiscogsClientError::Api`].
  pub(crate) async fn send(request: RequestBuilder) -> Result<Response> {
    let response = Self::execute(request).await?;
    let status = response.status();
    if status.is_success() {
      return Ok(response);
    }
    // Error pages of proxies (e.g. 502) are not JSON: no message then
    let message = response.json::<ApiErrorBody>().await.ok().and_then(|body| body.message);
    Err(DiscogsClientError::Api { status, message })
  }

  /// Sends the request without interpreting the status.
  #[instrument(name = "Calling Discogs", skip_all, fields(method, url, status))]
  pub(crate) async fn execute(request: RequestBuilder) -> Result<Response> {
    let (http, request) = request.build_split();
    let request = request?;
    // Credentials are only in headers, so the URL can be recorded as is
    let span = Span::current();
    span.record("method", request.method().as_str());
    span.record("url", request.url().as_str());

    let response = http.execute(request).await?;
    span.record("status", response.status().as_u16());
    Ok(response)
  }
}

/// Without authentication method, only public endpoints can be called. When several
/// authentication methods are set, the last one wins.
#[derive(Debug)]
#[must_use]
pub struct ClientBuilder {
  user_agent: String,
  auth: Authentication,
  base_url: String,
  http: Option<reqwest::Client>,
}

impl ClientBuilder {
  /// Authenticates with a personal access token, from <https://www.discogs.com/settings/developers>.
  ///
  /// Requests act on behalf of the token owner.
  pub fn personal_token(mut self, token: impl Into<String>) -> Self {
    self.auth = Authentication::PersonalToken(token.into().into());
    self
  }

  /// Authenticates as an application only, without user: higher rate limit and image URLs,
  /// but no access to user data.
  pub fn consumer(mut self, consumer: ConsumerCredentials) -> Self {
    self.auth = Authentication::Consumer(consumer);
    self
  }

  /// Authenticates with OAuth, on behalf of the user who granted `token`
  /// (see [`OAuthFlow`](crate::auth::client::OAuthFlow)).
  pub fn oauth(mut self, consumer: ConsumerCredentials, token: AccessToken) -> Self {
    self.auth = Authentication::OAuth(OAuthCredentials { consumer, token });
    self
  }

  #[cfg(test)]
  pub(crate) fn base_url(mut self, base_url: impl Into<String>) -> Self {
    self.base_url = base_url.into();
    self
  }

  /// Uses a preconfigured `reqwest` client (timeouts, proxy…). The user agent is still set on
  /// each request.
  pub fn http_client(mut self, http: reqwest::Client) -> Self {
    self.http = Some(http);
    self
  }

  /// # Errors
  ///
  /// Fails if the user agent is empty or not a valid header value, or if the HTTP client cannot be
  /// initialized.
  pub fn build(self) -> Result<Client> {
    if self.user_agent.trim().is_empty() {
      return Err(DiscogsClientError::InvalidUserAgent);
    }
    let user_agent = HeaderValue::from_str(&self.user_agent).map_err(|_| DiscogsClientError::InvalidUserAgent)?;
    let http = match self.http {
      Some(http) => http,
      None => reqwest::Client::builder().build()?,
    };

    Ok(Client {
      inner: Arc::new(Inner {
        http,
        user_agent,
        base_url: self.base_url.trim_end_matches('/').to_owned(),
        auth: self.auth,
      }),
    })
  }
}

#[cfg(test)]
mod tests {
  use reqwest::StatusCode;
  use wiremock::matchers::{header, header_regex, method, path, query_param};
  use wiremock::{Mock, MockServer, ResponseTemplate};

  use super::Client;
  use crate::auth::model::credentials::{AccessToken, ConsumerCredentials};
  use crate::error::DiscogsClientError;

  const UA: &str = "discogs-client-tests/1.0 +https://github.com/boehm-s/discogs-rs";

  fn json(body: &str) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_raw(body, mime::APPLICATION_JSON.as_ref())
  }

  #[tokio::test]
  async fn query_parameters_are_sent_and_encoded() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
      .and(path("/database/search"))
      .and(query_param("q", "Nirvana & Co"))
      .and(query_param("type", "release"))
      .respond_with(json("{}"))
      .expect(1)
      .mount(&server)
      .await;

    let client = Client::builder(UA).base_url(server.uri()).build().unwrap();
    client
      .get::<serde_json::Value>("/database/search", &[("q", "Nirvana & Co"), ("type", "release")])
      .await
      .unwrap();
  }

  #[tokio::test]
  async fn personal_token_and_user_agent_headers() {
    let server = MockServer::start().await;
    Mock::given(header("Authorization", "Discogs token=my-token"))
      .and(header("User-Agent", UA))
      .respond_with(json("{}"))
      .expect(1)
      .mount(&server)
      .await;

    let client = Client::builder(UA)
      .personal_token("my-token")
      .base_url(server.uri())
      .build()
      .unwrap();
    client.get::<serde_json::Value>("/anything", &[]).await.unwrap();
  }

  #[tokio::test]
  async fn consumer_header() {
    let server = MockServer::start().await;
    Mock::given(header_regex(
      "Authorization",
      // Anchored regex: `header()` would split the value on the comma
      "^Discogs key=consumer-key, secret=consumer-secret$",
    ))
    .respond_with(json("{}"))
    .expect(1)
    .mount(&server)
    .await;

    let client = Client::builder(UA)
      .consumer(ConsumerCredentials::new("consumer-key", "consumer-secret"))
      .base_url(server.uri())
      .build()
      .unwrap();
    client.get::<serde_json::Value>("/anything", &[]).await.unwrap();
  }

  #[tokio::test]
  async fn oauth_header() {
    let server = MockServer::start().await;
    Mock::given(header_regex(
      "Authorization",
      r#"^OAuth oauth_consumer_key="consumer-key",oauth_nonce="[^"]+",oauth_signature_method="HMAC-SHA1",oauth_timestamp="\d+",oauth_token="access-token",oauth_signature="[^"]+"$"#,
    ))
    .respond_with(json("{}"))
    .expect(1)
    .mount(&server)
    .await;

    let client = Client::builder(UA)
      .oauth(
        ConsumerCredentials::new("consumer-key", "consumer-secret"),
        AccessToken::new("access-token", "token-secret"),
      )
      .base_url(server.uri())
      .build()
      .unwrap();
    client.get::<serde_json::Value>("/anything", &[]).await.unwrap();
  }

  #[tokio::test]
  async fn no_authorization_header_without_auth() {
    let server = MockServer::start().await;
    Mock::given(method("GET")).respond_with(json("{}")).mount(&server).await;

    let client = Client::builder(UA).base_url(server.uri()).build().unwrap();
    client.get::<serde_json::Value>("/anything", &[]).await.unwrap();

    let requests = server.received_requests().await.unwrap();
    assert!(
      requests
        .iter()
        .all(|request| !request.headers.contains_key("authorization"))
    );
  }

  #[tokio::test]
  async fn error_status_becomes_api_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
      .respond_with(ResponseTemplate::new(401).set_body_raw(
        include_str!("../resources/auth/unauthenticated.json"),
        mime::APPLICATION_JSON.as_ref(),
      ))
      .mount(&server)
      .await;

    let client = Client::builder(UA).base_url(server.uri()).build().unwrap();
    match client.get::<serde_json::Value>("/oauth/identity", &[]).await {
      Err(DiscogsClientError::Api { status, message }) => {
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
          message.as_deref(),
          Some("You must authenticate to access this resource.")
        );
      }
      other => panic!("expected an API error, got {other:?}"),
    }
  }

  #[tokio::test]
  async fn error_without_json_body_has_no_message() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
      .respond_with(ResponseTemplate::new(502).set_body_raw("<html>Bad Gateway</html>", "text/html"))
      .mount(&server)
      .await;
    let client = Client::builder(UA).base_url(server.uri()).build().unwrap();

    match client.get::<serde_json::Value>("/oauth/identity", &[]).await {
      Err(DiscogsClientError::Api { status, message }) => {
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(message, None);
      }
      other => panic!("expected an API error, got {other:?}"),
    }
  }

  #[test]
  fn invalid_user_agent_is_rejected() {
    for user_agent in ["", "  ", "bad\nagent"] {
      assert!(
        matches!(
          Client::builder(user_agent).build(),
          Err(DiscogsClientError::InvalidUserAgent)
        ),
        "{user_agent:?}"
      );
    }
  }
}
