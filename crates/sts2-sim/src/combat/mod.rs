//! Combat setup and turn resolution.
mod error;
pub use error::Error;

mod encounter;
mod legal;
mod monster;
mod play;
mod start;
mod stream;
pub(crate) use play::{play, resolve};
pub(crate) use start::enter;
pub mod pile;
