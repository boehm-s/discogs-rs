//! Deserializers for the loose formats of Discogs not covered by `serde_with`.

use std::fmt;
use std::marker::PhantomData;
use std::num::IntErrorKind;
use std::time::Duration;

use serde::de::{self, Unexpected, Visitor};
use serde::{Deserialize, Deserializer, Serializer};
use serde_with::ser::SerializeAsWrap;
use serde_with::{DeserializeAs, Same, SerializeAs};

/// Track duration written `m:ss` or `h:mm:ss`. Empty or unreadable durations → `None`.
pub(crate) mod track_duration {
  use super::{Deserialize as _, Deserializer, Duration, Serializer};

  pub(crate) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Duration>, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.and_then(|duration| parse(&duration)))
  }

  #[expect(clippy::ref_option, reason = "signature imposed by serde's `serialize_with`")]
  pub(crate) fn serialize<S: Serializer>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error> {
    let Some(duration) = duration else {
      return serializer.serialize_str("");
    };
    let seconds = duration.as_secs();
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
      serializer.collect_str(&format_args!("{hours}:{minutes:02}:{seconds:02}"))
    } else {
      serializer.collect_str(&format_args!("{minutes}:{seconds:02}"))
    }
  }

  fn parse(duration: &str) -> Option<Duration> {
    let mut seconds = 0;
    for part in duration.trim().split(':') {
      seconds = seconds * 60 + part.parse::<u64>().ok()?;
    }
    Some(Duration::from_secs(seconds))
  }

  #[cfg(test)]
  mod tests {
    use std::time::Duration;

    use super::parse;

    #[test]
    fn parse_durations() {
      assert_eq!(parse("7:21"), Some(Duration::from_secs(441)));
      assert_eq!(parse("1:02:03"), Some(Duration::from_secs(3723)));
      assert_eq!(parse(""), None);
      assert_eq!(parse("?"), None);
    }
  }
}

/// `0` → `None`, for years Discogs does not know.
pub(crate) struct ZeroAsNone;

impl<'de> DeserializeAs<'de, Option<u32>> for ZeroAsNone {
  fn deserialize_as<D: Deserializer<'de>>(deserializer: D) -> Result<Option<u32>, D::Error> {
    Ok(Option::<u32>::deserialize(deserializer)?.filter(|value| *value != 0))
  }
}

/// `u32` sent as a number or a string; `None` when it does not fit (some art releases claim
/// 10^188 files). Serialized through `F`.
pub(crate) struct OutOfRangeAsNone<F = Same>(PhantomData<F>);

impl<'de, F> DeserializeAs<'de, Option<u32>> for OutOfRangeAsNone<F> {
  fn deserialize_as<D: Deserializer<'de>>(deserializer: D) -> Result<Option<u32>, D::Error> {
    deserializer.deserialize_any(OutOfRangeVisitor)
  }
}

impl<F: SerializeAs<u32>> SerializeAs<Option<u32>> for OutOfRangeAsNone<F> {
  fn serialize_as<S: Serializer>(value: &Option<u32>, serializer: S) -> Result<S::Ok, S::Error> {
    match value {
      Some(value) => serializer.serialize_some(&SerializeAsWrap::<u32, F>::new(value)),
      None => serializer.serialize_none(),
    }
  }
}

struct OutOfRangeVisitor;

impl Visitor<'_> for OutOfRangeVisitor {
  type Value = Option<u32>;

  fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    formatter.write_str("a non-negative integer, as a number or a string")
  }

  fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
    Ok(u32::try_from(value).ok())
  }

  fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
    u64::try_from(value)
      .map_err(|_| E::invalid_value(Unexpected::Signed(value), &self))
      .and_then(|value| self.visit_u64(value))
  }

  /// JSON integers beyond `u64` arrive as floats.
  fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
    if value.fract() == 0.0 && value > f64::from(u32::MAX) {
      Ok(None)
    } else {
      Err(E::invalid_value(Unexpected::Float(value), &self))
    }
  }

  fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
    match value.parse::<u32>() {
      Ok(value) => Ok(Some(value)),
      Err(error) if *error.kind() == IntErrorKind::PosOverflow => Ok(None),
      Err(_) => Err(E::invalid_value(Unexpected::Str(value), &self)),
    }
  }

  fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
    Ok(None)
  }

  fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
    Ok(None)
  }
}

#[cfg(test)]
mod tests {
  use serde::{Deserialize, Serialize};
  use serde_with::{DisplayFromStr, serde_as};

  use super::OutOfRangeAsNone;

  #[serde_as]
  #[derive(Debug, PartialEq, Serialize, Deserialize)]
  struct Quantities {
    #[serde_as(as = "OutOfRangeAsNone<DisplayFromStr>")]
    text: Option<u32>,
    #[serde_as(as = "OutOfRangeAsNone")]
    number: Option<u32>,
  }

  fn quantities(json: &str) -> Result<(Option<u32>, Option<u32>), serde_json::Error> {
    serde_json::from_str::<Quantities>(json).map(|quantities| (quantities.text, quantities.number))
  }

  #[test]
  fn out_of_range_as_none() {
    assert_eq!(
      quantities(r#"{ "text": "2", "number": 2 }"#).unwrap(),
      (Some(2), Some(2))
    );
    assert_eq!(
      quantities(r#"{ "text": "5000000000", "number": 5000000000 }"#).unwrap(),
      (None, None)
    );
    let huge = "5".repeat(189);
    assert_eq!(
      quantities(&format!(r#"{{ "text": "{huge}", "number": {huge} }}"#)).unwrap(),
      (None, None)
    );
    assert_eq!(quantities(r#"{ "text": null, "number": null }"#).unwrap(), (None, None));
    assert!(quantities(r#"{ "text": "two", "number": 2 }"#).is_err());
    assert!(quantities(r#"{ "text": "2", "number": -2 }"#).is_err());
  }

  #[test]
  fn out_of_range_as_none_serializes_through_f() {
    let quantities = Quantities {
      text: Some(2),
      number: Some(2),
    };
    assert_eq!(
      serde_json::to_string(&quantities).unwrap(),
      r#"{"text":"2","number":2}"#
    );
  }
}
