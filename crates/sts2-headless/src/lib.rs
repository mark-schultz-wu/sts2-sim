//! A client for the `sts2_headless` mod.
//!
//! The game is the oracle the simulator gets checked against: the real thing,
//! run headless, driven one decision at a time. This crate owns the transport
//! and nothing above it: launching the game, the handshake, and one typed
//! request/response pair per call. What the state *means* is deliberately left
//! alone — see the `response` module.
//!
//! [PROTOCOL.md](../../../mod/sts2_headless/PROTOCOL.md) is the contract this
//! implements.
//!
//! ```no_run
//! use sts2_content::ContentVersion;
//! use sts2_content::v0_107_1::V0_107_1;
//! use sts2_headless::{Command, Config, Game, Response};
//! use sts2_rng::Seed;
//!
//! // The build is named once and every id is reached through the trait, so
//! // pointing this at another build is a one-line change. `launch` refuses a
//! // game whose content or assembly hash disagrees with the tables.
//! type V = V0_107_1;
//! type Character = <V as ContentVersion>::Character;
//! type AscensionLevel = <V as ContentVersion>::AscensionLevel;
//!
//! let mut game: Game<V> = Game::launch(Config::new("/path/to/Slay the Spire 2"))?;
//! let reply = game.send(&Command::Start {
//!     seed: Seed::new("ABCDEF")?,
//!     character: Character::Ironclad,
//!     ascension: AscensionLevel::None,
//!     modifiers: vec![],
//! })?;
//!
//! // The outer `?` above covered the transport. This is the game's answer.
//! match reply {
//!     Ok(Response::Observation { digest, .. }) => println!("agreed at {digest}"),
//!     // `Debug` is what this crate offers. Rendering an observation for a
//!     // person is a front end's choice, and lives in `sts2-cli`.
//!     Ok(other) => println!("{other:?}"),
//!     Err(refusal) => println!("{refusal}"),
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod session;
mod wire;

pub use session::config::{Config, game_dir_from_env};
pub use session::error::Error;
pub use session::game::{Game, parse_reply};
pub use wire::command::{Answer, Command, TakeReward};
pub use wire::greeting::{Greeting, PROTOCOL_VERSION};
pub use wire::response::{CardOption, Question, Refusal, Response, RewardDetail, RewardKind};

/// What a command produced: the game's answer, or its refusal to act.
///
/// The outer `Result` in [`Game::send`] is the transport; this is the game.
pub type Reply = Result<Response, Refusal>;
