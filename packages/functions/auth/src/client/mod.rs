//! Client registry module.
//!
//! Manages OAuth client registration, validation, and secrets.

mod registry;
mod types;
mod validation;

pub use registry::*;
pub use types::*;
pub use validation::*;
