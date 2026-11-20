//! Pull reader over quick-xml, reusing its buffers: the only allocations are the values returned.

use std::io::BufRead;
use std::str::FromStr;

use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::Event;
use quick_xml::events::attributes::Attributes;
use quick_xml::name::QName;

use crate::error::{DiscogsClientError, Result};
use crate::xml_exports::error::ExportError;

pub(crate) struct Xml<R> {
  reader: quick_xml::Reader<R>,
  buf: Vec<u8>,
  /// Content (name and attributes) of the last start tag read
  start: String,
  name_len: usize,
  /// Text of the last element read with `text_ref`
  text: String,
}

/// Element whose start tag was just read: its content must be consumed with `text`, `children`…
/// or `skip`.
#[derive(Clone, Copy)]
#[must_use]
pub(crate) struct Element {
  empty: bool,
}

impl<R: BufRead> Xml<R> {
  pub(crate) fn new(reader: R) -> Self {
    Self {
      reader: quick_xml::Reader::from_reader(reader),
      buf: Vec::with_capacity(64 * 1024),
      start: String::with_capacity(256),
      name_len: 0,
      text: String::with_capacity(4 * 1024),
    }
  }

  /// Name of the last element read
  pub(crate) fn name(&self) -> &[u8] {
    &self.start.as_bytes()[..self.name_len]
  }

  /// Attributes of the last element read, until the next read
  pub(crate) fn attributes(&self) -> Attributes<'_> {
    Attributes::new(&self.start, self.name_len)
  }

  /// Root element of the document (`<releases>`…), `None` for an empty document.
  pub(crate) fn root(&mut self) -> Result<Option<Element>> {
    loop {
      self.buf.clear();
      match self.reader.read_event_into(&mut self.buf)? {
        Event::Start(start) => {
          self.name_len = start.name().as_ref().len();
          self.start.clear();
          self.start.push_str(&start);
          return Ok(Some(Element { empty: false }));
        }
        Event::Empty(_) | Event::Eof => return Ok(None),
        // Declaration, comments, whitespace…
        _ => {}
      }
    }
  }

  /// Next child of `parent`, `None` once its end tag is read.
  pub(crate) fn next_child(&mut self, parent: Element) -> Result<Option<Element>> {
    if parent.empty {
      return Ok(None);
    }
    loop {
      self.buf.clear();
      match self.reader.read_event_into(&mut self.buf)? {
        Event::Start(start) => {
          self.name_len = start.name().as_ref().len();
          self.start.clear();
          self.start.push_str(&start);
          return Ok(Some(Element { empty: false }));
        }
        Event::Empty(start) => {
          self.name_len = start.name().as_ref().len();
          self.start.clear();
          self.start.push_str(&start);
          return Ok(Some(Element { empty: true }));
        }
        Event::End(_) => return Ok(None),
        Event::Eof => return Err(malformed("unexpected end of file".to_owned())),
        // Text between children, comments…
        _ => {}
      }
    }
  }

  /// Skips the content of the element whose start tag was just read.
  pub(crate) fn skip(&mut self, element: Element) -> Result<()> {
    if !element.empty {
      self.buf.clear();
      let name = QName(&self.start[..self.name_len]);
      self.reader.read_to_end_into(name, &mut self.buf)?;
    }
    Ok(())
  }

  /// Text of the element, borrowed until the next read. Nested elements are skipped.
  pub(crate) fn text_ref(&mut self, element: Element) -> Result<&str> {
    let mut text = std::mem::take(&mut self.text);
    text.clear();
    let result = self.append_text(element, &mut text);
    self.text = text;
    result.map(|()| self.text.as_str())
  }

  pub(crate) fn text(&mut self, element: Element) -> Result<String> {
    let mut text = String::new();
    self.append_text(element, &mut text)?;
    Ok(text)
  }

  fn append_text(&mut self, element: Element, text: &mut String) -> Result<()> {
    if element.empty {
      return Ok(());
    }
    let mut depth = 0_usize;
    loop {
      self.buf.clear();
      match self.reader.read_event_into(&mut self.buf)? {
        Event::Text(chunk) if depth == 0 => text.push_str(&chunk.xml10_content()),
        Event::CData(data) if depth == 0 => text.push_str(&data.xml10_content()),
        Event::GeneralRef(reference) if depth == 0 => match reference.resolve_char_ref()? {
          Some(char) => text.push(char),
          None => {
            let name: &str = &reference;
            let resolved =
              resolve_predefined_entity(name).ok_or_else(|| malformed(format!("unknown entity &{name};")))?;
            text.push_str(resolved);
          }
        },
        Event::Start(_) => depth += 1,
        Event::End(_) if depth == 0 => return Ok(()),
        Event::End(_) => depth -= 1,
        Event::Eof => return Err(malformed("unexpected end of file".to_owned())),
        _ => {}
      }
    }
  }

  /// Text of the element, `None` when empty.
  pub(crate) fn optional_text(&mut self, element: Element) -> Result<Option<String>> {
    let text = self.text(element)?;
    Ok((!text.is_empty()).then_some(text))
  }

  /// Parsed text of the element, `None` when empty or unreadable.
  pub(crate) fn parsed<T: FromStr>(&mut self, element: Element) -> Result<Option<T>> {
    Ok(self.text_ref(element)?.trim().parse().ok())
  }

  /// Texts of the children of the element (`<genres><genre>Rock</genre>…</genres>`).
  pub(crate) fn texts(&mut self, element: Element) -> Result<Vec<String>> {
    let mut texts = Vec::new();
    while let Some(child) = self.next_child(element)? {
      if let Some(text) = self.optional_text(child)? {
        texts.push(text);
      }
    }
    Ok(texts)
  }

  /// Parses each child of the element with `parse`, which must consume the child.
  pub(crate) fn list<T>(
    &mut self,
    element: Element,
    mut parse: impl FnMut(&mut Self, Element) -> Result<Option<T>>,
  ) -> Result<Vec<T>> {
    let mut items = Vec::new();
    while let Some(child) = self.next_child(element)? {
      if let Some(item) = parse(self, child)? {
        items.push(item);
      }
    }
    Ok(items)
  }
}

/// Error of a record, once fully read: the next records can still be read.
pub(crate) fn invalid(message: String) -> DiscogsClientError {
  ExportError::InvalidRecord(message).into()
}

/// Error of the document: the export cannot be read further.
pub(crate) fn malformed(message: String) -> DiscogsClientError {
  ExportError::InvalidExport(message).into()
}
