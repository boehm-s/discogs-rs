//! Monthly XML exports of the Discogs database (<https://data.discogs.com>), behind the
//! `xml-exports` feature.
//!
//! Each month, Discogs publishes a complete export of its artists, labels, masters and releases:
//! one gzipped XML file per kind, from ~90 MB (labels) to ~11 GB (releases, ~65 GB once
//! decompressed). Exports are snapshots, not diffs: to keep a copy of the database up to date,
//! ingest each new export and upsert its records by id.
//!
//! [`ExportFile`](client::ExportFile) is the entry point: an export file of one kind of records,
//! the latest one published or any file, local or remote, gzipped or not. Its records are passed
//! to a function, from one thread or several:
//!
//! ```no_run
//! # async fn example(last_ingested: chrono::NaiveDate) -> discogs_client::error::Result<()> {
//! use discogs_client::client::Client;
//! use discogs_client::xml_exports::client::ExportFile;
//! use discogs_client::xml_exports::model::release::ExportRelease;
//!
//! let client = Client::builder("MyCollectionApp/1.0 +https://example.com").build()?;
//! let releases = ExportFile::<ExportRelease>::latest(&client).await?;
//! if releases.date().is_some_and(|date| date > last_ingested) {
//!   releases
//!     .for_each_sequential(|release| {
//!       let release = release?;
//!       // Upsert the release by `release.id`…
//!       Ok(())
//!     })
//!     .await?;
//! }
//! # Ok(())
//! # }
//! ```
//!
//! Memory stays bounded whatever the size of the export, so a small machine can process the
//! largest one. Remote files are parsed as they download, without being written to disk.

pub mod client;
pub mod error;
pub mod model;
pub mod parallel;
pub(crate) mod parser;
pub(crate) mod service;
#[cfg(test)]
pub(crate) mod test_helper;

pub(crate) const EXPORTS_URL: &str = "https://data.discogs.com/";
