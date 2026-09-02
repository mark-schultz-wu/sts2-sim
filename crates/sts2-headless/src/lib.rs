//! One implementation of a game: the real one, run headless.
//!
//! The game is the oracle the simulator gets checked against. The real thing,
//! driven one decision at a time. Decisions, steps and recordings are
//! `sts2-core`'s, because a simulator has to speak them too. What is this
//! crate's own is everything that makes the real game answer them. Launching the
//! process, the handshake, and the protocol's spelling of a decision.
//!
//! [`Oracle`] implements [`sts2_core::Play`] and [`sts2_core::Construct`], which
//! is the whole of what it offers a caller driving a game. Everything else here
//! is what it takes to make the real game answer. Launching the process, the
//! handshake, and the protocol's spelling of a decision and of an observation.
//!
//! [PROTOCOL.md](../../../mod/sts2_headless/PROTOCOL.md) is the contract this
//! implements.
//!
//! ```no_run
//! use sts2_content::v0_107_1::V0_107_1;
//! use sts2_core::{ContentVersion, Decision, Play, Seed};
//! use sts2_headless::{Config, Oracle};
//!
//! // The build is named once and every id is reached through the trait, so
//! // pointing this at another build is a one-line change. `launch` refuses a
//! // game whose content or assembly hash disagrees with the tables, and one
//! // running a mod this client was not built against.
//! type V = V0_107_1;
//! type Character = <V as ContentVersion>::Character;
//! type AscensionLevel = <V as ContentVersion>::AscensionLevel;
//!
//! let mut game: Oracle<V> = Oracle::launch(Config::new("/path/to/Slay the Spire 2"))?;
//! let answered = game.play(&Decision::Start {
//!     seed: Seed::new("ABCDEF")?,
//!     character: Character::Ironclad,
//!     ascension: AscensionLevel::None,
//!     modifiers: vec![],
//! })?;
//!
//! // The outer `?` above covered the transport. This is the game's answer.
//! match answered {
//!     Ok(applied) => println!("floor {}", applied.observation.floor),
//!     Err(refusal) => println!("{refusal}"),
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod recording;
mod session;
mod stamp;
mod wire;

pub use session::config::{Config, game_dir_from_env};
pub use session::error::Error;
pub use session::game::{Oracle, parse_reply};
pub use session::held::{Recorded, Session, Status};

/// Keeping a game alive between the processes that drive it.
///
/// A front end does not reach for this. It holds a [`Session`] and drives it
/// through [`sts2_core::Play`]. What is here is for the binary that agrees to
/// *be* the daemon. [`daemon::serve`] is the loop, and [`daemon::spawn`] takes
/// the command that re-enters it.
pub mod daemon {
    pub use crate::session::daemon::{DEFAULT_IDLE_TIMEOUT, running, serve, socket_path, spawn};
}
// The protocol's own shape. Exported for a relay -- something that speaks to a
// game on another process's behalf and must put the same bytes on the wire
// without being the thing that holds the game. Anything actually driving a game
// should build an `sts2_core::Decision` and call `Play::play`.
pub use stamp::Stamp;
pub use wire::command::Command;
pub use wire::greeting::{Greeting, MOD_IDENTITY};
pub use wire::ident::{Error as IdentError, card_ident, coord, model_id};
pub use wire::observation::{Error as ObservationError, Why, observation};
pub use wire::response::{Asked, Response};

/// One reply, as the envelope the game sent it in.
///
/// The wire-level pair, for [`parse_reply`] and the relay that uses it. Anything
/// driving a game holds [`sts2_core::Answered`] instead: the same two results,
/// with the envelope already dropped.
pub type Reply<V> = Result<Response, sts2_core::Refusal<V>>;
