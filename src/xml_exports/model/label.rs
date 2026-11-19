use serde::{Deserialize, Serialize};

use crate::helpers::ids::LabelId;
use crate::releases::model::status::DataQuality;
use crate::xml_exports::model::common::Reference;

/// Label of the `labels` export.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ExportLabel {
  pub id: LabelId,
  pub name: String,
  pub contact_info: Option<String>,
  pub profile: Option<String>,
  pub data_quality: Option<DataQuality>,
  pub urls: Vec<String>,
  pub parent_label: Option<Reference<LabelId>>,
  pub sub_labels: Vec<Reference<LabelId>>,
}
