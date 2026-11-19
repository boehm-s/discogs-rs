use std::io::BufRead;

use crate::error::Result;
use crate::helpers::ids::{MasterReleaseId, ReleaseId};
use crate::xml_exports::model::master::ExportMaster;
use crate::xml_exports::parser::common::{artist_credit, attribute, from_text, video};
use crate::xml_exports::parser::xml::{Element, Xml, invalid};

pub(crate) fn master<R: BufRead>(xml: &mut Xml<R>, element: Element) -> Result<ExportMaster> {
  let id = attribute(xml, b"id")?.and_then(|id| id.parse().ok());
  let mut master = ExportMaster {
    id: MasterReleaseId::new(0),
    main_release: None,
    title: String::new(),
    year: None,
    artists: Vec::new(),
    genres: Vec::new(),
    styles: Vec::new(),
    notes: None,
    data_quality: None,
    videos: Vec::new(),
  };
  while let Some(child) = xml.next_child(element)? {
    match xml.name() {
      b"main_release" => master.main_release = xml.parsed::<u64>(child)?.map(ReleaseId::new),
      b"title" => master.title = xml.text(child)?,
      b"year" => master.year = xml.parsed::<u32>(child)?.filter(|year| *year != 0),
      b"artists" => master.artists = xml.list(child, artist_credit)?,
      b"genres" => master.genres = xml.texts(child)?,
      b"styles" => master.styles = xml.texts(child)?,
      b"notes" => master.notes = xml.optional_text(child)?,
      b"data_quality" => master.data_quality = from_text(xml.text_ref(child)?),
      b"videos" => master.videos = xml.list(child, video)?,
      _ => xml.skip(child)?,
    }
  }
  master.id = MasterReleaseId::new(id.ok_or_else(|| invalid(format!("master without id: {}", master.title)))?);
  Ok(master)
}
