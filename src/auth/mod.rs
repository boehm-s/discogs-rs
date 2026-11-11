//! Authentication domain: personal access token, OAuth flow, application credentials and
//! identity of the authenticated user.

pub mod client;
pub mod model;
pub(crate) mod service;
#[cfg(test)]
pub(crate) mod test_helper;
