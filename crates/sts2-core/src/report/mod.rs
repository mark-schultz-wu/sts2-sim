//! What an implementation says back.
//!
//! Everything here is a *snapshot*: what was true at one quiescent moment,
//! derived and handed over. Never the machine that produced it. A simulator's
//! combat state holds RNG streams and an action queue, and is nobody else's
//! business. What it derives from them is [`Observation`], which is exactly what
//! the mod's `Observation.cs` derives from `RunState`. Two snapshots can be
//! compared. Two machines cannot.
//!
//! Three things, in the order a caller meets them. [`Observation`] is where the
//! run is standing. [`Question`] is what a parked game is waiting to be told,
//! and it sits beside an observation rather than inside one. The game has
//! stopped mid-action, so it has both a position and a question about it.
//! [`Digest`] is eight bytes over the first of them, for the comparison that
//! runs a few hundred times a second.

pub mod digest;
pub mod observation;
pub mod observed;
pub mod question;
pub mod view;
