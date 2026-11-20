//! Cutting scanner: finds where each record (direct child of the root) starts, so that the cutter
//! can make blocks of complete records. It parses nothing: the workers parse the records of each
//! block with quick-xml.
//!
//! A hand-written lexer, about 2× faster than quick-xml for this (which builds an event for every
//! tag and text node), since the cutting thread bounds the speed of a parallel run. It tracks the
//! markup state from the first byte: comments, CDATA sections and processing instructions are
//! skipped, `>` inside quoted attribute values does not end a tag. DOCTYPE declarations (and their
//! internal subsets, which could define entities) are rejected.

use std::io::{self, Read};

use memchr::memmem;

use crate::error::{DiscogsClientError, Result};
use crate::xml_exports::error::ExportError;

/// Records found by the cutting scanner
pub(crate) struct RecordStarts {
  pub(crate) records: u64,
  /// Offset of the root closing tag: the records end there
  pub(crate) end: u64,
}

const READ_SIZE: usize = 1 << 20;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
  Comment,
  CData,
  ProcessingInstruction,
  /// `<!DOCTYPE`…: not consumed, the scan must stop
  Declaration,
  End,
  Start,
}

struct Markup<'a> {
  offset: u64,
  kind: Kind,
  /// Name of a start tag
  name: &'a [u8],
  self_closing: bool,
}

struct CuttingScanner<R> {
  reader: R,
  buf: Vec<u8>,
  /// Start of the unprocessed bytes in `buf`
  pos: usize,
  /// End of the valid bytes in `buf`
  len: usize,
  /// Offset of `buf[0]` in the input
  base: u64,
}

impl<R: Read> CuttingScanner<R> {
  fn new(reader: R) -> Self {
    Self {
      reader,
      buf: Vec::new(),
      pos: 0,
      len: 0,
      base: 0,
    }
  }

  fn offset(&self, index: usize) -> u64 {
    self.base + index as u64
  }

  /// Reads more input, keeping `buf[pos..len]`. `false` at the end of the input.
  fn read_more(&mut self) -> io::Result<bool> {
    if self.pos > 0 {
      self.buf.copy_within(self.pos..self.len, 0);
      self.base += self.pos as u64;
      self.len -= self.pos;
      self.pos = 0;
    }
    if self.buf.len() - self.len < READ_SIZE {
      self.buf.resize(self.len + READ_SIZE, 0);
    }
    loop {
      match self.reader.read(&mut self.buf[self.len..]) {
        Ok(read) => {
          self.len += read;
          return Ok(read > 0);
        }
        Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
        Err(error) => return Err(error),
      }
    }
  }

  /// Ensures `count` bytes from `pos`, unless the input ends before.
  fn lookahead(&mut self, count: usize) -> io::Result<&[u8]> {
    while self.len - self.pos < count && self.read_more()? {}
    Ok(&self.buf[self.pos..self.len])
  }

  /// Index (in `buf`) of the next `needle` at or after `pos + from`.
  fn find(&mut self, from: usize, needle: &[u8]) -> Result<usize> {
    let finder = memmem::Finder::new(needle);
    let mut from = from;
    loop {
      if let Some(index) = finder.find(&self.buf[self.pos + from..self.len]) {
        return Ok(self.pos + from + index);
      }
      from = (self.len - self.pos).saturating_sub(needle.len() - 1).max(from);
      if !self.read_more()? {
        return Err(truncated(self.offset(self.len)));
      }
    }
  }

  /// Index (in `buf`) of the next `byte` at or after `pos + from`.
  fn find_byte(&mut self, from: usize, byte: u8) -> Result<usize> {
    let mut from = from;
    loop {
      if let Some(index) = memchr::memchr(byte, &self.buf[self.pos + from..self.len]) {
        return Ok(self.pos + from + index);
      }
      from = self.len - self.pos;
      if !self.read_more()? {
        return Err(truncated(self.offset(self.len)));
      }
    }
  }

  /// Index (in `buf`) of the `>` ending the tag starting at `pos`, skipping quoted values.
  fn tag_end(&mut self) -> Result<usize> {
    let mut index = self.pos + 1;
    loop {
      let Some(found) = memchr::memchr3(b'>', b'"', b'\'', &self.buf[index..self.len]) else {
        if let Some(lt) = memchr::memchr(b'<', &self.buf[index..self.len]) {
          return Err(malformed(self.offset(index + lt), "`<` inside a tag"));
        }
        let relative = self.len - self.pos;
        if !self.read_more()? {
          return Err(truncated(self.offset(self.len)));
        }
        index = self.pos + relative;
        continue;
      };
      let found = index + found;
      if let Some(lt) = memchr::memchr(b'<', &self.buf[index..found]) {
        return Err(malformed(self.offset(index + lt), "`<` inside a tag"));
      }
      let byte = self.buf[found];
      if byte == b'>' {
        return Ok(found);
      }
      // Quoted value, which can contain `>`
      let close = self.find_byte(found + 1 - self.pos, byte)?;
      index = close + 1;
    }
  }

  /// Next markup, `None` at the end of the input.
  fn next_markup(&mut self) -> Result<Option<Markup<'_>>> {
    loop {
      if let Some(index) = memchr::memchr(b'<', &self.buf[self.pos..self.len]) {
        self.pos += index;
        break;
      }
      self.pos = self.len;
      if !self.read_more()? {
        return Ok(None);
      }
    }
    let offset = self.offset(self.pos);
    let head = self.lookahead(9)?;
    let kind = match head {
      [b'<', b'!', b'-', b'-', ..] => Kind::Comment,
      [b'<', b'!', b'[', b'C', b'D', b'A', b'T', b'A', b'[', ..] => Kind::CData,
      [b'<', b'?', ..] => Kind::ProcessingInstruction,
      [b'<', b'!', ..] => Kind::Declaration,
      [b'<', b'/', ..] => Kind::End,
      [_, _, ..] => Kind::Start,
      _ => return Err(truncated(offset)),
    };
    let mut markup = Markup {
      offset,
      kind,
      name: &[],
      self_closing: false,
    };
    match kind {
      Kind::Comment => self.pos = self.find(4, b"-->")? + 3,
      Kind::CData => self.pos = self.find(9, b"]]>")? + 3,
      Kind::ProcessingInstruction => self.pos = self.find(2, b"?>")? + 2,
      Kind::Declaration => {}
      Kind::End => self.pos = self.find_byte(2, b'>')? + 1,
      Kind::Start => {
        let tag_end = self.tag_end()?;
        let tag = &self.buf[self.pos + 1..tag_end];
        let name_len = tag
          .iter()
          .position(|byte| byte.is_ascii_whitespace() || *byte == b'/')
          .unwrap_or(tag.len());
        self.pos = tag_end + 1;
        markup.name = &tag[..name_len];
        markup.self_closing = tag.last() == Some(&b'/');
      }
    }
    Ok(Some(markup))
  }
}

/// Calls `on_record` with the offset of each record, in order.
pub(crate) fn find_record_starts(
  reader: impl Read,
  root: &[u8],
  record: &[u8],
  max_record_bytes: u64,
  mut on_record: impl FnMut(u64),
) -> Result<RecordStarts> {
  let mut scanner = CuttingScanner::new(reader);
  let mut depth = 0_usize;
  let mut root_seen = false;
  let mut end = None;
  let mut records = 0_u64;
  let mut record_start = 0_u64;
  while let Some(markup) = scanner.next_markup()? {
    let start = markup.offset;
    match markup.kind {
      Kind::Comment | Kind::ProcessingInstruction => {}
      Kind::CData if depth == 0 => return Err(malformed(start, "CDATA outside of the root")),
      Kind::CData => {}
      Kind::Declaration => return Err(unsupported(start, "DOCTYPE and other declarations")),
      Kind::End => {
        if depth == 0 {
          return Err(malformed(start, "closing tag without opening tag"));
        }
        depth -= 1;
        if depth == 1 && scanner.offset(scanner.pos) - record_start > max_record_bytes {
          return Err(oversized(record_start, max_record_bytes));
        }
        if depth == 0 {
          end = Some(start);
        }
      }
      Kind::Start => {
        let (name, self_closing) = (markup.name, markup.self_closing);
        match depth {
          0 if root_seen => return Err(malformed(start, "second root element")),
          0 if name != root => {
            return Err(malformed(
              start,
              &format!("root `{}` instead of `{}`", lossy(name), lossy(root)),
            ));
          }
          0 => {
            root_seen = true;
            if self_closing {
              end = Some(start);
            }
          }
          1 if name != record => {
            return Err(malformed(
              start,
              &format!("record `{}` instead of `{}`", lossy(name), lossy(record)),
            ));
          }
          1 => {
            records += 1;
            record_start = start;
            on_record(start);
          }
          _ => {}
        }
        if !self_closing {
          depth += 1;
        }
      }
    }
  }
  match end {
    Some(end) if depth == 0 => Ok(RecordStarts { records, end }),
    _ if !root_seen => Ok(RecordStarts { records, end: 0 }),
    _ => Err(truncated(scanner.offset(scanner.len))),
  }
}

fn lossy(bytes: &[u8]) -> String {
  String::from_utf8_lossy(bytes).into_owned()
}

fn malformed(offset: u64, message: &str) -> DiscogsClientError {
  ExportError::InvalidExport(format!("{message} at byte {offset}")).into()
}

fn unsupported(offset: u64, construct: &str) -> DiscogsClientError {
  ExportError::InvalidExport(format!("unsupported XML construct ({construct}) at byte {offset}")).into()
}

fn truncated(offset: u64) -> DiscogsClientError {
  ExportError::InvalidExport(format!("truncated export: unexpected end at byte {offset}")).into()
}

fn oversized(offset: u64, max_record_bytes: u64) -> DiscogsClientError {
  ExportError::InvalidExport(format!("record at byte {offset} larger than {max_record_bytes} bytes")).into()
}

#[cfg(test)]
mod tests {
  use super::find_record_starts;
  use crate::error::DiscogsClientError;

  const TRICKY: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!-- <master id="999"> fake record -->
<masters>
<master id="1"><title>Ωé</title><notes><![CDATA[ <master id="998"></master> </masters> ]]></notes></master>
<!-- </master><master id="997"> </masters> <masters> -->
<?pi <master id="996">?>
<master id="2"><title>B &amp; &lt;master id="995"&gt;</title><video src="https://example.com/?a=1&amp;b=>" a='"<'/></master>
<master id = "3" ><sub><master id="5"><master/></master></sub></master>
<master id="4"/>
</masters>
"#;

  fn offsets(content: &str, max_record_bytes: u64) -> Result<(Vec<u64>, u64), DiscogsClientError> {
    let mut offsets = Vec::new();
    let scan = find_record_starts(content.as_bytes(), b"masters", b"master", max_record_bytes, |offset| {
      offsets.push(offset)
    })?;
    assert_eq!(scan.records, offsets.len() as u64);
    Ok((offsets, scan.end))
  }

  #[test]
  fn records_are_the_children_of_the_root() {
    let (offsets, end) = offsets(TRICKY, 1 << 20).unwrap();
    let expected: Vec<u64> = [
      "<master id=\"1\">",
      "<master id=\"2\">",
      "<master id = ",
      "<master id=\"4\"/>",
    ]
    .iter()
    .map(|tag| TRICKY.find(tag).unwrap() as u64)
    .collect();
    assert_eq!(offsets, expected);
    assert_eq!(end, TRICKY.rfind("</masters>").unwrap() as u64);
  }

  #[test]
  fn invalid_exports() {
    for content in [
      &TRICKY[..TRICKY.len() - 5],
      "<!DOCTYPE masters><masters><master id=\"1\"/></masters>",
      "<masters><master id=\"1\"/></masters><masters><master id=\"2\"/></masters>",
      "<masters><master id=\"1\"/><release id=\"2\"/></masters>",
      "<masters><master id=\"1\"/><master id=\"2\"><master id=\"3\"/></masters>",
      "<masters><master id=\"1\"/></master><master id=\"3\"/></masters>",
    ] {
      assert!(offsets(content, 1 << 20).is_err(), "{content}");
    }
    assert!(offsets(TRICKY, 50).is_err());
  }
}
