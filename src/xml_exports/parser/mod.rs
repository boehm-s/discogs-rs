//! Pull parsers of the records of an export.

pub(crate) mod artist;
pub(crate) mod common;
pub(crate) mod input;
pub(crate) mod label;
pub(crate) mod master;
pub(crate) mod release;
pub(crate) mod xml;

use std::io::BufRead;
use std::marker::PhantomData;

use crate::error::{DiscogsClientError, Result};
use crate::xml_exports::error::ExportError;
use crate::xml_exports::model::ExportRecord;
use crate::xml_exports::model::artist::ExportArtist;
use crate::xml_exports::model::export::ExportKind;
use crate::xml_exports::model::label::ExportLabel;
use crate::xml_exports::model::master::ExportMaster;
use crate::xml_exports::model::release::ExportRelease;
use crate::xml_exports::parser::xml::{Element, Xml};

pub(crate) mod sealed {
  use std::io::BufRead;

  use crate::error::Result;
  use crate::xml_exports::parser::xml::{Element, Xml};

  pub trait Parse: Sized {
    /// Tag of the records (`<release>`…)
    const TAG: &'static [u8];

    #[expect(private_interfaces, reason = "sealed trait, only implemented in this crate")]
    fn parse<R: BufRead>(xml: &mut Xml<R>, element: Element) -> Result<Self>;
  }
}

macro_rules! export_record {
  ($record:ty, $kind:expr, $tag:literal, $parse:path) => {
    impl ExportRecord for $record {
      const KIND: ExportKind = $kind;
    }

    impl sealed::Parse for $record {
      const TAG: &'static [u8] = $tag;

      #[expect(private_interfaces, reason = "sealed trait, only implemented in this crate")]
      fn parse<R: BufRead>(xml: &mut Xml<R>, element: Element) -> Result<Self> {
        $parse(xml, element)
      }
    }
  };
}

export_record!(ExportArtist, ExportKind::Artists, b"artist", artist::artist);
export_record!(ExportLabel, ExportKind::Labels, b"label", label::label);
export_record!(ExportMaster, ExportKind::Masters, b"master", master::master);
export_record!(ExportRelease, ExportKind::Releases, b"release", release::release);

/// Iterator over the records of a decompressed export, parsed one by one.
/// [`ExportError::InvalidRecord`] only concerns its record and the iteration goes on; other errors
/// end it.
pub(crate) struct ExportReader<T, R> {
  xml: Xml<R>,
  root: Option<Element>,
  started: bool,
  done: bool,
  /// `T` is produced, not stored: `fn() -> T` keeps the reader `Send` and `Sync` whatever `T`
  record: PhantomData<fn() -> T>,
}

impl<T: ExportRecord, R: BufRead> ExportReader<T, R> {
  pub(crate) fn new(reader: R) -> Self {
    Self {
      xml: Xml::new(reader),
      root: None,
      started: false,
      done: false,
      record: PhantomData,
    }
  }

  fn next_record(&mut self) -> Result<Option<T>> {
    if !self.started {
      self.started = true;
      self.root = self.xml.root()?;
    }
    let Some(root) = self.root else {
      return Ok(None);
    };
    while let Some(element) = self.xml.next_child(root)? {
      if self.xml.name() == T::TAG {
        return T::parse(&mut self.xml, element).map(Some);
      }
      self.xml.skip(element)?;
    }
    Ok(None)
  }
}

impl<T: ExportRecord, R: BufRead> Iterator for ExportReader<T, R> {
  type Item = Result<T>;

  fn next(&mut self) -> Option<Self::Item> {
    if self.done {
      return None;
    }
    let record = self.next_record().transpose();
    if !matches!(
      record,
      Some(Ok(_) | Err(DiscogsClientError::Export(ExportError::InvalidRecord(_))))
    ) {
      self.done = true;
    }
    record
  }
}

#[cfg(test)]
pub(crate) mod tests {
  use std::io::Write as _;
  use std::time::Duration;

  use flate2::Compression;
  use flate2::write::GzEncoder;
  use jiff::civil::date;

  use super::ExportReader;
  use crate::error::{DiscogsClientError, Result};
  use crate::helpers::ids::{ArtistId, LabelId, MasterReleaseId, ReleaseId};
  use crate::releases::model::release_date::ReleaseDate;
  use crate::releases::model::status::DataQuality;
  use crate::xml_exports::error::ExportError;
  use crate::xml_exports::model::ExportRecord;
  use crate::xml_exports::model::artist::ExportArtist;
  use crate::xml_exports::model::common::Reference;
  use crate::xml_exports::model::label::ExportLabel;
  use crate::xml_exports::model::master::ExportMaster;
  use crate::xml_exports::model::release::ExportRelease;

  pub(crate) const ARTISTS: &str = include_str!("../../../resources/xml_exports/artists.xml");
  pub(crate) const LABELS: &str = include_str!("../../../resources/xml_exports/labels.xml");
  pub(crate) const MASTERS: &str = include_str!("../../../resources/xml_exports/masters.xml");
  pub(crate) const RELEASES: &str = include_str!("../../../resources/xml_exports/releases.xml");

  fn parse<T: ExportRecord>(xml: &str) -> Vec<T> {
    ExportReader::<T, _>::new(xml.as_bytes())
      .collect::<Result<_>>()
      .unwrap()
  }

  pub(crate) fn gzip(xml: &str) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(xml.as_bytes()).unwrap();
    encoder.finish().unwrap()
  }

  #[test]
  fn parse_artists() {
    let artists = parse::<ExportArtist>(ARTISTS);

    assert_eq!(artists.len(), 3);
    let persuader = &artists[0];
    assert_eq!(persuader.id, ArtistId::new(1));
    assert_eq!(persuader.real_name.as_deref(), Some("Jesper Dahlbäck"));
    assert_eq!(persuader.data_quality, Some(DataQuality::NeedsVote));
    assert_eq!(persuader.name_variations, ["Persuader", "The Presuader"]);
    assert_eq!(
      persuader.aliases[0],
      Reference {
        id: ArtistId::new(239),
        name: "Jesper Dahlbäck".to_owned()
      }
    );
    assert_eq!(artists[1].name, "Mr. James Barth & A.D.");
    assert_eq!(artists[1].members.len(), 2);
    assert!(
      artists[2]
        .profile
        .as_deref()
        .unwrap()
        .contains("New York.\r\nCo-founded")
    );
  }

  #[test]
  fn parse_labels() {
    let labels = parse::<ExportLabel>(LABELS);

    assert_eq!(labels.len(), 2);
    assert_eq!(labels[0].id, LabelId::new(1));
    assert_eq!(
      labels[0].contact_info.as_deref(),
      Some("Example Records\r\n1 Example Street\r\nEmail: contact@example.com")
    );
    assert_eq!(labels[0].sub_labels.len(), 8);
    assert_eq!(labels[0].parent_label, None);
    assert_eq!(
      labels[1].parent_label,
      Some(Reference {
        id: LabelId::new(4711),
        name: "Goldhead Music".to_owned()
      })
    );
  }

  #[test]
  fn parse_masters() {
    let masters = parse::<ExportMaster>(MASTERS);

    assert_eq!(masters.len(), 3);
    assert_eq!(masters[0].id, MasterReleaseId::new(114));
    assert_eq!(masters[0].videos[0].duration, Duration::from_secs(448));
    assert!(masters[0].videos[0].embed);
    assert_eq!(masters[1].artists[0].anv.as_deref(), Some("Vоплі Vідоплясова"));
    assert_eq!(masters[1].artists[0].join.as_deref(), Some(","));
    assert_eq!(masters[1].year, Some(1997));
    // Artist not in the database
    assert_eq!(masters[2].artists[0].id, None);
    assert_eq!(masters[2].artists[0].name, "Hecate");
  }

  #[test]
  fn parse_releases() {
    let releases = parse::<ExportRelease>(RELEASES);

    assert_eq!(releases.len(), 4);
    let release = releases
      .iter()
      .find(|release| release.id == ReleaseId::new(25))
      .unwrap();
    assert_eq!(release.title, "Digitalsoul (Session Two)");
    assert_eq!(release.labels[0].catno.as_deref(), Some("LAR-053"));
    assert_eq!(release.series[0].id, Some(LabelId::new(586_116)));
    assert_eq!(release.master_id, Some(MasterReleaseId::new(903_113)));
    assert!(release.is_main_release);
    assert_eq!(
      release.released,
      Some(ReleaseDate::Known(date(2000, 1, 1)..=date(2000, 12, 31)))
    );
    assert_eq!(release.formats[0].quantity, Some(1));
    assert_eq!(release.formats[0].text, None);
    assert_eq!(release.formats[0].descriptions[0], "12\"");
    assert_eq!(release.identifiers[0].kind, "Barcode");
    assert_eq!(release.identifiers[0].description, None);
    assert_eq!(release.companies[2].entity_type_name.as_deref(), Some("Lacquer Cut At"));
    assert!(release.notes.as_deref().unwrap().contains("℗&© 2000"));
    // Heading, then tracks
    assert_eq!(release.tracklist[0].position, None);
    assert_eq!(release.tracklist[1].duration, Some(Duration::from_secs(7 * 60 + 30)));
    assert_eq!(release.tracklist[5].extra_artists[0].role.as_deref(), Some("Words By"));

    let index = releases
      .iter()
      .find(|release| release.id == ReleaseId::new(723))
      .unwrap();
    let dreammaker = index
      .tracklist
      .iter()
      .find(|track| track.title == "Dreammaker")
      .unwrap();
    assert_eq!(dreammaker.sub_tracks.len(), 3);

    let undated = releases
      .iter()
      .find(|release| release.id == ReleaseId::new(590))
      .unwrap();
    assert_eq!(undated.released, None);
  }

  #[test]
  fn invalid_record_does_not_stop_the_reading() {
    let xml = "<artists><artist><name>No id</name></artist><artist><id>7</id><name>Ok</name></artist></artists>";
    let records: Vec<_> = ExportReader::<ExportArtist, _>::new(xml.as_bytes()).collect();

    assert!(matches!(
      records[0],
      Err(DiscogsClientError::Export(ExportError::InvalidRecord(_)))
    ));
    assert_eq!(records[1].as_ref().unwrap().id, ArtistId::new(7));
  }

  #[test]
  fn malformed_xml_stops_the_reading() {
    let xml = "<artists><artist><id>7</id><name>Ok</nom></artist><artist><id>8</id></artist></artists>";
    let records: Vec<_> = ExportReader::<ExportArtist, _>::new(xml.as_bytes()).collect();

    assert_eq!(records.len(), 1);
    assert!(matches!(
      records[0],
      Err(DiscogsClientError::Export(ExportError::Xml(_)))
    ));
  }

  #[test]
  fn empty_export() {
    assert_eq!(parse::<ExportArtist>("").len(), 0);
    assert_eq!(parse::<ExportArtist>("<artists/>").len(), 0);
  }
}
