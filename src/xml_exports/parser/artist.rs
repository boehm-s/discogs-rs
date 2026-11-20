use std::io::BufRead;

use crate::error::Result;
use crate::helpers::ids::ArtistId;
use crate::xml_exports::model::artist::ExportArtist;
use crate::xml_exports::parser::common::{from_text, reference};
use crate::xml_exports::parser::xml::{Element, Xml, invalid};

pub(crate) fn artist<R: BufRead>(xml: &mut Xml<R>, element: Element) -> Result<ExportArtist> {
  let mut id = None;
  let mut artist = ExportArtist {
    id: ArtistId::new(0),
    name: String::new(),
    real_name: None,
    profile: None,
    data_quality: None,
    urls: Vec::new(),
    name_variations: Vec::new(),
    aliases: Vec::new(),
    members: Vec::new(),
    groups: Vec::new(),
  };
  while let Some(child) = xml.next_child(element)? {
    match xml.name() {
      b"id" => id = xml.parsed(child)?,
      b"name" => artist.name = xml.text(child)?,
      b"realname" => artist.real_name = xml.optional_text(child)?,
      b"profile" => artist.profile = xml.optional_text(child)?,
      b"data_quality" => artist.data_quality = from_text(xml.text_ref(child)?),
      b"urls" => artist.urls = xml.texts(child)?,
      b"namevariations" => artist.name_variations = xml.texts(child)?,
      b"aliases" => artist.aliases = xml.list(child, reference)?,
      b"members" => artist.members = xml.list(child, reference)?,
      b"groups" => artist.groups = xml.list(child, reference)?,
      _ => xml.skip(child)?,
    }
  }
  artist.id = ArtistId::new(id.ok_or_else(|| invalid(format!("artist without id: {}", artist.name)))?);
  Ok(artist)
}
