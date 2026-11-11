//! How requests are authenticated: `Authorization` header of each method and OAuth signature.

use oauth1_request::{Builder, Credentials, HmacSha1, ParameterList};
use secrecy::{ExposeSecret as _, SecretString};

use crate::auth::model::credentials::{AccessToken, ConsumerCredentials};

#[derive(Clone, Debug)]
pub(crate) enum Authentication {
  None,
  PersonalToken(SecretString),
  Consumer(ConsumerCredentials),
  OAuth(OAuthCredentials),
}

#[derive(Clone, Debug)]
pub(crate) struct OAuthCredentials {
  pub(crate) consumer: ConsumerCredentials,
  pub(crate) token: AccessToken,
}

impl OAuthCredentials {
  fn authorization_header(&self, method: &str, url: &str, query: &ParameterList<&str, &str>) -> String {
    let mut builder = oauth_builder(&self.consumer);
    builder.token(Credentials::new(self.token.token(), self.token.secret()));
    builder.authorize(method, url, query)
  }
}

impl Authentication {
  /// `Authorization` header for a request to `url` (without query) with the given query parameters.
  pub(crate) fn authorization_header(
    &self,
    method: &str,
    url: &str,
    query: &ParameterList<&str, &str>,
  ) -> Option<String> {
    match self {
      Self::None => None,
      Self::PersonalToken(token) => Some(format!("Discogs token={}", token.expose_secret())),
      Self::Consumer(consumer) => Some(format!("Discogs key={}, secret={}", consumer.key(), consumer.secret())),
      Self::OAuth(credentials) => Some(credentials.authorization_header(method, url, query)),
    }
  }
}

/// OAuth 1.0a signer (HMAC-SHA1) of the application `consumer`. Callers add the token, callback or
/// verifier, then call `authorize` with the URL *without* query: query parameters are passed
/// separately so that they are part of the signature.
pub(crate) fn oauth_builder(consumer: &ConsumerCredentials) -> Builder<'_, HmacSha1, &str> {
  Builder::new(Credentials::new(consumer.key(), consumer.secret()), HmacSha1::new())
}

#[cfg(test)]
mod tests {
  use std::num::NonZeroU64;

  use oauth1_request::{Credentials, ParameterList};

  use super::{Authentication, oauth_builder};
  use crate::auth::model::credentials::ConsumerCredentials;

  const URL: &str = "https://api.discogs.com/database/search";

  fn consumer() -> ConsumerCredentials {
    ConsumerCredentials::new("consumer-key", "consumer-secret")
  }

  #[test]
  fn personal_token_header() {
    let auth = Authentication::PersonalToken("my-token".to_owned().into());
    assert_eq!(
      auth
        .authorization_header("GET", URL, &ParameterList::default())
        .as_deref(),
      Some("Discogs token=my-token")
    );
  }

  #[test]
  fn consumer_header() {
    let auth = Authentication::Consumer(consumer());
    assert_eq!(
      auth
        .authorization_header("GET", URL, &ParameterList::default())
        .as_deref(),
      Some("Discogs key=consumer-key, secret=consumer-secret")
    );
  }

  #[test]
  fn no_header_without_auth() {
    assert_eq!(
      Authentication::None.authorization_header("GET", URL, &ParameterList::default()),
      None
    );
  }

  /// Expected signature computed independently from the OAuth 1.0a spec (RFC 5849 §3.4):
  /// base string = `GET&<encoded url>&<encoded sorted params>`, key = `consumer-secret&token-secret`.
  #[test]
  fn oauth_signature_includes_query_parameters() {
    let consumer = consumer();
    let mut builder = oauth_builder(&consumer);
    builder
      .token(Credentials::new("access-token", "token-secret"))
      .nonce("fixednonce")
      .timestamp(NonZeroU64::new(1_700_000_000));
    let query = ParameterList::new(vec![("type", "release"), ("q", "Nirvana Nevermind")]);

    assert_eq!(
      builder.authorize("GET", URL, &query),
      "OAuth oauth_consumer_key=\"consumer-key\",oauth_nonce=\"fixednonce\",\
       oauth_signature_method=\"HMAC-SHA1\",oauth_timestamp=\"1700000000\",oauth_token=\"access-token\",\
       oauth_signature=\"LTxA1iKFhPDDcRhqOcY6aQ%2BrXVE%3D\""
    );
  }
}
