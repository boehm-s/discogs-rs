//! Typed ids, so that a release id cannot be passed where a master id is expected.

use std::cmp::Ordering;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::num::ParseIntError;
use std::ops::Deref;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Discogs id typed by the resource it identifies.
///
/// ```
/// # use discogs_client::helpers::ids::ReleaseId;
/// let release_id = ReleaseId::new(878_534);
/// ```
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct TypeId<T>(pub u64, #[serde(skip)] PhantomData<T>);

impl<T> TypeId<T> {
  pub const fn new(id: u64) -> Self {
    Self(id, PhantomData)
  }
}

impl<T> Debug for TypeId<T> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    let full_type_name = std::any::type_name::<T>();
    let type_name = full_type_name.split("::").last().unwrap_or(full_type_name);
    write!(f, "TypeId<{}>({})", type_name, self.0)
  }
}

// Implemented manually: deriving would require `T` to implement them too
impl<T> Clone for TypeId<T> {
  fn clone(&self) -> Self {
    *self
  }
}
impl<T> Copy for TypeId<T> {}
impl<T> PartialEq for TypeId<T> {
  fn eq(&self, other: &Self) -> bool {
    self.0 == other.0
  }
}
impl<T> Eq for TypeId<T> {}
impl<T> Hash for TypeId<T> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.0.hash(state);
  }
}
impl<T> PartialOrd for TypeId<T> {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}
impl<T> Ord for TypeId<T> {
  fn cmp(&self, other: &Self) -> Ordering {
    self.0.cmp(&other.0)
  }
}

impl<T> Display for TypeId<T> {
  fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    Display::fmt(&self.0, f)
  }
}

impl<T> FromStr for TypeId<T> {
  type Err = ParseIntError;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Ok(Self::new(s.parse()?))
  }
}

impl<T> Deref for TypeId<T> {
  type Target = u64;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl<T> From<u64> for TypeId<T> {
  fn from(id: u64) -> Self {
    Self::new(id)
  }
}

impl<T> From<TypeId<T>> for u64 {
  fn from(id: TypeId<T>) -> Self {
    id.0
  }
}

pub enum _ArtistPhantomType {}
pub type ArtistId = TypeId<_ArtistPhantomType>;

pub enum _LabelPhantomType {}
pub type LabelId = TypeId<_LabelPhantomType>;

pub enum _MasterReleasePhantomType {}
pub type MasterReleaseId = TypeId<_MasterReleasePhantomType>;

pub enum _ReleasePhantomType {}
pub type ReleaseId = TypeId<_ReleasePhantomType>;

pub enum _UserPhantomType {}
pub type UserId = TypeId<_UserPhantomType>;

#[cfg(test)]
mod tests {
  use super::ReleaseId;

  #[test]
  fn debug_shows_the_resource() {
    assert_eq!(
      format!("{:?}", ReleaseId::new(878_534)),
      "TypeId<_ReleasePhantomType>(878534)"
    );
  }

  #[test]
  fn serializes_as_a_number() {
    assert_eq!(serde_json::to_string(&ReleaseId::new(878_534)).unwrap(), "878534");
    assert_eq!(
      serde_json::from_str::<ReleaseId>("878534").unwrap(),
      ReleaseId::new(878_534)
    );
  }
}
