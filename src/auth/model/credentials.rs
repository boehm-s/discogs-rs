//! Credentials of the different authentication methods.

use secrecy::{ExposeSecret as _, SecretString};
use serde::{Deserialize, Serialize, Serializer};

const AUTHORIZE_URL: &str = "https://www.discogs.com/oauth/authorize";

/// Credentials of a Discogs application, from <https://www.discogs.com/settings/developers>.
///
/// Used alone for app-only requests ([`ClientBuilder::consumer`](crate::client::ClientBuilder::consumer)),
/// or with an [`AccessToken`] for OAuth.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsumerCredentials {
  key: String,
  #[serde(serialize_with = "serialize_secret")]
  secret: SecretString,
}

impl ConsumerCredentials {
  pub fn new(key: impl Into<String>, secret: impl Into<String>) -> Self {
    Self {
      key: key.into(),
      secret: secret.into().into(),
    }
  }

  pub fn key(&self) -> &str {
    &self.key
  }
  pub fn secret(&self) -> &str {
    self.secret.expose_secret()
  }
}

/// OAuth access token of a user, obtained with [`OAuthFlow`](crate::auth::client::OAuthFlow).
///
/// It does not expire: store it (it implements `Serialize`/`Deserialize`) to skip the
/// authorization flow next time. Its `Debug` output hides the secret.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccessToken {
  token: String,
  #[serde(serialize_with = "serialize_secret")]
  secret: SecretString,
}

impl AccessToken {
  pub fn new(token: impl Into<String>, secret: impl Into<String>) -> Self {
    Self {
      token: token.into(),
      secret: secret.into().into(),
    }
  }

  pub fn token(&self) -> &str {
    &self.token
  }

  pub fn secret(&self) -> &str {
    self.secret.expose_secret()
  }
}

/// Temporary token returned by [`OAuthFlow::request_token`](crate::auth::client::OAuthFlow::request_token).
///
/// Web applications can store it (`Serialize`/`Deserialize`) between the redirection to Discogs
/// and the callback. Its `Debug` output hides the secret.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestToken {
  token: String,
  #[serde(serialize_with = "serialize_secret")]
  secret: SecretString,
}

impl RequestToken {
  pub(crate) fn new(token: String, secret: String) -> Self {
    Self {
      token,
      secret: secret.into(),
    }
  }

  pub fn token(&self) -> &str {
    &self.token
  }

  pub(crate) fn secret(&self) -> &str {
    self.secret.expose_secret()
  }

  /// URL where the user authorizes the application (step 2).
  pub fn authorize_url(&self) -> String {
    let token: String = url::form_urlencoded::byte_serialize(self.token.as_bytes()).collect();
    format!("{AUTHORIZE_URL}?oauth_token={token}")
  }
}

/// `secrecy` does not serialize strings on purpose: tokens are meant to be stored, so expose them here.
fn serialize_secret<S: Serializer>(secret: &SecretString, serializer: S) -> Result<S::Ok, S::Error> {
  serializer.serialize_str(secret.expose_secret())
}

#[cfg(test)]
mod tests {
  use super::{AccessToken, RequestToken};

  #[test]
  fn access_token_roundtrips_through_serde() {
    let token = AccessToken::new("access-token", "token-secret");
    let json = serde_json::to_string(&token).unwrap();
    assert_eq!(json, r#"{"token":"access-token","secret":"token-secret"}"#);
    let deserialized = serde_json::from_str::<AccessToken>(&json).unwrap();
    assert_eq!(
      (deserialized.token(), deserialized.secret()),
      ("access-token", "token-secret")
    );
  }

  #[test]
  fn authorize_url() {
    let request_token = RequestToken::new("request-token".to_owned(), "request-secret".to_owned());
    assert_eq!(
      request_token.authorize_url(),
      "https://www.discogs.com/oauth/authorize?oauth_token=request-token"
    );
  }
}
