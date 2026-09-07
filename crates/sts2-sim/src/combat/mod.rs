//! Combat setup and turn resolution.
mod error;
pub use error::Error;

mod encounter;
mod legal;
mod monster;
mod start;
mod stream;
pub(crate) use start::enter;
pub mod pile;
