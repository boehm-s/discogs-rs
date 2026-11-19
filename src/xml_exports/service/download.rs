//! Downloads of export files: streamed to the parser, or saved to disk.

use std::io::{self, BufRead, Read};
use std::path::Path;

use bytes::Bytes;
use futures_util::StreamExt as _;
use sha2::{Digest as _, Sha256};
use tokio::io::AsyncWriteExt as _;
use tokio::sync::mpsc;
use tracing::instrument;
use url::Url;

use crate::client::Client;
use crate::error::Result;
use crate::xml_exports::error::ExportError;

/// Network chunks waiting for the reading thread
const CHUNKS_IN_FLIGHT: usize = 64;

/// Starts the download of `url`, read as it arrives with the returned blocking reader.
/// Dropping the reader stops the download.
#[instrument(skip(client))]
pub(crate) async fn open_remote(client: &Client, url: &Url) -> Result<RemoteReader> {
  let response = Client::send(client.file_request(url.clone())).await?;
  let (sender, chunks) = mpsc::channel(CHUNKS_IN_FLIGHT);
  tokio::spawn(async move {
    let mut body = response.bytes_stream();
    while let Some(chunk) = body.next().await {
      let chunk = chunk.map_err(io::Error::other);
      let failed = chunk.is_err();
      if sender.send(chunk).await.is_err() || failed {
        return;
      }
    }
  });
  Ok(RemoteReader {
    chunks,
    chunk: Bytes::new(),
  })
}

/// Downloads `url` to `destination`, through a `.part` file renamed once complete and verified.
#[instrument(skip(client))]
pub(crate) async fn download_file(
  client: &Client,
  url: &Url,
  destination: &Path,
  expected_sha256: Option<&str>,
) -> Result<()> {
  let response = Client::send(client.file_request(url.clone())).await?;
  let mut partial = destination.as_os_str().to_owned();
  partial.push(".part");
  let mut output = tokio::fs::File::create(&partial).await?;
  let mut hasher = Sha256::new();
  let mut body = response.bytes_stream();
  while let Some(chunk) = body.next().await {
    let chunk = chunk?;
    hasher.update(&chunk);
    output.write_all(&chunk).await?;
  }
  output.flush().await?;
  drop(output);

  let name = destination.file_name().unwrap_or_default().to_string_lossy();
  if let Err(error) = verify(&name, expected_sha256, &hex(&hasher.finalize())) {
    tokio::fs::remove_file(&partial).await?;
    return Err(error.into());
  }
  tokio::fs::rename(&partial, destination).await?;
  Ok(())
}

pub(crate) fn verify(file: &str, expected: Option<&str>, actual: &str) -> std::result::Result<(), ExportError> {
  match expected {
    Some(expected) if expected != actual => Err(ExportError::ChecksumMismatch {
      file: file.to_owned(),
      expected: expected.to_owned(),
      actual: actual.to_owned(),
    }),
    _ => Ok(()),
  }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
  use std::fmt::Write as _;

  bytes
    .iter()
    .fold(String::with_capacity(bytes.len() * 2), |mut hex, byte| {
      let _ = write!(hex, "{byte:02x}");
      hex
    })
}

/// Blocking reader over the chunks received by the download task.
pub(crate) struct RemoteReader {
  chunks: mpsc::Receiver<io::Result<Bytes>>,
  chunk: Bytes,
}

impl Read for RemoteReader {
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    let available = self.fill_buf()?;
    let amount = available.len().min(buf.len());
    buf[..amount].copy_from_slice(&available[..amount]);
    self.consume(amount);
    Ok(amount)
  }
}

impl BufRead for RemoteReader {
  fn fill_buf(&mut self) -> io::Result<&[u8]> {
    while self.chunk.is_empty() {
      match self.chunks.blocking_recv() {
        Some(chunk) => self.chunk = chunk?,
        None => break,
      }
    }
    Ok(&self.chunk)
  }

  fn consume(&mut self, amount: usize) {
    let _ = self.chunk.split_to(amount.min(self.chunk.len()));
  }
}
