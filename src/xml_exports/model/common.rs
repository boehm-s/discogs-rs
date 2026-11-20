use serde::{Deserialize, Serialize};

/// Entity cited by its id and name (`<name id="239">Jesper Dahlbäck</name>`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Reference<Id> {
  pub id: Id,
  pub name: String,
}
