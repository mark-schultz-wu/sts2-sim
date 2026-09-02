//! Reaching a position ordinary play takes an hour to arrive at.
//!
//! Separate from [`crate::Decision`] because these are not moves in a run, and
//! not every implementation can make them. The oracle has them because the mod
//! exposes the game's own debug commands, and a simulator gets them when it can
//! build the same states.

use crate::ContentVersion;

/// One step of scenario construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Setup<V: ContentVersion> {
    /// Jumps straight into a named encounter, off the map (H6).
    ///
    /// The encounter is named by type, so a name the tables do not have is a
    /// compile error rather than a refusal from the game.
    ///
    /// The run lands at an unassigned map point, so map and room disagree by
    /// construction. Reaching the same fight by travelling means hunting for a
    /// seed that puts one where you want it.
    Fight { encounter: V::Encounter },
    /// Jumps straight to a named event, off the map.
    ///
    /// `enter_room` takes a room *type*, so an `Event` room gives whichever
    /// event the seed offers. Naming one is the only way to reach a particular
    /// event without playing to it.
    Event { event: V::Event },
    /// Puts a card, a relic or a potion straight into the run.
    Grant(Gift<V>),
}

/// What a [`Setup::Grant`] puts into the run.
///
/// The mod reads `card`, `relic` and `potion` as three optional keys and will
/// do all of them in one message: `Driver.GrantCard`, then `RelicCmd.Obtain`,
/// then `PotionCmd.TryToProcure`. Three grants reach the same state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gift<V: ContentVersion> {
    Card(V::Card),
    Relic(V::Relic),
    /// Procured the way the game procures one, so a full belt silently refuses
    /// it. The belt in the next observation says whether it landed.
    Potion(V::Potion),
}
