use std::io::BufRead;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::de::value::{Error as ValueError, StrDeserializer};

use quick_xml::XmlVersion;

use crate::error::Result;
use crate::helpers::ids::{ArtistId, TypeId};
use crate::releases::model::media::Video;
use crate::xml_exports::model::artist::ArtistCredit;
use crate::xml_exports::model::common::Reference;
use crate::xml_exports::parser::xml::{Element, Xml};

/// Enum of the API models (`DataQuality`…) from its text.
pub(crate) fn from_text<T: DeserializeOwned>(text: &str) -> Option<T> {
  T::deserialize(StrDeserializer::<ValueError>::new(text)).ok()
}

/// Value of the attribute of the last element read, `None` when missing or empty.
pub(crate) fn attribute<R: BufRead>(xml: &Xml<R>, name: &[u8]) -> Result<Option<String>> {
  for attribute in xml.attributes() {
    let attribute = attribute.map_err(quick_xml::Error::from)?;
    if attribute.key.as_ref().as_bytes() == name {
      let value = attribute.normalized_value(XmlVersion::Implicit1_0)?;
      return Ok((!value.is_empty()).then(|| value.into_owned()));
    }
  }
  Ok(None)
}

/// `<name id="239">Jesper Dahlbäck</name>`, `None` without id.
pub(crate) fn reference<R: BufRead, T>(xml: &mut Xml<R>, element: Element) -> Result<Option<Reference<TypeId<T>>>> {
  let id = attribute(xml, b"id")?.and_then(|id| id.parse().ok());
  let name = xml.text(element)?;
  Ok(id.map(|id| Reference {
    id: TypeId::new(id),
    name,
  }))
}

/// `<artist><id>…</id><name>…</name><anv/><join/><role/><tracks/></artist>`
pub(crate) fn artist_credit<R: BufRead>(xml: &mut Xml<R>, element: Element) -> Result<Option<ArtistCredit>> {
  let mut credit = ArtistCredit {
    id: None,
    name: String::new(),
    anv: None,
    join: None,
    role: None,
    tracks: None,
  };
  while let Some(child) = xml.next_child(element)? {
    match xml.name() {
      b"id" => credit.id = xml.parsed::<u64>(child)?.map(ArtistId::new),
      b"name" => credit.name = xml.text(child)?,
      b"anv" => credit.anv = xml.optional_text(child)?,
      b"join" => credit.join = xml.optional_text(child)?,
      b"role" => credit.role = xml.optional_text(child)?,
      b"tracks" => credit.tracks = xml.optional_text(child)?,
      _ => xml.skip(child)?,
    }
  }
  Ok(Some(credit))
}

/// `<video src="…" duration="321" embed="true"><title/><description/></video>`
pub(crate) fn video<R: BufRead>(xml: &mut Xml<R>, element: Element) -> Result<Option<Video>> {
  let Some(uri) = attribute(xml, b"src")? else {
    xml.skip(element)?;
    return Ok(None);
  };
  let duration = attribute(xml, b"duration")?.and_then(|duration| duration.parse().ok());
  let embed = attribute(xml, b"embed")?.as_deref() == Some("true");
  let mut video = Video {
    uri,
    title: String::new(),
    description: None,
    duration: Duration::from_secs(duration.unwrap_or_default()),
    embed,
  };
  while let Some(child) = xml.next_child(element)? {
    match xml.name() {
      b"title" => video.title = xml.text(child)?,
      b"description" => video.description = xml.optional_text(child)?,
      _ => xml.skip(child)?,
    }
  }
  Ok(Some(video))
}
