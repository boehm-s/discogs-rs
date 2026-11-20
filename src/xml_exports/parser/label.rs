use std::io::BufRead;

use crate::error::Result;
use crate::helpers::ids::LabelId;
use crate::xml_exports::model::label::ExportLabel;
use crate::xml_exports::parser::common::{from_text, reference};
use crate::xml_exports::parser::xml::{Element, Xml, invalid};

pub(crate) fn label<R: BufRead>(xml: &mut Xml<R>, element: Element) -> Result<ExportLabel> {
  let mut id = None;
  let mut label = ExportLabel {
    id: LabelId::new(0),
    name: String::new(),
    contact_info: None,
    profile: None,
    data_quality: None,
    urls: Vec::new(),
    parent_label: None,
    sub_labels: Vec::new(),
  };
  while let Some(child) = xml.next_child(element)? {
    match xml.name() {
      b"id" => id = xml.parsed(child)?,
      b"name" => label.name = xml.text(child)?,
      b"contactinfo" => label.contact_info = xml.optional_text(child)?,
      b"profile" => label.profile = xml.optional_text(child)?,
      b"data_quality" => label.data_quality = from_text(xml.text_ref(child)?),
      b"urls" => label.urls = xml.texts(child)?,
      b"parentLabel" => label.parent_label = reference(xml, child)?,
      b"sublabels" => label.sub_labels = xml.list(child, reference)?,
      _ => xml.skip(child)?,
    }
  }
  label.id = LabelId::new(id.ok_or_else(|| invalid(format!("label without id: {}", label.name)))?);
  Ok(label)
}
