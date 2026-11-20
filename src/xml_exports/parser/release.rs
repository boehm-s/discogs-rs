use std::io::BufRead;

use crate::error::Result;
use crate::helpers::deserializers::track_duration;
use crate::helpers::ids::{LabelId, MasterReleaseId, ReleaseId};
use crate::releases::model::release::{Format, Identifier};
use crate::releases::model::release_date::ReleaseDate;
use crate::xml_exports::model::release::{CatalogEntry, Company, ExportRelease, Track};
use crate::xml_exports::parser::common::{artist_credit, attribute, from_text, video};
use crate::xml_exports::parser::xml::{Element, Xml, invalid};

pub(crate) fn release<R: BufRead>(xml: &mut Xml<R>, element: Element) -> Result<ExportRelease> {
  let id = attribute(xml, b"id")?.and_then(|id| id.parse().ok());
  let status = attribute(xml, b"status")?.and_then(|status| from_text(&status));
  let mut release = ExportRelease {
    id: ReleaseId::new(0),
    status,
    title: String::new(),
    artists: Vec::new(),
    extra_artists: Vec::new(),
    labels: Vec::new(),
    series: Vec::new(),
    companies: Vec::new(),
    formats: Vec::new(),
    genres: Vec::new(),
    styles: Vec::new(),
    country: None,
    released: None,
    notes: None,
    data_quality: None,
    master_id: None,
    is_main_release: false,
    tracklist: Vec::new(),
    identifiers: Vec::new(),
    videos: Vec::new(),
  };
  while let Some(child) = xml.next_child(element)? {
    match xml.name() {
      b"title" => release.title = xml.text(child)?,
      b"artists" => release.artists = xml.list(child, artist_credit)?,
      b"extraartists" => release.extra_artists = xml.list(child, artist_credit)?,
      b"labels" => release.labels = xml.list(child, catalog_entry)?,
      b"series" => release.series = xml.list(child, catalog_entry)?,
      b"companies" => release.companies = xml.list(child, company)?,
      b"formats" => release.formats = xml.list(child, format)?,
      b"genres" => release.genres = xml.texts(child)?,
      b"styles" => release.styles = xml.texts(child)?,
      b"country" => release.country = xml.optional_text(child)?,
      b"released" => release.released = ReleaseDate::from_discogs(xml.text_ref(child)?.trim()),
      b"notes" => release.notes = xml.optional_text(child)?,
      b"data_quality" => release.data_quality = from_text(xml.text_ref(child)?),
      b"master_id" => {
        release.is_main_release = attribute(xml, b"is_main_release")?.as_deref() == Some("true");
        release.master_id = xml.parsed::<u64>(child)?.map(MasterReleaseId::new);
      }
      b"tracklist" => release.tracklist = xml.list(child, track)?,
      b"identifiers" => release.identifiers = xml.list(child, identifier)?,
      b"videos" => release.videos = xml.list(child, video)?,
      _ => xml.skip(child)?,
    }
  }
  release.id = ReleaseId::new(id.ok_or_else(|| invalid(format!("release without id: {}", release.title)))?);
  Ok(release)
}

/// `<label name="Svek" catno="SK032" id="5"/>`, same for `<series>`
fn catalog_entry<R: BufRead>(xml: &mut Xml<R>, element: Element) -> Result<Option<CatalogEntry>> {
  let entry = CatalogEntry {
    id: attribute(xml, b"id")?.and_then(|id| id.parse().ok()).map(LabelId::new),
    name: attribute(xml, b"name")?.unwrap_or_default(),
    catno: attribute(xml, b"catno")?,
  };
  xml.skip(element)?;
  Ok(Some(entry))
}

fn company<R: BufRead>(xml: &mut Xml<R>, element: Element) -> Result<Option<Company>> {
  let mut company = Company {
    id: None,
    name: String::new(),
    catno: None,
    entity_type: None,
    entity_type_name: None,
  };
  while let Some(child) = xml.next_child(element)? {
    match xml.name() {
      b"id" => company.id = xml.parsed::<u64>(child)?.map(LabelId::new),
      b"name" => company.name = xml.text(child)?,
      b"catno" => company.catno = xml.optional_text(child)?,
      b"entity_type" => company.entity_type = xml.parsed(child)?,
      b"entity_type_name" => company.entity_type_name = xml.optional_text(child)?,
      _ => xml.skip(child)?,
    }
  }
  Ok(Some(company))
}

/// `<format name="Vinyl" qty="2" text=""><descriptions><description>LP</description>…`
fn format<R: BufRead>(xml: &mut Xml<R>, element: Element) -> Result<Option<Format>> {
  let mut format = Format {
    name: attribute(xml, b"name")?.unwrap_or_default(),
    quantity: attribute(xml, b"qty")?.and_then(|quantity| quantity.parse().ok()),
    descriptions: Vec::new(),
    text: attribute(xml, b"text")?,
  };
  while let Some(child) = xml.next_child(element)? {
    match xml.name() {
      b"descriptions" => format.descriptions = xml.texts(child)?,
      _ => xml.skip(child)?,
    }
  }
  Ok(Some(format))
}

/// `<identifier type="Barcode" description="Text" value="…"/>`
fn identifier<R: BufRead>(xml: &mut Xml<R>, element: Element) -> Result<Option<Identifier>> {
  let identifier = Identifier {
    kind: attribute(xml, b"type")?.unwrap_or_default(),
    value: attribute(xml, b"value")?.unwrap_or_default(),
    description: attribute(xml, b"description")?,
  };
  xml.skip(element)?;
  Ok(Some(identifier))
}

fn track<R: BufRead>(xml: &mut Xml<R>, element: Element) -> Result<Option<Track>> {
  let mut parsed = Track {
    position: None,
    title: String::new(),
    duration: None,
    artists: Vec::new(),
    extra_artists: Vec::new(),
    sub_tracks: Vec::new(),
  };
  while let Some(child) = xml.next_child(element)? {
    match xml.name() {
      b"position" => parsed.position = xml.optional_text(child)?,
      b"title" => parsed.title = xml.text(child)?,
      b"duration" => parsed.duration = track_duration::parse(xml.text_ref(child)?),
      b"artists" => parsed.artists = xml.list(child, artist_credit)?,
      b"extraartists" => parsed.extra_artists = xml.list(child, artist_credit)?,
      b"sub_tracks" => parsed.sub_tracks = xml.list(child, track)?,
      _ => xml.skip(child)?,
    }
  }
  Ok(Some(parsed))
}
