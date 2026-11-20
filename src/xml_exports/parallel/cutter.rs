//! Cutting of the decompressed export into blocks of complete records, on one thread.
//!
//! The cutting scanner reads the export through a [`Tee`], which copies every byte read into
//! `pending`. Each time it meets the start of a record, everything before it in
//! `pending` is complete records: once that is at least `block_bytes`, it is cut there and sent to
//! the workers.
//!
//! ```text
//!  decompressed ──▶ Tee ──▶ cutting scanner ── on_record(offset) for each `<release` under the root
//!  export            │
//!                    └──▶ pending.bytes (copy of everything read, not sent yet)
//!
//!  pending.bytes, when on_record(offset) is called:
//!
//!   pending.start                               offset            read so far
//!   │                                           │                 │
//!   ▼                                           ▼                 ▼
//!   ┌───────────┬───────────┬─────┬───────────┬─────────────────────┐
//!   │ <release> │ <release> │  …  │ <release> │ <release> … (partial)│
//!   └───────────┴───────────┴─────┴───────────┴─────────────────────┘
//!   ╰──────── complete records: ≥ block_bytes? ╯╰── rest ────────────╯
//!              │ yes                              │
//!              ▼                                  ▼
//!   Block { start, bytes } ──▶ workers     new pending.bytes (a recycled buffer),
//!                                          pending.start = offset
//! ```
//!
//! - Before the first record, `pending` holds the XML declaration and the root's start tag: they
//!   are dropped, the workers wrap each block in a synthetic root instead.
//! - After the last record, the cutting scanner reaches the root's end tag: the records before it are the
//!   last block.
//! - Blocks are only cut at record starts, so a record larger than `block_bytes` makes a larger
//!   block: it is never split.

use std::cell::RefCell;
use std::io::{self, Read};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;

use crate::error::Result;
use crate::xml_exports::model::ExportRecord;
use crate::xml_exports::parallel::ParallelOptions;
use crate::xml_exports::parallel::cutting_scanner::find_record_starts;
use crate::xml_exports::parser::input::BUFFER_SIZE;

/// Complete records, from the offset `start` of the decompressed export
pub(super) struct Block {
  pub(super) start: u64,
  pub(super) bytes: Vec<u8>,
}

/// Decompressed bytes not sent to the workers yet
struct Pending {
  bytes: Vec<u8>,
  /// Offset of `bytes[0]` in the decompressed export
  start: u64,
  /// Offset of the first record, once met
  first: Option<u64>,
}

/// Copies the bytes read into `pending`, and fails once the run is stopped.
struct Tee<'a, R> {
  reader: R,
  pending: Rc<RefCell<Pending>>,
  stop: &'a AtomicBool,
}

impl<R: Read> Read for Tee<'_, R> {
  fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
    if self.stop.load(Ordering::Relaxed) {
      return Err(io::Error::other("stopped"));
    }
    let read = self.reader.read(buf)?;
    self.pending.borrow_mut().bytes.extend_from_slice(&buf[..read]);
    Ok(read)
  }
}

/// Sends blocks of complete records to the workers, until the end of `input`. Returns the number
/// of records found.
pub(super) fn cut_blocks<T: ExportRecord>(
  input: impl Read,
  options: &ParallelOptions,
  deliver: &mut impl FnMut(Block),
  recycled: &Receiver<Vec<u8>>,
  stop: &AtomicBool,
) -> Result<u64> {
  let capacity = options.block_bytes + BUFFER_SIZE;
  let pending = Rc::new(RefCell::new(Pending {
    bytes: Vec::with_capacity(capacity),
    start: 0,
    first: None,
  }));
  let mut tee = Tee {
    reader: input,
    pending: Rc::clone(&pending),
    stop,
  };
  // Sends the records before `end`, keeps the bytes after it in `pending`
  let mut send = |pending: &mut Pending, end: u64| {
    let length = usize::try_from(end - pending.start).unwrap_or(usize::MAX);
    let mut rest = recycled.try_recv().unwrap_or_default();
    rest.clear();
    rest.reserve(capacity);
    rest.extend_from_slice(&pending.bytes[length..]);
    let mut bytes = std::mem::replace(&mut pending.bytes, rest);
    bytes.truncate(length);
    let block = Block {
      start: pending.start,
      bytes,
    };
    pending.start = end;
    deliver(block);
  };
  let block_bytes = options.block_bytes as u64;
  let scan = find_record_starts(
    &mut tee,
    T::KIND.as_str().as_bytes(),
    T::TAG,
    options.max_record_bytes,
    |offset| {
      let mut pending = pending.borrow_mut();
      if pending.first.is_none() {
        let before = usize::try_from(offset - pending.start).unwrap_or(usize::MAX);
        pending.bytes.drain(..before);
        pending.start = offset;
        pending.first = Some(offset);
      } else if offset - pending.start >= block_bytes {
        send(&mut pending, offset);
      }
    },
  );
  let scan = match scan {
    Ok(scan) => scan,
    // Stopped by a worker, whose error is returned instead
    Err(_) if stop.load(Ordering::Relaxed) => return Ok(0),
    Err(error) => return Err(error),
  };
  if pending.borrow().first.is_some() {
    send(&mut pending.borrow_mut(), scan.end);
  }
  Ok(scan.records)
}
