//! Pagination of the list endpoints.

use std::fmt;
use std::marker::PhantomData;

use serde::de::{self, IgnoredAny, MapAccess, Visitor};
use serde::ser::SerializeMap as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Page to request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pagination {
  /// Starts at 1.
  pub page: u32,
  /// At most 100.
  #[serde(rename = "per_page")]
  pub number_of_items_per_page: u32,
}

impl Default for Pagination {
  /// Discogs' defaults.
  fn default() -> Self {
    Self {
      page: 1,
      number_of_items_per_page: 50,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PaginationDetails {
  pub page: u32,
  #[serde(rename = "pages")]
  pub pages_count: u32,
  #[serde(rename = "items")]
  pub items_count: u32,
  #[serde(rename = "per_page")]
  pub number_of_items_per_page: u32,
}

/// Entity listed by a paginated endpoint.
pub trait ApiResource {
  /// Key of the list in the response, e.g. `versions` for `/masters/{id}/versions`.
  const COLLECTION_KEY: &'static str;
}

/// Page of a list endpoint: `{ "pagination": {...}, "<T::COLLECTION_KEY>": [...] }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaginatedResponse<T> {
  pub pagination: PaginationDetails,
  pub results: Vec<T>,
}

impl<T: ApiResource + Serialize> Serialize for PaginatedResponse<T> {
  fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(Some(2))?;
    map.serialize_entry("pagination", &self.pagination)?;
    map.serialize_entry(T::COLLECTION_KEY, &self.results)?;
    map.end()
  }
}

impl<'de, T: ApiResource + Deserialize<'de>> Deserialize<'de> for PaginatedResponse<T> {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    deserializer.deserialize_map(PaginatedResponseVisitor(PhantomData))
  }
}

/// Serde can only derive fixed field names: the list key is only known through `T`.
struct PaginatedResponseVisitor<T>(PhantomData<T>);

impl<'de, T: ApiResource + Deserialize<'de>> Visitor<'de> for PaginatedResponseVisitor<T> {
  type Value = PaginatedResponse<T>;

  fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(formatter, "a page of `{}`", T::COLLECTION_KEY)
  }

  fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
    let mut pagination = None;
    let mut results = None;
    while let Some(key) = map.next_key::<String>()? {
      match key.as_str() {
        "pagination" => pagination = Some(map.next_value()?),
        key if key == T::COLLECTION_KEY => results = Some(map.next_value()?),
        _ => {
          map.next_value::<IgnoredAny>()?;
        }
      }
    }
    Ok(PaginatedResponse {
      pagination: pagination.ok_or_else(|| de::Error::missing_field("pagination"))?,
      results: results.ok_or_else(|| de::Error::missing_field(T::COLLECTION_KEY))?,
    })
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SortOrder {
  Asc,
  Desc,
}

#[cfg(test)]
mod tests {
  use serde::{Deserialize, Serialize};

  use super::{ApiResource, PaginatedResponse};

  #[derive(Debug, PartialEq, Serialize, Deserialize)]
  struct Entry {
    id: u32,
  }

  impl ApiResource for Entry {
    const COLLECTION_KEY: &'static str = "entries";
  }

  const PAGE: &str = r#"{
    "pagination": { "page": 2, "pages": 3, "per_page": 1, "items": 3, "urls": {} },
    "entries": [{ "id": 7 }]
  }"#;

  #[test]
  fn deserialize_reads_the_collection_key() {
    let page: PaginatedResponse<Entry> = serde_json::from_str(PAGE).unwrap();
    assert_eq!(page.results, [Entry { id: 7 }]);
    assert_eq!((page.pagination.page, page.pagination.items_count), (2, 3));
  }

  #[test]
  fn serde_round_trip() {
    let page: PaginatedResponse<Entry> = serde_json::from_str(PAGE).unwrap();
    let json = serde_json::to_string(&page).unwrap();
    assert_eq!(serde_json::from_str::<PaginatedResponse<Entry>>(&json).unwrap(), page);
  }

  #[test]
  fn missing_collection_is_an_error() {
    let error = serde_json::from_str::<PaginatedResponse<Entry>>(
      r#"{ "pagination": { "page": 1, "pages": 1, "per_page": 1, "items": 0 }, "versions": [] }"#,
    )
    .unwrap_err();
    assert!(error.to_string().contains("missing field `entries`"), "{error}");
  }
}
