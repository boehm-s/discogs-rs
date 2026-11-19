//! Index of the exports: the HTML pages of data.discogs.com.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use chrono::NaiveDate;
use scraper::{Html, Selector};
use tracing::instrument;

use crate::client::Client;
use crate::error::{DiscogsClientError, Result};
use crate::xml_exports::error::ExportError;
use crate::xml_exports::model::export::{Export, ExportChecksums, ExportKind, PublishedFile};

#[expect(
  clippy::expect_used,
  reason = "constant selector, parsed by every test of the listing"
)]
static LINKS: LazyLock<Selector> = LazyLock::new(|| Selector::parse("a[href]").expect("valid selector"));

/// Most recent export. The first export of a year is listed once published: until then, the
/// previous year has the latest one.
#[instrument(skip(client))]
pub(crate) async fn fetch_latest_export(client: &Client) -> Result<Export> {
  for year in fetch_export_years(client).await?.into_iter().rev().take(2) {
    match fetch_exports(client, year).await {
      Ok(mut exports) => {
        if let Some(export) = exports.pop() {
          return Ok(export);
        }
      }
      Err(DiscogsClientError::Export(ExportError::NotFound(_))) => {}
      Err(error) => return Err(error),
    }
  }
  Err(ExportError::NotFound("no export published".to_owned()).into())
}

/// Years with exports (`?prefix=data/`), in order
#[instrument(skip(client))]
pub(crate) async fn fetch_export_years(client: &Client) -> Result<Vec<i32>> {
  let page = fetch_text(client, &[("prefix", "data/")]).await?;
  parse_years(&page)
}

/// Exports of a year, oldest first (`?prefix=data/2026/`). `NotFound` when none is listed yet.
#[instrument(skip(client))]
pub(crate) async fn fetch_exports(client: &Client, year: i32) -> Result<Vec<Export>> {
  let page = fetch_text(client, &[("prefix", &format!("data/{year}/"))]).await?;
  parse_exports(&page)
}

/// `discogs_YYYYMMDD_CHECKSUM.txt`: `<sha256> <file name>` per line
#[instrument(skip(client))]
pub(crate) async fn fetch_checksums(client: &Client, export: &Export) -> Result<ExportChecksums> {
  let file = export
    .checksum_file
    .as_deref()
    .ok_or_else(|| ExportError::NotFound(format!("checksums of the {} export", export.date)))?;
  let text = fetch_text(client, &[("download", &download_path(export, file))]).await?;
  parse_checksums(&text)
}

/// `data/2026/discogs_20261101_releases.xml.gz`
pub(crate) fn download_path(export: &Export, file: &str) -> String {
  format!("data/{}/{file}", export.date.format("%Y"))
}

async fn fetch_text(client: &Client, query: &[(&str, &str)]) -> Result<String> {
  Ok(Client::send(client.exports_request(query)).await?.text().await?)
}

fn invalid(message: String) -> DiscogsClientError {
  ExportError::InvalidListing(message).into()
}

/// Values of the `parameter` of the page's links (`<a href="?prefix=data%2F2026%2F">`), decoded.
fn links(page: &str, parameter: &str) -> Vec<String> {
  Html::parse_document(page)
    .select(&LINKS)
    .filter_map(|link| {
      let query = link.value().attr("href")?.strip_prefix('?')?;
      url::form_urlencoded::parse(query.as_bytes())
        .find_map(|(name, value)| (name == parameter).then(|| value.into_owned()))
    })
    .collect()
}

/// Every folder of `data/` is a year: `data/2026/`.
fn parse_years(page: &str) -> Result<Vec<i32>> {
  let mut years = links(page, "prefix")
    .iter()
    .map(|path| {
      path
        .strip_prefix("data/")
        .and_then(|year| year.strip_suffix('/'))
        .and_then(|year| year.parse().ok())
        .ok_or_else(|| invalid(format!("folder `{path}` instead of a year")))
    })
    .collect::<Result<Vec<i32>>>()?;
  if years.is_empty() {
    return Err(invalid("no year of exports".to_owned()));
  }
  years.sort_unstable();
  years.dedup();
  Ok(years)
}

/// Every file of a year is an export file (`discogs_20261101_releases.xml.gz`) or its checksums
/// (`discogs_20261101_CHECKSUM.txt`).
pub(crate) fn parse_exports(page: &str) -> Result<Vec<Export>> {
  let paths = links(page, "download");
  if paths.is_empty() {
    return Err(ExportError::NotFound("no export listed".to_owned()).into());
  }
  let mut exports: BTreeMap<NaiveDate, Export> = BTreeMap::new();
  for path in paths {
    let name = path.rsplit_once('/').map_or(path.as_str(), |(_, name)| name);
    let (date, kind) = parse_file_name(name).ok_or_else(|| invalid(format!("unexpected file `{path}`")))?;
    let export = exports.entry(date).or_insert_with(|| Export {
      date,
      files: Vec::new(),
      checksum_file: None,
    });
    match kind {
      Some(kind) => export.files.push(PublishedFile {
        kind,
        name: name.to_owned(),
      }),
      None => export.checksum_file = Some(name.to_owned()),
    }
  }
  Ok(exports.into_values().collect())
}

/// Date and kind of an export file, no kind for the checksums
fn parse_file_name(name: &str) -> Option<(NaiveDate, Option<ExportKind>)> {
  let (date, suffix) = name.strip_prefix("discogs_")?.split_once('_')?;
  let date = NaiveDate::parse_from_str(date, "%Y%m%d").ok()?;
  if suffix == "CHECKSUM.txt" {
    return Some((date, None));
  }
  let kind = ExportKind::ALL
    .into_iter()
    .find(|kind| suffix == format!("{kind}.xml.gz"))?;
  Some((date, Some(kind)))
}

fn parse_checksums(text: &str) -> Result<ExportChecksums> {
  if text.trim().is_empty() {
    return Err(invalid("empty checksum file".to_owned()));
  }
  let checksums = text
    .lines()
    .filter(|line| !line.trim().is_empty())
    .map(|line| {
      line
        .trim()
        .split_once(char::is_whitespace)
        .filter(|(checksum, _)| checksum.len() == 64 && checksum.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map(|(checksum, name)| (name.trim().to_owned(), checksum.to_ascii_lowercase()))
        .ok_or_else(|| invalid(format!("checksum line `{line}`")))
    })
    .collect::<Result<_>>()?;
  Ok(ExportChecksums(checksums))
}

#[cfg(test)]
mod tests {
  use super::{parse_checksums, parse_exports, parse_years};
  use crate::error::{DiscogsClientError, Result};
  use crate::xml_exports::error::ExportError;
  use crate::xml_exports::test_helper::exports_api::{CHECKSUMS, EXPORTS_2026_PAGE, YEARS_PAGE};

  fn page(links: &[&str]) -> String {
    let links: String = links
      .iter()
      .map(|link| format!(r#"<a href="{link}">file</a>"#))
      .collect();
    format!("<html><body><pre>{links}</pre></body></html>")
  }

  fn assert_invalid<T: std::fmt::Debug>(result: Result<T>, expected: &str) {
    match result {
      Err(DiscogsClientError::Export(ExportError::InvalidListing(message))) => {
        assert!(message.contains(expected), "{message}");
      }
      other => panic!("expected an invalid listing, got {other:?}"),
    }
  }

  #[test]
  fn listing_pages() {
    let years = parse_years(YEARS_PAGE).unwrap();
    assert_eq!(
      (years.first(), years.last(), years.len()),
      (Some(&2008), Some(&2026), 19)
    );

    let exports = parse_exports(EXPORTS_2026_PAGE).unwrap();
    assert_eq!(exports.len(), 11);
    assert!(
      exports
        .iter()
        .all(|export| export.files.len() == 4 && export.checksum_file.is_some())
    );

    assert_eq!(parse_checksums(CHECKSUMS).unwrap().0.len(), 4);
  }

  #[test]
  fn nothing_listed() {
    assert!(matches!(
      parse_exports(&page(&["?prefix=data%2F"])),
      Err(DiscogsClientError::Export(ExportError::NotFound(_)))
    ));
    assert_invalid(parse_years(&page(&[])), "no year");
    assert_invalid(parse_checksums("\n"), "empty");
  }

  #[test]
  fn unexpected_formats() {
    assert_invalid(
      parse_years(&page(&["?prefix=data%2F2026%2F", "?prefix=data%2Fold%2F"])),
      "`data/old/`",
    );
    assert_invalid(
      parse_exports(&page(&[
        "?download=data%2F2026%2Fdiscogs_20261101_labels.xml.gz",
        "?download=data%2F2026%2Fdiscogs_20261101_labels.json.gz",
      ])),
      "`data/2026/discogs_20261101_labels.json.gz`",
    );
    assert_invalid(parse_checksums("0123 discogs_20261101_labels.xml.gz"), "`0123 ");
  }
}
