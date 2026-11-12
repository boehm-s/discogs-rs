use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_with::{DurationSeconds, NoneAsEmptyString, serde_as};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Image {
  #[serde(rename = "type")]
  pub kind: ImageKind,
  pub uri: String,
  pub resource_url: String,
  pub uri150: String,
  pub width: u32,
  pub height: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageKind {
  Primary,
  Secondary,
  #[serde(untagged)]
  Unknown(String),
}

#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Video {
  pub uri: String,
  pub title: String,
  #[serde_as(deserialize_as = "NoneAsEmptyString")]
  #[serde(default)]
  pub description: Option<String>,
  #[serde_as(as = "DurationSeconds<u64>")]
  pub duration: Duration,
  pub embed: bool,
}
