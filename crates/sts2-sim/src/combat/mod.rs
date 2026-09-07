//! Combat setup and turn resolution.
mod error;
pub use error::Error;

mod encounter;
mod monster;
pub(crate) use encounter::opening;
pub(crate) use monster::initialize;
pub mod pile;
