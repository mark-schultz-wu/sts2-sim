//! Where the player is: what is on screen, and which room they are in.
//!
//! The two are independent. A fight, a shop and a rest site are all
//! [`ScreenType::Room`], told apart by the [`RoomType`] beside them.

use serde::{Deserialize, Serialize};

/// What the player is looking at. The game's `Multiplayer.NetScreenType`,
/// minus its `None`, which Rust spells `Option`.
///
/// Says nothing on its own about what is happening — see [`RoomType`].
///
/// Carries the game's full set, including values that are UI-only and will
/// never appear at a decision point, so that nothing needs a catch-all.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScreenType {
    /// In a room. [`Obs::room`](crate::Obs::room) says which kind.
    Room,
    Map,
    Settings,
    Compendium,
    DeckView,
    CardPile,
    SimpleCardsView,
    CardSelection,
    GameOver,
    PauseMenu,
    Rewards,
    Feedback,
    SharedRelicPicking,
    RemotePlayerExpandedState,
}

/// Which room the player is in. The game's `Rooms.RoomType`, minus its
/// `Unassigned`, which Rust spells `Option`.
///
/// `Elite`, `Boss` and `Treasure` differ from `Monster` in what they roll for
/// rewards rather than in what is drawn, which is why the room is its own axis
/// and not part of [`ScreenType`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoomType {
    Monster,
    Elite,
    Boss,
    Treasure,
    Shop,
    Event,
    RestSite,
    /// The room, not [`ScreenType::Map`], which is the screen.
    Map,
}
