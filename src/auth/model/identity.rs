//! Identity of the authenticated user.

use serde::{Deserialize, Serialize};

use crate::helpers::ids::UserId;

/// The user the client acts on behalf of, returned by
/// [`Client::identity`](crate::client::Client::identity).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
// Discogs may add fields to its responses: `non_exhaustive` forbids users to build or exhaustively
// destructure this struct, so adding a field is not a breaking change.
#[non_exhaustive]
pub struct Identity {
  pub id: UserId,
  pub username: String,
  pub resource_url: String,
  pub consumer_name: String,
}

#[cfg(test)]
mod tests {
  use super::Identity;
  use crate::helpers::ids::UserId;

  #[test]
  fn deserialize_identity_response() {
    let identity: Identity = serde_json::from_str(include_str!("../../../resources/auth/identity.json")).unwrap();
    assert_eq!(
      identity,
      Identity {
        id: UserId::new(1_234_567),
        username: "example_user".to_owned(),
        resource_url: "https://api.discogs.com/users/example_user".to_owned(),
        consumer_name: "Example App".to_owned(),
      }
    );
  }
}
