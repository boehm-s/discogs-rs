use serde::{Deserialize, Serialize};

/// Moderation status of a release.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReleaseStatus {
  Accepted,
  Draft,
  Deleted,
  Rejected,
  #[serde(untagged)]
  Unknown(String),
}

/// Quality of the data, as voted by the community.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataQuality {
  #[serde(rename = "Complete and Correct")]
  CompleteAndCorrect,
  Correct,
  #[serde(rename = "Needs Vote")]
  NeedsVote,
  #[serde(rename = "Needs Minor Changes")]
  NeedsMinorChanges,
  #[serde(rename = "Needs Major Changes")]
  NeedsMajorChanges,
  #[serde(rename = "Entirely Incorrect")]
  EntirelyIncorrect,
  #[serde(untagged)]
  Unknown(String),
}
