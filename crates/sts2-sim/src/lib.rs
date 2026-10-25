//! The simulator: the game's rules in Rust, with no game process behind them.
//!
//! Gameplay rules live under their domain modules. This first slice generates
//! the act map from the same random stream and constraints as the game.

mod bounded;
pub mod combat;
mod engine;
pub mod event;
mod legal;
pub mod map;
mod reward;
pub mod rng;
pub mod run;
mod seats;
mod stream;

pub use combat::Error;
pub use engine::{Sim, SimSeat};
pub use legal::Legal;
