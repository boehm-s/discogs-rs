//! Reading of the export on its own thread, decompressed when gzipped and verified when its
//! checksum is known: parsing never waits for a read or for the decompression.

use std::cell::RefCell;
use std::io::{self, BufRead, BufReader, Read};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, sync_channel};
use std::thread;

use flate2::bufread::MultiGzDecoder;
use sha2::{Digest as _, Sha256};

use crate::error::Result;
use crate::xml_exports::error::ExportError;
use crate::xml_exports::service::download::{hex, verify};

pub(crate) const BUFFER_SIZE: usize = 1 << 20;

/// Chunks in flight between the threads: bounds the memory used
const CHUNKS_IN_FLIGHT: usize = 4;

const GZIP_MAGIC: [u8; 2] = [0x1f, 0x8b];

/// Expected SHA-256 of a file
#[derive(Clone, Debug)]
pub(crate) struct Checksum {
  pub(crate) file: String,
  pub(crate) sha256: String,
}

/// Decompressed XML, read by a thread. The thread stops when this reader is dropped.
pub(crate) struct Input {
  chunks: Receiver<io::Result<Vec<u8>>>,
  /// Result of the checksum verification (`Ok` without checksum), sent once the whole file was
  /// read
  verified: Receiver<std::result::Result<(), ExportError>>,
  chunk: Vec<u8>,
  position: usize,
  /// The empty chunk marking the end was received
  ended: bool,
}

impl Input {
  /// Reads `source` on a new thread, gunzipping it when gzipped. The checksum is verified
  /// once the whole file was read: see [`finish`](Self::finish).
  pub(crate) fn spawn(source: impl Read + Send + 'static, checksum: Option<Checksum>) -> io::Result<Self> {
    let (sender, chunks) = sync_channel(CHUNKS_IN_FLIGHT);
    let (verdict, verified) = sync_channel(1);
    thread::Builder::new()
      .name("discogs-export-input".to_owned())
      .spawn(move || {
        let hasher = checksum.as_ref().map(|_| Rc::new(RefCell::new(Sha256::new())));
        let hashed = Hashed {
          source,
          hasher: hasher.clone(),
        };
        let mut decompressed = match decompressed(hashed) {
          Ok(decompressed) => decompressed,
          Err(error) => {
            let _ = sender.send(Err(error));
            return;
          }
        };
        loop {
          let chunk = read_chunk(&mut decompressed);
          let end = !matches!(&chunk, Ok(chunk) if !chunk.is_empty());
          // The end of the decompressed file is the end of the file
          if let (Ok(_), true) = (&chunk, end) {
            let verified = match (&checksum, &hasher) {
              (Some(checksum), Some(hasher)) => {
                verify(&checksum.file, Some(&checksum.sha256), &hex(&hasher.take().finalize()))
              }
              _ => Ok(()),
            };
            let _ = verdict.send(verified);
          }
          if sender.send(chunk).is_err() || end {
            return;
          }
        }
      })?;
    Ok(Self {
      chunks,
      verified,
      chunk: Vec::new(),
      position: 0,
      ended: false,
    })
  }

  /// Reads to the end of the file, and returns the result of its checksum verification: a
  /// [`ChecksumMismatch`](ExportError::ChecksumMismatch) when the file differs from the one
  /// published.
  pub(crate) fn finish(mut self) -> Result<()> {
    io::copy(&mut self, &mut io::sink())?;
    // Sent before the end chunk, which was received
    self.verified.recv().map_err(|_| stopped())??;
    Ok(())
  }
}

/// The reading thread stopped before the end of the file, without error: it panicked.
fn stopped() -> ExportError {
  ExportError::InvalidExport("the reading thread stopped before the end of the file".to_owned())
}

/// Hashes the bytes read
struct Hashed<R> {
  source: R,
  hasher: Option<Rc<RefCell<Sha256>>>,
}

impl<R: Read> Read for Hashed<R> {
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    let read = self.source.read(buf)?;
    if let Some(hasher) = &self.hasher {
      hasher.borrow_mut().update(&buf[..read]);
    }
    Ok(read)
  }
}

/// Gunzipped when it starts with the gzip magic number
fn decompressed(source: impl Read + 'static) -> io::Result<Box<dyn Read>> {
  let mut source = BufReader::with_capacity(BUFFER_SIZE, source);
  if source.fill_buf()?.starts_with(&GZIP_MAGIC) {
    Ok(Box::new(MultiGzDecoder::new(source)))
  } else {
    Ok(Box::new(source))
  }
}

/// Fills a chunk, shorter only at the end of the input.
fn read_chunk(reader: &mut impl Read) -> io::Result<Vec<u8>> {
  let mut chunk = vec![0; BUFFER_SIZE];
  let mut length = 0;
  while length < chunk.len() {
    match reader.read(&mut chunk[length..]) {
      Ok(0) => break,
      Ok(read) => length += read,
      Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
      Err(error) => return Err(error),
    }
  }
  chunk.truncate(length);
  Ok(chunk)
}

impl Read for Input {
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    let available = self.fill_buf()?;
    let amount = available.len().min(buf.len());
    buf[..amount].copy_from_slice(&available[..amount]);
    self.consume(amount);
    Ok(amount)
  }
}

impl BufRead for Input {
  fn fill_buf(&mut self) -> io::Result<&[u8]> {
    if self.position == self.chunk.len() && !self.ended {
      self.chunk = self.chunks.recv().map_err(|_| io::Error::other(stopped()))??;
      self.position = 0;
      self.ended = self.chunk.is_empty();
    }
    Ok(&self.chunk[self.position..])
  }

  fn consume(&mut self, amount: usize) {
    self.position = (self.position + amount).min(self.chunk.len());
  }
}

#[cfg(test)]
mod tests {
  use std::io::{Cursor, Read as _};

  use sha2::{Digest as _, Sha256};

  use super::{Checksum, Input};
  use crate::error::{DiscogsClientError, Result};
  use crate::xml_exports::error::ExportError;
  use crate::xml_exports::parser::tests::{MASTERS, gzip};
  use crate::xml_exports::service::download::hex;

  fn read(input: Vec<u8>) -> Result<String> {
    let mut text = String::new();
    Input::spawn(Cursor::new(input), None)?.read_to_string(&mut text)?;
    Ok(text)
  }

  #[test]
  fn plain_and_gzipped() {
    assert_eq!(read(MASTERS.as_bytes().to_vec()).unwrap(), MASTERS);
    assert_eq!(read(gzip(MASTERS)).unwrap(), MASTERS);
    assert_eq!(read(Vec::new()).unwrap(), "");
  }

  #[test]
  fn checksum_verified_at_the_end() {
    let gzipped = gzip(MASTERS);
    let checksum = |sha256: String| Checksum {
      file: "masters.xml.gz".to_owned(),
      sha256,
    };
    let right = checksum(hex(&Sha256::digest(&gzipped)));
    Input::spawn(Cursor::new(gzipped.clone()), Some(right))
      .unwrap()
      .finish()
      .unwrap();

    let mut input = Input::spawn(Cursor::new(gzipped), Some(checksum("0123".to_owned()))).unwrap();
    let mut head = vec![0; MASTERS.len()];
    input.read_exact(&mut head).unwrap();
    assert!(matches!(
      input.finish(),
      Err(DiscogsClientError::Export(ExportError::ChecksumMismatch { expected, .. })) if expected == "0123"
    ));
  }
}
