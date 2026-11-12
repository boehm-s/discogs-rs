//! Link of a credited entity (artist, label…) to its Discogs page.

use serde::{Deserialize, Serialize};

/// Entities credited on draft releases are not published yet: they have no id nor URL.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Publication<Id> {
  Published { id: Id, resource_url: String },
  Draft {},
}
