//! Front-end concerns for driving the game: the parts that are a choice.
//!
//! `sts2-headless` reports what the game said. How that is spelled at a command
//! line, and how it looks, are opinions. Opinions live here, so a second front
//! end can hold different ones.

pub mod cli;
pub mod coverage;
#[cfg(test)]
pub(crate) mod fixture;
pub mod render;
pub mod runs;
pub mod tui;

pub use render::Readable;
