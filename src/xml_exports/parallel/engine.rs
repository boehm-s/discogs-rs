//! Threads of a parallel run, and how they hand the work over.
//!
//! ```text
//!               ┌──────────────┐         ┌──────────────┐  blocks queue:     ┌──────────┐
//!  file, HTTP ─▶│ input thread │ chunks  │ cutting      │  at most 2         │ worker 1 │─▶ f(record)
//!               │ reads,       │────────▶│ thread       │═══════════════════▶│ worker 2 │─▶ f(record)
//!               │ gunzips      │ (1 MiB) │ (`cutter`)   │  sync_channel      │   …      │
//!               └──────────────┘         └──────────────┘                    │ worker n │─▶ f(record)
//!                                               ▲                            └──────────┘
//!                                               │   emptied block buffers         │
//!                                               └─────────────────────────────────┘
//!                                                       recycle channel
//! ```
//!
//! - **Adaptive workers**: the run starts with one worker. When the cutting thread finds the
//!   queue full, the workers are behind: it starts another one, up to `max_workers`, then waits
//!   for room in the queue. The pool thus grows to what the input thread delivers, about 4 to 6
//!   workers on the releases export, more when `f` is slow, and never starts idle threads.
//! - **Blocks queue** (`sync_channel(2)`): bounded, so the cutting thread waits when the workers
//!   are behind at their maximum. Memory stays bounded: a block per worker, two waiting, and the
//!   block being filled.
//! - **Shared receiver** (`Mutex<Receiver>`): a std channel has a single receiver; the workers
//!   take turns on the mutex to pull the next block, whichever worker is free first. The mutex is
//!   held only while waiting for a block, never while parsing one.
//! - **Recycle channel**: a parsed block's buffer goes back to the cutting thread, which refills
//!   it instead of allocating a new `block_bytes` buffer for every block. Buffers stay allocated and
//!   freed by the same thread.
//! - **Stop flag**: the first error (from `f` or the export) sets it, as does dropping the future
//!   of the processing. Workers then drop their blocks unparsed, and the cutting thread's reads
//!   fail, which ends the scan.
//! - **Record count**: the workers count the records they parse, the cutting thread counts those
//!   it finds. A difference means a block was parsed wrongly, and is an error.

use std::io::{BufReader, Cursor, Read};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvError, TrySendError, channel, sync_channel};
use std::sync::{Mutex, PoisonError};
use std::thread;

use crate::error::{DiscogsClientError, Result};
use crate::xml_exports::error::ExportError;
use crate::xml_exports::model::ExportRecord;
use crate::xml_exports::parallel::ParallelOptions;
use crate::xml_exports::parallel::cutter::{Block, cut_blocks};
use crate::xml_exports::parser::ExportReader;
use crate::xml_exports::parser::input::BUFFER_SIZE;

/// Blocks waiting for a worker. Small, so that a full queue soon shows the workers are behind.
const QUEUED_BLOCKS: usize = 2;

/// Synthetic root wrapping each block, so that it parses as a document
const BLOCK_PREFIX: &[u8] = b"<block>";
const BLOCK_SUFFIX: &[u8] = b"</block>";

/// Calls `f` from the workers with each record of the decompressed `input`, cut on the calling
/// thread, and returns the number of records. The first error, returned by `f` or met in the
/// export, stops the run.
pub(crate) fn run<T: ExportRecord, F>(
  input: impl Read,
  options: &ParallelOptions,
  stop: &AtomicBool,
  f: &F,
) -> Result<u64>
where
  F: Fn(Result<T>) -> Result<()> + Sync,
{
  options.validate()?;
  let (blocks, received) = sync_channel::<Block>(QUEUED_BLOCKS);
  let received = Mutex::new(received);
  let (recycle, recycled) = channel::<Vec<u8>>();
  let records = AtomicU64::new(0);

  let (cut, results) = thread::scope(|scope| {
    let worker = || {
      let (received, recycle, records) = (&received, recycle.clone(), &records);
      move || {
        let mut result = Ok(());
        while let Ok(block) = next_block(received) {
          if result.is_ok() && !stop.load(Ordering::Relaxed) {
            result = parse_block(&block, stop, records, f);
            if result.is_err() {
              stop.store(true, Ordering::Relaxed);
            }
          }
          let _ = recycle.send(block.bytes);
        }
        result
      }
    };
    let mut workers = vec![scope.spawn(worker())];
    // A full queue means the workers are behind: one more, up to the maximum
    let mut deliver = |block| match blocks.try_send(block) {
      Ok(()) | Err(TrySendError::Disconnected(_)) => {}
      Err(TrySendError::Full(block)) => {
        if workers.len() < options.max_workers {
          workers.push(scope.spawn(worker()));
        }
        // Fails once the workers are gone: the scan then stops on `stop`
        let _ = blocks.send(block);
      }
    };
    let cut = cut_blocks::<T>(input, options, &mut deliver, &recycled, stop);
    // Closes the queue: the workers end once it is empty
    drop(blocks);
    if cut.is_err() {
      stop.store(true, Ordering::Relaxed);
    }
    let results: Vec<_> = workers.into_iter().map(thread::ScopedJoinHandle::join).collect();
    (cut, results)
  });

  let mut panicked = 0;
  for result in results {
    match result {
      Ok(Ok(())) => {}
      Ok(Err(error)) => return Err(error),
      Err(_) => panicked += 1,
    }
  }
  if panicked > 0 {
    return Err(ExportError::InvalidExport(format!("{panicked} parsing worker(s) panicked")).into());
  }
  let found = cut?;
  let parsed = records.into_inner();
  if parsed != found {
    return Err(ExportError::InvalidExport(format!("{parsed} records parsed, {found} found in the export")).into());
  }
  Ok(parsed)
}

fn next_block(received: &Mutex<Receiver<Block>>) -> std::result::Result<Block, RecvError> {
  received.lock().unwrap_or_else(PoisonError::into_inner).recv()
}

/// Calls `f` with the records of a block, in file order.
fn parse_block<T: ExportRecord, F>(block: &Block, stop: &AtomicBool, records: &AtomicU64, f: &F) -> Result<()>
where
  F: Fn(Result<T>) -> Result<()>,
{
  // Positions in the export, as the block's are meaningless to users
  let context = |error| {
    let end = block.start + block.bytes.len() as u64;
    let context = format!("records [{}, {end})", block.start);
    match error {
      DiscogsClientError::Export(ExportError::InvalidRecord(message)) => {
        ExportError::InvalidRecord(format!("{context}: {message}")).into()
      }
      error => ExportError::InvalidExport(format!("{context}: {error}")).into(),
    }
  };
  let input = Cursor::new(BLOCK_PREFIX)
    .chain(block.bytes.as_slice())
    .chain(Cursor::new(BLOCK_SUFFIX));
  for record in ExportReader::<T, _>::new(BufReader::with_capacity(BUFFER_SIZE, input)) {
    if stop.load(Ordering::Relaxed) {
      return Ok(());
    }
    let record = match record {
      Ok(record) => Ok(record),
      Err(error @ DiscogsClientError::Export(ExportError::InvalidRecord(_))) => Err(context(error)),
      Err(error) => return Err(context(error)),
    };
    records.fetch_add(1, Ordering::Relaxed);
    f(record)?;
  }
  Ok(())
}
