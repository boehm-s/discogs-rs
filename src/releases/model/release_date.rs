use std::convert::Infallible;
use std::fmt::{Display, Formatter};
use std::ops::RangeInclusive;
use std::str::FromStr;

use chrono::{Datelike as _, Months, NaiveDate};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_with::DeserializeAs;

/// Release date, as the range of possible days: `1969-10-10..=1969-10-10` for a known day. Discogs
/// writes unknown months and days as `00` (`1969-11-00`), or only gives the year.
///
/// Serialized in ISO 8601: `1969-10-10`, `1969-11`, `1969`, or an interval `1969-11-01/1970-02-28`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReleaseDate {
  Known(RangeInclusive<NaiveDate>),
  /// Not a date Discogs usually writes, kept as is.
  Unknown(String),
}

impl ReleaseDate {
  fn year(year: i32) -> Option<RangeInclusive<NaiveDate>> {
    Some(NaiveDate::from_ymd_opt(year, 1, 1)?..=NaiveDate::from_ymd_opt(year, 12, 31)?)
  }

  fn month(year: i32, month: u32) -> Option<RangeInclusive<NaiveDate>> {
    let start = NaiveDate::from_ymd_opt(year, month, 1)?;
    let end = start.checked_add_months(Months::new(1))?.pred_opt()?;
    Some(start..=end)
  }

  fn parse(date: &str) -> Option<Self> {
    match date.split_once('/') {
      Some((start, end)) => Some(Self::Known(
        Self::parse_one(start)?.start().to_owned()..=Self::parse_one(end)?.end().to_owned(),
      )),
      None => Self::parse_one(date).map(Self::Known),
    }
  }

  /// `YYYY-MM-DD`, `YYYY-MM` or `YYYY`, `00` meaning an unknown month or day. Years are written
  /// with 4 digits; Discogs has typos like `197` or `0000-12-25`.
  fn parse_one(date: &str) -> Option<RangeInclusive<NaiveDate>> {
    let mut parts = date.split('-');
    let year = parts.next()?;
    if year.len() != 4 || year.starts_with('0') {
      return None;
    }
    let year = year.parse().ok()?;
    let month = parts.next().map(str::parse).transpose().ok()?.unwrap_or(0);
    let day = parts.next().map(str::parse).transpose().ok()?.unwrap_or(0);
    if parts.next().is_some() {
      return None;
    }
    match (month, day) {
      (0, _) => Self::year(year),
      (month, 0) => Self::month(year, month),
      (month, day) => NaiveDate::from_ymd_opt(year, month, day).map(|day| day..=day),
    }
  }
}

impl FromStr for ReleaseDate {
  type Err = Infallible;

  fn from_str(date: &str) -> Result<Self, Self::Err> {
    Ok(Self::parse(date).unwrap_or_else(|| Self::Unknown(date.to_owned())))
  }
}

/// ISO 8601, with the precision of the date: `1969-10-10`, `1969-11`, `1969`, or an interval.
impl Display for ReleaseDate {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    let days = match self {
      Self::Known(days) => days,
      Self::Unknown(date) => return f.write_str(date),
    };
    let (start, end) = (days.start(), days.end());
    if start == end {
      write!(f, "{}", start.format("%Y-%m-%d"))
    } else if Self::year(start.year()).as_ref() == Some(days) {
      write!(f, "{}", start.format("%Y"))
    } else if Self::month(start.year(), start.month()).as_ref() == Some(days) {
      write!(f, "{}", start.format("%Y-%m"))
    } else {
      write!(f, "{}/{}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d"))
    }
  }
}

/// `""`, `0`, `0000`, `0000-00-00` → `None`: Discogs' ways of saying the release has no date.
pub(crate) struct NoDateAsNone;

impl<'de> DeserializeAs<'de, Option<ReleaseDate>> for NoDateAsNone {
  fn deserialize_as<D: Deserializer<'de>>(deserializer: D) -> Result<Option<ReleaseDate>, D::Error> {
    let date = Option::<String>::deserialize(deserializer)?;
    Ok(
      date
        .filter(|date| !date.chars().all(|char| char == '0' || char == '-'))
        .map(|date| date.parse().unwrap_or_else(|never: Infallible| match never {})),
    )
  }
}

impl Serialize for ReleaseDate {
  fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.collect_str(self)
  }
}

impl<'de> Deserialize<'de> for ReleaseDate {
  fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
    let date = String::deserialize(deserializer)?;
    Ok(date.parse().unwrap_or_else(|never: Infallible| match never {}))
  }
}

#[cfg(test)]
mod tests {
  use chrono::NaiveDate;

  use super::ReleaseDate;

  fn date(year: i32, month: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month, day).unwrap()
  }

  #[test]
  fn parse_discogs_dates() {
    let cases = [
      (
        "1969-10-10",
        ReleaseDate::Known(date(1969, 10, 10)..=date(1969, 10, 10)),
      ),
      ("1969-11-00", ReleaseDate::Known(date(1969, 11, 1)..=date(1969, 11, 30))),
      ("1969-00-00", ReleaseDate::Known(date(1969, 1, 1)..=date(1969, 12, 31))),
      ("1969", ReleaseDate::Known(date(1969, 1, 1)..=date(1969, 12, 31))),
      ("Unknown", ReleaseDate::Unknown("Unknown".to_owned())),
      ("197", ReleaseDate::Unknown("197".to_owned())),
      ("0338", ReleaseDate::Unknown("0338".to_owned())),
      ("0000-12-25", ReleaseDate::Unknown("0000-12-25".to_owned())),
    ];
    for (text, expected) in cases {
      assert_eq!(text.parse::<ReleaseDate>().unwrap(), expected, "{text}");
    }
  }

  #[test]
  fn display_in_iso_8601() {
    let cases = [
      ("1969-10-10", "1969-10-10"),
      ("1969-11-00", "1969-11"),
      ("1969-00-00", "1969"),
      ("1969", "1969"),
      ("Unknown", "Unknown"),
    ];
    for (discogs, iso) in cases {
      assert_eq!(discogs.parse::<ReleaseDate>().unwrap().to_string(), iso, "{discogs}");
    }
    let interval = ReleaseDate::Known(date(1969, 11, 15)..=date(1970, 2, 28));
    assert_eq!(interval.to_string(), "1969-11-15/1970-02-28");
  }

  #[test]
  fn parse_iso_8601() {
    for iso in ["1969-10-10", "1969-11", "1969", "1969-11-15/1970-02-28"] {
      assert_eq!(iso.parse::<ReleaseDate>().unwrap().to_string(), iso);
    }
  }

  #[test]
  fn no_date_as_none() {
    #[serde_with::serde_as]
    #[derive(serde::Deserialize)]
    struct Dated {
      #[serde_as(deserialize_as = "super::NoDateAsNone")]
      #[serde(default)]
      released: Option<ReleaseDate>,
    }
    let released = |json: &str| serde_json::from_str::<Dated>(json).unwrap().released;
    for json in [
      r#"{}"#,
      r#"{"released":null}"#,
      r#"{"released":""}"#,
      r#"{"released":"0"}"#,
      r#"{"released":"0000"}"#,
      r#"{"released":"0000-00-00"}"#,
    ] {
      assert_eq!(released(json), None, "{json}");
    }
    assert_eq!(released(r#"{"released":"1969"}"#), Some("1969".parse().unwrap()));
  }
}
