use wiremock::matchers::{header, header_regex, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Response bodies of the OAuth endpoints, in the format documented by Discogs
pub(crate) const REQUEST_TOKEN_RESPONSE: &str =
  "oauth_token=request-token&oauth_token_secret=request-secret&oauth_callback_confirmed=true";
pub(crate) const ACCESS_TOKEN_RESPONSE: &str = "oauth_token=access-token&oauth_token_secret=access-secret";
/// Real Discogs answer to an unknown consumer key (plain text, not JSON)
pub(crate) const INVALID_CONSUMER_RESPONSE: &str = "Invalid consumer.";

fn oauth_response(status: u16, response: &str) -> ResponseTemplate {
  ResponseTemplate::new(status).set_body_raw(response, mime::APPLICATION_WWW_FORM_URLENCODED.as_ref())
}

/// `GET /oauth/request_token`, signed with the consumer `consumer-key` and the given callback.
pub(crate) async fn mount_request_token_api(server: &MockServer, callback: Option<&str>, status: u16, response: &str) {
  let callback = callback.map_or_else(String::new, |callback| {
    let callback: String = url::form_urlencoded::byte_serialize(callback.as_bytes()).collect();
    format!(r#"oauth_callback="{callback}","#)
  });
  Mock::given(method("GET"))
    .and(path("/oauth/request_token"))
    .and(header("Content-Type", mime::APPLICATION_WWW_FORM_URLENCODED.as_ref()))
    .and(header_regex(
      "Authorization",
      &format!(
        r#"^OAuth {callback}oauth_consumer_key="consumer-key",oauth_nonce="[^"]+",oauth_signature_method="HMAC-SHA1",oauth_timestamp="\d+",oauth_signature="[^"]+"$"#
      ),
    ))
    .respond_with(oauth_response(status, response))
    .expect(1)
    .mount(server)
    .await;
}

/// `POST /oauth/access_token`, signed with the consumer `consumer-key`, the request token and the verifier.
pub(crate) async fn mount_access_token_api(server: &MockServer, request_token: &str, verifier: &str, response: &str) {
  Mock::given(method("POST"))
    .and(path("/oauth/access_token"))
    .and(header("Content-Type", mime::APPLICATION_WWW_FORM_URLENCODED.as_ref()))
    .and(header_regex(
      "Authorization",
      &format!(
        r#"^OAuth oauth_consumer_key="consumer-key",oauth_nonce="[^"]+",oauth_signature_method="HMAC-SHA1",oauth_timestamp="\d+",oauth_token="{request_token}",oauth_verifier="{verifier}",oauth_signature="[^"]+"$"#
      ),
    ))
    .respond_with(oauth_response(200, response))
    .expect(1)
    .mount(server)
    .await;
}
