//! Parses a local export and prints its number of records and the throughput. With a number of
//! workers, parses it in parallel.
//!
//! `cargo run --release --features xml-exports --example parse_export -- discogs_20261101_masters.xml.gz [workers]`

#![expect(clippy::print_stdout, reason = "command-line example")]
#![expect(unused_crate_dependencies, reason = "examples see all the dependencies of the crate")]

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use discogs_client::error::{DiscogsClientError, Result};
use discogs_client::xml_exports::client::ExportFile;
use discogs_client::xml_exports::error::ExportError;
use discogs_client::xml_exports::model::ExportRecord;
use discogs_client::xml_exports::model::artist::ExportArtist;
use discogs_client::xml_exports::model::label::ExportLabel;
use discogs_client::xml_exports::model::master::ExportMaster;
use discogs_client::xml_exports::model::release::ExportRelease;
use discogs_client::xml_exports::parallel::ParallelOptions;

async fn count<T: ExportRecord + 'static>(path: &Path, workers: usize) -> Result<()> {
  let file = ExportFile::<T>::from_file(path);
  let invalid = Arc::new(AtomicU64::new(0));
  let counted = Arc::clone(&invalid);
  let tally = move |record: Result<T>| {
    if let Err(DiscogsClientError::Export(ExportError::InvalidRecord(error))) = record {
      println!("invalid record: {error}");
      counted.fetch_add(1, Ordering::Relaxed);
      return Ok(());
    }
    record.map(drop)
  };
  let stats = if workers == 0 {
    file.for_each_sequential(tally).await?
  } else {
    file
      .for_each_parallel(&ParallelOptions::default().with_max_workers(workers), tally)
      .await?
  };
  let seconds = stats.elapsed.as_secs_f64();
  let mebibytes = std::fs::metadata(path)?.len() as f64 / f64::from(1 << 20);
  println!(
    "{workers} workers: {} records ({} invalid) in {seconds:.2}s, {:.0} records/s, {:.0} MiB/s of file",
    stats.records,
    invalid.load(Ordering::Relaxed),
    stats.records as f64 / seconds,
    mebibytes / seconds
  );
  Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
  let mut args = std::env::args().skip(1);
  let path = args.next().unwrap_or_default();
  let workers = args.next().and_then(|workers| workers.parse().ok()).unwrap_or(0);
  let path = Path::new(&path);
  let name = path.to_string_lossy();
  if name.contains("artists") {
    count::<ExportArtist>(path, workers).await
  } else if name.contains("labels") {
    count::<ExportLabel>(path, workers).await
  } else if name.contains("masters") {
    count::<ExportMaster>(path, workers).await
  } else {
    count::<ExportRelease>(path, workers).await
  }
}
