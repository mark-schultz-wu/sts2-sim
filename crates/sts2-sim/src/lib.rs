//! The simulator: the game's rules in Rust, with no game process behind them.
//!
//! Gameplay rules live under their domain modules. This first slice generates
//! the act map from the same random stream and constraints as the game.

pub mod combat;
mod engine;
pub mod event;
pub mod map;
mod reward;
pub mod run;

pub use combat::Error;
pub use engine::Sim;
