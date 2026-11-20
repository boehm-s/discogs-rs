//! Options of [`ExportFile::for_each_parallel`](crate::xml_exports::client::ExportFile::for_each_parallel).
//!
//! The export is read once, as a stream: one thread reads and decompresses it, another cuts it
//! into blocks of complete records with a lexical scan (`cutter`), and workers parse the blocks
//! (`engine`). These two threads bound the throughput: with 24 CPUs, the releases export
//! (19.6 M records) is parsed in 84 s gzipped (11 GB) and 77 s decompressed (63 GB), against
//! 4 min 16 s sequentially.
//!
//! The function is called from the workers, with records in file order within a block but
//! unordered across blocks. Convert each record there (Arrow rows, serialized message…): sending
//! the records themselves to another thread would free them there, which the glibc allocator
//! handles badly.

mod cutter;
mod cutting_scanner;
pub(crate) mod engine;

use std::thread;

use crate::error::Result;
use crate::xml_exports::error::ExportError;

/// ```
/// use discogs_client::xml_exports::parallel::ParallelOptions;
///
/// let options = ParallelOptions::default().with_max_workers(8);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParallelOptions {
  /// Maximum parsing threads, besides the reading and cutting threads. Workers are started as
  /// needed: the run starts with one, and adds one each time they fall behind. Default: available
  /// parallelism minus 2, at least 1. The available parallelism can be lower than the number of
  /// CPUs (affinity, container quotas). When `f` mostly waits (synchronous network calls…), more
  /// workers than CPUs keep the run going: raise it.
  pub max_workers: usize,
  /// Size of the blocks of records given to the workers. Memory used: about
  /// `(workers + 3) × block_bytes`: a block per worker, two waiting, one being cut.
  pub block_bytes: usize,
  /// Larger records are an error.
  pub max_record_bytes: u64,
}

impl Default for ParallelOptions {
  fn default() -> Self {
    let available = thread::available_parallelism().map_or(1, usize::from);
    Self {
      max_workers: available.saturating_sub(2).max(1),
      block_bytes: 4 * 1024 * 1024,
      max_record_bytes: 64 * 1024 * 1024,
    }
  }
}

impl ParallelOptions {
  #[must_use]
  pub fn with_max_workers(mut self, max_workers: usize) -> Self {
    self.max_workers = max_workers;
    self
  }

  #[must_use]
  pub fn with_block_bytes(mut self, block_bytes: usize) -> Self {
    self.block_bytes = block_bytes;
    self
  }

  #[must_use]
  pub fn with_max_record_bytes(mut self, max_record_bytes: u64) -> Self {
    self.max_record_bytes = max_record_bytes;
    self
  }

  fn validate(&self) -> Result<()> {
    if self.max_workers == 0 || self.block_bytes == 0 {
      return Err(ExportError::InvalidOptions("max_workers and block_bytes must be at least 1".to_owned()).into());
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use std::fmt::Write as _;
  use std::io::BufRead;
  use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
  use std::sync::{Mutex, PoisonError};

  use super::ParallelOptions;
  use super::engine::run;
  use crate::error::{DiscogsClientError, Result};
  use crate::xml_exports::error::ExportError;
  use crate::xml_exports::model::ExportRecord;
  use crate::xml_exports::model::artist::ExportArtist;
  use crate::xml_exports::model::export::ExportKind;
  use crate::xml_exports::model::label::ExportLabel;
  use crate::xml_exports::model::master::ExportMaster;
  use crate::xml_exports::model::release::ExportRelease;
  use crate::xml_exports::parser::ExportReader;
  use crate::xml_exports::parser::master::master;
  use crate::xml_exports::parser::sealed::Parse;
  use crate::xml_exports::parser::tests::{ARTISTS, LABELS, MASTERS, RELEASES};
  use crate::xml_exports::parser::xml::{Element, Xml};

  fn options(workers: usize, block_bytes: usize) -> ParallelOptions {
    ParallelOptions::default()
      .with_max_workers(workers)
      .with_block_bytes(block_bytes)
      .with_max_record_bytes(1 << 20)
  }

  /// Records parsed by the workers
  fn parallel<T: ExportRecord + Send>(content: &str, options: &ParallelOptions) -> Result<Vec<T>> {
    let records = Mutex::new(Vec::new());
    let count = run(content.as_bytes(), options, &AtomicBool::new(false), &|record| {
      records.lock().unwrap_or_else(PoisonError::into_inner).push(record?);
      Ok(())
    })?;
    let records = records.into_inner().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(count, records.len() as u64);
    Ok(records)
  }

  fn same_records<T>(content: &str, id: impl Fn(&T) -> u64)
  where
    T: ExportRecord + Send + PartialEq + std::fmt::Debug,
  {
    let mut expected: Vec<T> = ExportReader::new(content.as_bytes()).collect::<Result<_>>().unwrap();
    expected.sort_by_key(&id);
    assert!(!expected.is_empty());
    for (workers, block_bytes) in [(1, 1), (2, 64), (5, 300), (8, 4096), (64, 1 << 20)] {
      let mut records = parallel::<T>(content, &options(workers, block_bytes)).unwrap();
      records.sort_by_key(&id);
      assert_eq!(records, expected, "{workers} workers, blocks of {block_bytes} bytes");
    }
  }

  #[test]
  fn same_records_as_the_sequential_reader() {
    same_records::<ExportArtist>(ARTISTS, |artist| artist.id.0);
    same_records::<ExportLabel>(LABELS, |label| label.id.0);
    same_records::<ExportMaster>(MASTERS, |master| master.id.0);
    same_records::<ExportRelease>(RELEASES, |release| release.id.0);
  }

  /// First records of the exports of November 2026
  #[test]
  fn samples_of_the_exports() {
    const SAMPLES: [&str; 4] = [
      include_str!("../../../resources/xml_exports/sample/artists.xml"),
      include_str!("../../../resources/xml_exports/sample/labels.xml"),
      include_str!("../../../resources/xml_exports/sample/masters.xml"),
      include_str!("../../../resources/xml_exports/sample/releases.xml"),
    ];
    for (workers, block_bytes) in [(4, 4 * 1024), (8, 16 * 1024)] {
      let options = options(workers, block_bytes);
      let counts = [
        parallel::<ExportArtist>(SAMPLES[0], &options).unwrap().len(),
        parallel::<ExportLabel>(SAMPLES[1], &options).unwrap().len(),
        parallel::<ExportMaster>(SAMPLES[2], &options).unwrap().len(),
        parallel::<ExportRelease>(SAMPLES[3], &options).unwrap().len(),
      ];
      assert_eq!(counts, [100; 4], "{workers} workers");
    }
    same_records::<ExportRelease>(SAMPLES[3], |release| release.id.0);
  }

  #[test]
  fn records_larger_than_the_blocks_and_multibyte_characters() {
    let mut content = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<artists>\n");
    for id in 1..=300 {
      let profile = if id == 150 {
        "Ω≈ç√∫".repeat(20_000)
      } else {
        "é".repeat(id % 50)
      };
      let _ = writeln!(
        content,
        "<artist><id>{id}</id><name>✓ {id}</name><profile>{profile}</profile></artist>"
      );
    }
    content.push_str("</artists>\n");
    let mut artists = parallel::<ExportArtist>(&content, &options(4, 4096)).unwrap();
    artists.sort_by_key(|artist| artist.id);
    let ids: Vec<u64> = artists.iter().map(|artist| artist.id.0).collect();
    assert_eq!(ids, (1..=300).collect::<Vec<_>>());
    assert_eq!(artists[149].profile.as_deref().map(str::len), Some(13 * 20_000));
  }

  const TRICKY: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!-- <master id="999"> fake record -->
<masters>
<master id="1"><title>A</title><notes><![CDATA[ <master id="998"></master> ]]></notes></master>
<!-- </master><master id="997"> -->
<?pi <master id="996">?>
<master id="2"><title>B &amp; &lt;master id="995"&gt;</title><videos><video src="https://example.com/?a=1&amp;b=>" duration="1" embed="true"><title>t</title></video></videos></master>
<master id = "3" ><title/></master>
<master id="4"/>
</masters>
"#;

  #[test]
  fn fake_records_in_comments_cdata_and_attributes_are_ignored() {
    for block_bytes in [1, 50, 1 << 20] {
      let mut masters = parallel::<ExportMaster>(TRICKY, &options(3, block_bytes)).unwrap();
      masters.sort_by_key(|master| master.id);
      let ids: Vec<u64> = masters.iter().map(|master| master.id.0).collect();
      assert_eq!(ids, [1, 2, 3, 4]);
      assert_eq!(masters[0].notes.as_deref(), Some(r#" <master id="998"></master> "#));
      assert_eq!(masters[1].title, r#"B & <master id="995">"#);
      assert_eq!(masters[1].videos[0].uri, "https://example.com/?a=1&b=>");
    }
  }

  #[test]
  fn empty_exports() {
    for content in ["", "<masters/>", "<masters>\n</masters>"] {
      assert!(parallel::<ExportMaster>(content, &options(2, 64)).unwrap().is_empty());
    }
  }

  fn invalid_export(content: &str, options: &ParallelOptions) -> String {
    match parallel::<ExportMaster>(content, options) {
      Err(DiscogsClientError::Export(ExportError::InvalidExport(message))) => message,
      other => panic!("expected an invalid export, got {other:?}"),
    }
  }

  #[test]
  fn invalid_exports_fail_clearly() {
    let options = options(2, 8);
    let doctype = invalid_export(r#"<!DOCTYPE masters [<!ENTITY x "y">]><masters></masters>"#, &options);
    assert!(doctype.contains("unsupported"), "{doctype}");
    let truncated = invalid_export(
      r#"<masters><master id="1"><title>A</title></master><master id="2"><ti"#,
      &options,
    );
    assert!(truncated.contains("truncated"), "{truncated}");
    let wrong_root = invalid_export("<releases></releases>", &options);
    assert!(wrong_root.contains("root `releases`"), "{wrong_root}");
    let wrong_record = invalid_export(r#"<masters><release id="1"/></masters>"#, &options);
    assert!(wrong_record.contains("record `release`"), "{wrong_record}");
    let oversized = invalid_export(
      &format!(
        r#"<masters><master id="1"><notes>{}</notes></master></masters>"#,
        "x".repeat(2_000)
      ),
      &options.with_max_record_bytes(1_000),
    );
    assert!(oversized.contains("larger than 1000 bytes"), "{oversized}");
    // Accepted by the scan, refused by the parser: reported with its position
    let unknown_entity = invalid_export(
      r#"<masters><master id="1"><title>&unknown;</title></master></masters>"#,
      &options,
    );
    assert!(unknown_entity.contains("records [9, "), "{unknown_entity}");
  }

  fn masters(count: u64) -> String {
    let masters: String = (1..=count).map(|id| format!(r#"<master id="{id}"/>"#)).collect();
    format!("<masters>{masters}</masters>")
  }

  #[test]
  fn first_error_of_the_function_stops_the_run() {
    let calls = AtomicUsize::new(0);
    let result = run::<ExportMaster, _>(
      masters(2_000).as_bytes(),
      &options(4, 64),
      &AtomicBool::new(false),
      &|_| {
        calls.fetch_add(1, Ordering::Relaxed);
        Err(ExportError::InvalidOptions("stop".to_owned()).into())
      },
    );
    assert!(matches!(
      result,
      Err(DiscogsClientError::Export(ExportError::InvalidOptions(message))) if message == "stop"
    ));
    assert!(calls.into_inner() <= 4);
  }

  #[test]
  fn slow_function() {
    let calls = AtomicUsize::new(0);
    run::<ExportMaster, _>(
      masters(50).as_bytes(),
      &options(4, 16),
      &AtomicBool::new(false),
      &|master| {
        master?;
        std::thread::sleep(std::time::Duration::from_millis(1));
        calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
      },
    )
    .unwrap();
    assert_eq!(calls.into_inner(), 50);
  }

  /// Master whose parsing panics on the id 2
  #[derive(Clone, Debug)]
  struct Panicking;

  impl ExportRecord for Panicking {
    const KIND: ExportKind = ExportKind::Masters;
  }

  impl Parse for Panicking {
    const TAG: &'static [u8] = b"master";

    fn parse<R: BufRead>(xml: &mut Xml<R>, element: Element) -> Result<Self> {
      let master = master(xml, element)?;
      assert_ne!(master.id.0, 2, "parser bug");
      Ok(Self)
    }
  }

  #[test]
  fn worker_panics_are_reported() {
    let result = run::<Panicking, _>(
      masters(100).as_bytes(),
      &options(4, 32),
      &AtomicBool::new(false),
      &|_| Ok(()),
    );
    assert!(matches!(
      result,
      Err(DiscogsClientError::Export(ExportError::InvalidExport(message))) if message.contains("panicked")
    ));
  }

  #[test]
  fn invalid_options() {
    let result = run::<ExportMaster, _>(
      MASTERS.as_bytes(),
      &options(0, 64),
      &AtomicBool::new(false),
      &|_| Ok(()),
    );
    assert!(matches!(
      result,
      Err(DiscogsClientError::Export(ExportError::InvalidOptions(_)))
    ));
  }
}
