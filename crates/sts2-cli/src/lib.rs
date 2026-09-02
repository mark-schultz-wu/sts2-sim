//! Front-end concerns for driving the game: the parts that are a choice.
//! `sts2-headless` reports what the game said. How that is spelled at a command line and what it must look like are opinions. Opinions live here so a second front end can hold different ones.

pub mod cli;
pub mod render;
pub mod session;

pub use render::Readable;
