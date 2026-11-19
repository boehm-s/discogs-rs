//! [`ExportFile`]: an export file of one kind of records, local or remote, and its processing.

use std::fs::File;
use std::io::{BufRead as _, Read};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use tokio::sync::oneshot;
use tracing::instrument;
use url::Url;

use crate::client::Client;
use crate::error::{DiscogsClientError, Result};
use crate::xml_exports::error::ExportError;
use crate::xml_exports::model::ExportRecord;
use crate::xml_exports::parallel::{ParallelOptions, engine};
use crate::xml_exports::parser::ExportReader;
use crate::xml_exports::parser::input::{Checksum, Input};
use crate::xml_exports::service::download::{download_file, open_remote};
use crate::xml_exports::service::listing::{download_path, fetch_checksums, fetch_latest_export};

/// Where an export file is.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum ExportLocation {
  Local(PathBuf),
  /// Downloaded with the client's user agent, never with its API credentials.
  Remote {
    client: Client,
    url: Url,
  },
}

impl ExportLocation {
  pub fn local(path: impl Into<PathBuf>) -> Self {
    Self::Local(path.into())
  }

  pub fn remote(client: &Client, url: Url) -> Self {
    Self::Remote {
      client: client.clone(),
      url,
    }
  }

  /// HTTP(S) URL of a file, such as a link of data.discogs.com
  /// (`https://data.discogs.com/?download=data%2F2026%2Fdiscogs_20261101_labels.xml.gz`).
  ///
  /// ```
  /// # use discogs_client::client::Client;
  /// # use discogs_client::xml_exports::client::ExportLocation;
  /// # fn main() -> discogs_client::error::Result<()> {
  /// let client = Client::builder("MyApp/1.0").build()?;
  /// let location = ExportLocation::parse_remote(
  ///   &client,
  ///   "https://data.discogs.com/?download=data%2F2026%2Fdiscogs_20261101_labels.xml.gz",
  /// )?;
  /// # Ok(())
  /// # }
  /// ```
  pub fn parse_remote(client: &Client, url: &str) -> Result<Self> {
    let invalid = |reason: &str| ExportError::InvalidOptions(format!("`{url}` {reason}"));
    let url = Url::parse(url).map_err(|error| invalid(&format!("is not a URL: {error}")))?;
    if !matches!(url.scheme(), "http" | "https") {
      return Err(invalid("is not an HTTP(S) URL").into());
    }
    file_name(&url)?;
    Ok(Self::remote(client, url))
  }
}

impl From<PathBuf> for ExportLocation {
  fn from(path: PathBuf) -> Self {
    Self::Local(path)
  }
}

impl From<&Path> for ExportLocation {
  fn from(path: &Path) -> Self {
    Self::Local(path.to_owned())
  }
}

/// Result of a processing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExportStats {
  /// Records passed to the function, invalid ones included
  pub records: u64,
  pub elapsed: Duration,
}

/// Export file of the records `T` ([`ExportRelease`], [`ExportArtist`]…), local or remote,
/// gzipped (as published by Discogs) or not.
///
/// Its records are passed one by one to a function, from one thread
/// ([`for_each_sequential`](Self::for_each_sequential)) or several
/// ([`for_each_parallel`](Self::for_each_parallel)). Memory stays bounded whatever the size of
/// the file, and a remote file is parsed as it downloads, without being written to disk.
///
/// ```no_run
/// # async fn example(client: discogs_client::client::Client) -> discogs_client::error::Result<()> {
/// use discogs_client::xml_exports::client::ExportFile;
/// use discogs_client::xml_exports::model::release::ExportRelease;
/// use discogs_client::xml_exports::parallel::ParallelOptions;
///
/// let releases = ExportFile::<ExportRelease>::latest(&client).await?;
/// let stats = releases
///   .for_each_parallel(&ParallelOptions::default(), |release| {
///     let release = release?;
///     // Convert and write `release`…
///     Ok(())
///   })
///   .await?;
/// # Ok(())
/// # }
/// ```
///
/// [`ExportRelease`]: crate::xml_exports::model::release::ExportRelease
/// [`ExportArtist`]: crate::xml_exports::model::artist::ExportArtist
#[derive(Clone, Debug)]
pub struct ExportFile<T> {
  location: ExportLocation,
  sha256: Option<String>,
  date: Option<NaiveDate>,
  /// Only the type of the records: `fn() -> T` keeps the file `Send` and `Sync` whatever `T`
  record: PhantomData<fn() -> T>,
}

impl<T: ExportRecord> ExportFile<T> {
  /// XML file, gzipped as published by Discogs or decompressed: the compression is detected.
  pub fn from_file(location: impl Into<ExportLocation>) -> Self {
    Self {
      location: location.into(),
      sha256: None,
      date: None,
      record: PhantomData,
    }
  }

  /// File of the latest export published on data.discogs.com, verified against the checksums
  /// Discogs publishes with it (once published: releases come last, hours after the others).
  ///
  /// Discogs rate limits this server aggressively (up to 15 minutes):
  /// [`RateLimited`](DiscogsClientError::RateLimited) gives the time to wait.
  #[instrument(skip_all)]
  pub async fn latest(client: &Client) -> Result<Self> {
    let export = fetch_latest_export(client).await?;
    let file = export
      .file(T::KIND)
      .ok_or_else(|| ExportError::NotFound(format!("{} of the {} export", T::KIND, export.date)))?;
    let sha256 = match &export.checksum_file {
      Some(_) => Some(
        fetch_checksums(client, &export)
          .await?
          .get(file)
          .ok_or_else(|| ExportError::NotFound(format!("checksum of {}", file.name)))?
          .to_owned(),
      ),
      None => None,
    };
    let url = client.exports_file_url(&download_path(&export, &file.name))?;
    Ok(Self {
      sha256,
      date: Some(export.date),
      ..Self::from_file(ExportLocation::remote(client, url))
    })
  }

  /// Verifies the file against its SHA-256, in hexadecimal. A mismatch is only known once the
  /// whole file was read: it is returned after the function was called with every record.
  #[must_use]
  pub fn with_sha256(mut self, sha256: impl Into<String>) -> Self {
    self.sha256 = Some(sha256.into().to_ascii_lowercase());
    self
  }

  pub fn location(&self) -> &ExportLocation {
    &self.location
  }

  /// Date of the export, for [`latest`](Self::latest).
  pub fn date(&self) -> Option<NaiveDate> {
    self.date
  }

  /// Saves a remote file into `directory` (verified, when its checksum is known), and returns
  /// it as a local file; a local file is returned as is. Discogs does not support resuming a
  /// download: on an unreliable network, download the file before processing it.
  #[instrument(skip_all)]
  pub async fn download(&self, directory: impl AsRef<Path>) -> Result<Self> {
    let ExportLocation::Remote { client, url } = &self.location else {
      return Ok(self.clone());
    };
    let destination = directory.as_ref().join(file_name(url)?);
    download_file(client, url, &destination, self.sha256.as_deref()).await?;
    Ok(Self {
      location: ExportLocation::Local(destination),
      // Verified
      sha256: None,
      ..self.clone()
    })
  }

  /// Calls `f` with each record, in file order, from a thread of its own. Reading and
  /// decompression run on another thread.
  ///
  /// [`ExportError::InvalidRecord`] only concerns its record: `f` decides whether to go on. The
  /// first error returned by `f`, or met in the file, stops the processing and is returned.
  #[instrument(skip_all)]
  pub async fn for_each_sequential<F>(&self, mut f: F) -> Result<ExportStats>
  where
    F: FnMut(Result<T>) -> Result<()> + Send + 'static,
  {
    self
      .process(move |mut input, stop| {
        // Errors of the input thread, before the XML parser wraps them
        input.fill_buf()?;
        let mut records = 0;
        for record in ExportReader::<T, _>::new(&mut input) {
          if stop.load(Ordering::Relaxed) {
            // Nobody waits for the result anymore
            return Ok(records);
          }
          match record {
            Ok(_) | Err(DiscogsClientError::Export(ExportError::InvalidRecord(_))) => {
              records += 1;
              f(record)?;
            }
            Err(error) => return Err(error),
          }
        }
        input.finish()?;
        Ok(records)
      })
      .await
  }

  /// Calls `f` with each record from several threads (see [`parallel`](crate::xml_exports::parallel)):
  /// in file order within a block of records, unordered across blocks.
  ///
  /// [`ExportError::InvalidRecord`] only concerns its record: `f` decides whether to go on. The
  /// first error returned by `f`, or met in the file, stops the processing and is returned.
  #[instrument(skip_all)]
  pub async fn for_each_parallel<F>(&self, options: &ParallelOptions, f: F) -> Result<ExportStats>
  where
    F: Fn(Result<T>) -> Result<()> + Send + Sync + 'static,
  {
    let options = *options;
    self
      .process(move |mut input, stop| {
        let records = engine::run(&mut input, &options, stop, &f)?;
        input.finish()?;
        Ok(records)
      })
      .await
  }

  /// Runs `parse` on a thread of its own, with the decompressed file. Dropping the future stops
  /// it.
  async fn process<P>(&self, parse: P) -> Result<ExportStats>
  where
    P: FnOnce(Input, &AtomicBool) -> Result<u64> + Send + 'static,
  {
    let started = Instant::now();
    let source: Box<dyn Read + Send> = match &self.location {
      ExportLocation::Local(path) => Box::new(File::open(path)?),
      ExportLocation::Remote { client, url } => Box::new(open_remote(client, url).await?),
    };
    let checksum = self.sha256.clone().map(|sha256| Checksum {
      file: self.name(),
      sha256,
    });
    let input = Input::spawn(source, checksum)?;
    let stop = StopOnDrop(Arc::new(AtomicBool::new(false)));
    let (sender, result) = oneshot::channel();
    let flag = Arc::clone(&stop.0);
    thread::Builder::new()
      .name("discogs-export-parser".to_owned())
      .spawn(move || {
        let _ = sender.send(parse(input, &flag));
      })?;
    let records = result
      .await
      .unwrap_or_else(|_| Err(ExportError::InvalidExport("the parsing thread panicked".to_owned()).into()))?;
    Ok(ExportStats {
      records,
      elapsed: started.elapsed(),
    })
  }

  fn name(&self) -> String {
    match &self.location {
      ExportLocation::Local(path) => path.display().to_string(),
      ExportLocation::Remote { url, .. } => file_name(url).unwrap_or_else(|_| url.to_string()),
    }
  }
}

/// Last segment of the downloaded path: `discogs_20261101_releases.xml.gz`
fn file_name(url: &Url) -> Result<String> {
  let path = url
    .query_pairs()
    .find(|(name, _)| name == "download")
    .map_or_else(|| url.path().to_owned(), |(_, path)| path.into_owned());
  path
    .rsplit('/')
    .next()
    .filter(|name| !name.is_empty())
    .map(str::to_owned)
    .ok_or_else(|| ExportError::InvalidOptions(format!("no file name in {url}")).into())
}

/// Stops the parsing when the future processing the file is dropped
struct StopOnDrop(Arc<AtomicBool>);

impl Drop for StopOnDrop {
  fn drop(&mut self) {
    self.0.store(true, Ordering::Relaxed);
  }
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;
  use std::sync::{Arc, Mutex};
  use std::time::Duration;

  use chrono::NaiveDate;
  use reqwest::header::AUTHORIZATION;
  use sha2::{Digest as _, Sha256};
  use wiremock::{MockServer, ResponseTemplate};

  use super::{ExportFile, ExportLocation};
  use crate::client::Client;
  use crate::error::{DiscogsClientError, Result};
  use crate::xml_exports::error::ExportError;
  use crate::xml_exports::model::artist::ExportArtist;
  use crate::xml_exports::model::master::ExportMaster;
  use crate::xml_exports::parallel::ParallelOptions;
  use crate::xml_exports::parser::tests::{MASTERS, gzip};
  use crate::xml_exports::service::download::hex;
  use crate::xml_exports::test_helper::exports_api::{
    CHECKSUMS, EXPORTS_2026_PAGE, YEARS_PAGE, exports_client, mount_download_api, mount_listing_api,
  };

  const MASTERS_PATH: &str = "data/2026/discogs_20261101_masters.xml.gz";
  const TITLES: [&str; 3] = ["Biologique", "Музіка", "Seven Veils Of Silence"];

  /// Temporary directory, deleted on drop
  struct TempDir(PathBuf);

  impl TempDir {
    fn new(test: &str) -> Self {
      let path = std::env::temp_dir().join(format!("discogs-client-{test}-{}", std::process::id()));
      std::fs::create_dir_all(&path).unwrap();
      Self(path)
    }
  }

  impl Drop for TempDir {
    fn drop(&mut self) {
      let _ = std::fs::remove_dir_all(&self.0);
    }
  }

  fn sha256(body: &[u8]) -> String {
    hex(&Sha256::digest(body))
  }

  async fn remote_masters(server: &MockServer) -> (Client, ExportFile<ExportMaster>) {
    let body = gzip(MASTERS);
    let checksum = sha256(&body);
    mount_download_api(server, MASTERS_PATH, ResponseTemplate::new(200).set_body_bytes(body)).await;
    let client = exports_client(server);
    let url = client.exports_file_url(MASTERS_PATH).unwrap();
    let file = ExportFile::from_file(ExportLocation::remote(&client, url)).with_sha256(checksum);
    (client, file)
  }

  async fn sequential_titles(file: &ExportFile<ExportMaster>) -> Result<Vec<String>> {
    let titles = Arc::new(Mutex::new(Vec::new()));
    let collected = Arc::clone(&titles);
    let stats = file
      .for_each_sequential(move |master| {
        collected.lock().unwrap().push(master?.title);
        Ok(())
      })
      .await?;
    let titles = titles.lock().unwrap().clone();
    assert_eq!(stats.records, titles.len() as u64);
    Ok(titles)
  }

  async fn parallel_titles(file: &ExportFile<ExportMaster>) -> Result<Vec<String>> {
    let titles = Arc::new(Mutex::new(Vec::new()));
    let collected = Arc::clone(&titles);
    let options = ParallelOptions::default().with_max_workers(2).with_block_bytes(64);
    let stats = file
      .for_each_parallel(&options, move |master| {
        collected.lock().unwrap().push(master?.title);
        Ok(())
      })
      .await?;
    let mut titles = titles.lock().unwrap().clone();
    assert_eq!(stats.records, titles.len() as u64);
    titles.sort_by_key(|title| TITLES.iter().position(|expected| expected == title));
    Ok(titles)
  }

  #[tokio::test]
  async fn latest() {
    let server = MockServer::start().await;
    mount_listing_api(&server, "data/", YEARS_PAGE).await;
    mount_listing_api(&server, "data/2026/", EXPORTS_2026_PAGE).await;
    mount_download_api(
      &server,
      "data/2026/discogs_20261101_CHECKSUM.txt",
      ResponseTemplate::new(200).set_body_string(CHECKSUMS),
    )
    .await;

    let file = ExportFile::<ExportMaster>::latest(&exports_client(&server))
      .await
      .unwrap();

    assert_eq!(file.date(), NaiveDate::from_ymd_opt(2026, 11, 1));
    assert_eq!(
      file.sha256.as_deref(),
      Some("75eb03aeb1d03a07797ffdcb57a4b81c87cf10955353c800fa7b6bc86aac1d67")
    );
    assert_eq!(file.name(), "discogs_20261101_masters.xml.gz");
    // The API credentials are not sent to the exports server
    let requests = server.received_requests().await.unwrap();
    assert!(
      requests
        .iter()
        .all(|request| !request.headers.contains_key(AUTHORIZATION))
    );
  }

  #[tokio::test]
  async fn latest_of_the_previous_year_in_january() {
    let server = MockServer::start().await;
    mount_listing_api(&server, "data/", YEARS_PAGE).await;
    mount_listing_api(&server, "data/2026/", "<html><body>No export yet</body></html>").await;
    // The page of 2026 stands for the one of 2025, without checksums
    let without_checksums: String = EXPORTS_2026_PAGE
      .lines()
      .filter(|line| !line.contains("CHECKSUM"))
      .collect();
    mount_listing_api(&server, "data/2025/", &without_checksums).await;

    let file = ExportFile::<ExportArtist>::latest(&exports_client(&server))
      .await
      .unwrap();

    assert_eq!(file.date(), NaiveDate::from_ymd_opt(2026, 11, 1));
    assert_eq!(file.sha256, None);
  }

  #[tokio::test]
  async fn remote_file_sequentially() {
    let server = MockServer::start().await;
    let (_, file) = remote_masters(&server).await;
    assert_eq!(sequential_titles(&file).await.unwrap(), TITLES);
  }

  #[tokio::test]
  async fn remote_file_in_parallel() {
    let server = MockServer::start().await;
    let (_, file) = remote_masters(&server).await;
    assert_eq!(parallel_titles(&file).await.unwrap(), TITLES);
  }

  #[tokio::test]
  async fn wrong_checksum_after_all_the_records() {
    for parallel in [false, true] {
      let server = MockServer::start().await;
      let (_, file) = remote_masters(&server).await;
      let file = file.with_sha256("0123");
      let calls = Arc::new(Mutex::new(0));
      let counted = Arc::clone(&calls);
      let count = move |_: Result<ExportMaster>| {
        *counted.lock().unwrap() += 1;
        Ok(())
      };
      let result = if parallel {
        file.for_each_parallel(&ParallelOptions::default(), count).await
      } else {
        file.for_each_sequential(count).await
      };
      assert!(
        matches!(
          &result,
          Err(DiscogsClientError::Export(ExportError::ChecksumMismatch { expected, file, .. }))
            if expected == "0123" && file == "discogs_20261101_masters.xml.gz"
        ),
        "{result:?}"
      );
      assert_eq!(*calls.lock().unwrap(), 3, "parallel: {parallel}");
    }
  }

  #[tokio::test]
  async fn local_files_plain_and_gzipped() {
    let directory = TempDir::new("local_files");
    let plain = directory.0.join("masters.xml");
    let gzipped = directory.0.join("masters.xml.gz");
    std::fs::write(&plain, MASTERS).unwrap();
    std::fs::write(&gzipped, gzip(MASTERS)).unwrap();

    for file in [
      ExportFile::<ExportMaster>::from_file(plain.as_path()),
      ExportFile::from_file(gzipped.as_path()).with_sha256(sha256(&gzip(MASTERS))),
    ] {
      assert_eq!(sequential_titles(&file).await.unwrap(), TITLES);
      assert_eq!(parallel_titles(&file).await.unwrap(), TITLES);
    }
  }

  #[tokio::test]
  async fn invalid_records_are_passed_to_the_function() {
    let directory = TempDir::new("invalid_records");
    let path = directory.0.join("artists.xml");
    std::fs::write(
      &path,
      "<artists><artist><name>No id</name></artist><artist><id>7</id><name>Ok</name></artist></artists>",
    )
    .unwrap();
    let results = Arc::new(Mutex::new(Vec::new()));
    let collected = Arc::clone(&results);

    let stats = ExportFile::<ExportArtist>::from_file(path)
      .for_each_sequential(move |artist| {
        collected.lock().unwrap().push(artist.map(|artist| artist.id.0));
        Ok(())
      })
      .await
      .unwrap();

    let results = results.lock().unwrap();
    assert_eq!(stats.records, 2);
    assert!(matches!(
      results[0],
      Err(DiscogsClientError::Export(ExportError::InvalidRecord(_)))
    ));
    assert_eq!(results[1].as_ref().ok(), Some(&7));
  }

  #[tokio::test]
  async fn first_error_of_the_function_stops_the_processing() {
    let server = MockServer::start().await;
    let (_, file) = remote_masters(&server).await;
    let calls = Arc::new(Mutex::new(0));
    let counted = Arc::clone(&calls);

    let result = file
      .for_each_sequential(move |_| {
        *counted.lock().unwrap() += 1;
        Err(ExportError::InvalidOptions("stop".to_owned()).into())
      })
      .await;

    assert!(matches!(
      result,
      Err(DiscogsClientError::Export(ExportError::InvalidOptions(message))) if message == "stop"
    ));
    assert_eq!(*calls.lock().unwrap(), 1);
  }

  #[tokio::test]
  async fn download() {
    let server = MockServer::start().await;
    let (_, file) = remote_masters(&server).await;
    let directory = TempDir::new("download");

    let local = file.download(&directory.0).await.unwrap();

    let path = directory.0.join("discogs_20261101_masters.xml.gz");
    assert!(matches!(local.location(), ExportLocation::Local(local) if *local == path));
    assert_eq!(std::fs::read(&path).unwrap(), gzip(MASTERS));
    assert_eq!(sequential_titles(&local).await.unwrap(), TITLES);
  }

  #[tokio::test]
  async fn download_with_wrong_checksum() {
    let server = MockServer::start().await;
    let (_, file) = remote_masters(&server).await;
    let directory = TempDir::new("download_with_wrong_checksum");

    let result = file.with_sha256("0123").download(&directory.0).await;

    assert!(matches!(
      result,
      Err(DiscogsClientError::Export(ExportError::ChecksumMismatch { .. }))
    ));
    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 0);
  }

  #[tokio::test]
  async fn remote_file_from_its_link() {
    let server = MockServer::start().await;
    mount_download_api(
      &server,
      MASTERS_PATH,
      ResponseTemplate::new(200).set_body_bytes(gzip(MASTERS)),
    )
    .await;
    let client = exports_client(&server);
    let link = format!(
      "{}/?download=data%2F2026%2Fdiscogs_20261101_masters.xml.gz",
      server.uri()
    );

    let file = ExportFile::from_file(ExportLocation::parse_remote(&client, &link).unwrap());

    assert_eq!(sequential_titles(&file).await.unwrap(), TITLES);
    assert_eq!(file.name(), "discogs_20261101_masters.xml.gz");
    for (link, reason) in [
      ("data/2026/discogs_20261101_masters.xml.gz", "is not a URL"),
      ("ftp://example.com/masters.xml.gz", "is not an HTTP(S) URL"),
      ("https://example.com/", "no file name"),
    ] {
      match ExportLocation::parse_remote(&client, link) {
        Err(DiscogsClientError::Export(ExportError::InvalidOptions(message))) => {
          assert!(message.contains(reason), "{message}");
        }
        other => panic!("expected invalid options for {link}, got {other:?}"),
      }
    }
  }

  #[tokio::test]
  async fn rate_limited() {
    let server = MockServer::start().await;
    mount_download_api(
      &server,
      MASTERS_PATH,
      ResponseTemplate::new(429)
        .insert_header("Retry-After", "912")
        .set_body_raw(
          r#"{"message": "You are making requests too quickly."}"#,
          "application/json",
        ),
    )
    .await;
    let client = exports_client(&server);
    let url = client.exports_file_url(MASTERS_PATH).unwrap();

    let result = ExportFile::<ExportMaster>::from_file(ExportLocation::remote(&client, url))
      .for_each_sequential(|_| Ok(()))
      .await;

    assert!(matches!(
      result,
      Err(DiscogsClientError::RateLimited {
        retry_after: Some(retry_after)
      }) if retry_after == Duration::from_secs(912)
    ));
  }
}

#[cfg(all(test, feature = "live-tests"))]
mod live_tests {
  use crate::auth::test_helper::USER_AGENT;
  use crate::client::Client;
  use crate::xml_exports::client::ExportFile;
  use crate::xml_exports::model::label::ExportLabel;
  use crate::xml_exports::parallel::ParallelOptions;

  /// The labels export is the smallest: about 90 MB
  #[tokio::test]
  async fn latest_label_file_streamed_and_verified() {
    let client = Client::builder(USER_AGENT).build().unwrap();

    let file = ExportFile::<ExportLabel>::latest(&client).await.unwrap();
    assert!(file.date().is_some());
    assert_eq!(file.sha256.as_deref().map(str::len), Some(64));

    let stats = file
      .for_each_parallel(&ParallelOptions::default(), |record| record.map(drop))
      .await
      .unwrap();
    assert!(stats.records > 1_000_000, "{stats:?}");
  }
}
