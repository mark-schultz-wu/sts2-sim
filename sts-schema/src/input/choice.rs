//! Answering a request raised by an action already in flight.
//!
//! When a `GameAction` needs something from the player mid-resolution it enters
//! `GameActionState::GatheringPlayerChoice` and waits. The answer is a
//! `PlayerChoiceResult`, tagged with a `PlayerChoiceType`:
//!
//! ```text
//! None | CanonicalCard | CombatCard | DeckCard | MutableCard | Player | Index
//! ```
//!
//! Card rewards, shop purchases and event options are all `Index` choices
//! raised this way.

use serde::{Deserialize, Serialize};
use tinyvec::TinyVec;

use crate::CombatPile;

/// A response to a pending player choice.
///
/// Declining is not a variant: whether a prompt may be refused is stated on the
/// *request*, via `CardSelectorPrefs`'s `MinSelect`, `MaxSelect` and
/// `Cancelable`. Skipping a card reward is an empty [`Selection`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "choice", rename_all = "snake_case")]
pub enum Choice {
    /// Positions in the list the game offered: card rewards, shop stock, event
    /// options, generated card options. The game's `Index` choice type.
    PickOptions { positions: Selection },
    /// Cards picked out of a combat pile — choose a card to discard, put a
    /// card from the draw pile on top. The game's `CombatCard` choice type.
    ///
    /// Not how a card is played; that is
    /// [`Action::PlayCard`](super::Action::PlayCard).
    PickCombatCards {
        pile: CombatPile,
        positions: Selection,
    },
    /// Cards picked out of the master deck — upgrading at a rest site,
    /// removing at a shop. The game's `DeckCard` choice type.
    ///
    /// No pile field: `CardScope::Run` has exactly one pile, so the choice type
    /// already names it.
    ///
    /// The other two choice types, `CanonicalCard` and `MutableCard`, are
    /// template versus instance rather than places, and no known decision point
    /// picks a card that way.
    PickDeckCards { positions: Selection },
    /// Which player, in co-op. The game's `Player` choice type.
    PickPlayer { player: u8 },
}

/// Selected positions, inline up to [`INLINE`](Selection::INLINE) and on the
/// heap beyond that.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Selection(TinyVec<[u8; Selection::INLINE]>);

impl Selection {
    /// Chosen to exceed the largest hand either game deals.
    pub const INLINE: usize = 16;

    pub fn one(position: u8) -> Self {
        let mut v = TinyVec::new();
        v.push(position);
        Selection(v)
    }

    pub fn positions(&self) -> &[u8] {
        &self.0
    }
}

impl FromIterator<u8> for Selection {
    fn from_iter<I: IntoIterator<Item = u8>>(iter: I) -> Self {
        Selection(iter.into_iter().collect())
    }
}

impl core::fmt::Display for Selection {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for (i, p) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(",")?;
            }
            write!(f, "{p}")?;
        }
        Ok(())
    }
}

impl core::fmt::Display for Choice {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Choice::PickOptions { positions } => write!(f, "choose option {positions}"),
            Choice::PickCombatCards { pile, positions } => {
                write!(f, "choose {pile:?} card {positions}")
            }
            Choice::PickDeckCards { positions } => write!(f, "choose deck card {positions}"),
            Choice::PickPlayer { player } => write!(f, "choose player {player}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::*;

    /// Pinned against literals in both directions. Feeding our own output back
    /// in would test nothing, since both directions share a derive.
    #[test]
    fn wire_form_is_tagged() {
        let cases = [
            (
                Choice::PickOptions {
                    positions: Selection::one(2),
                },
                r#"{"choice":"pick_options","positions":[2]}"#,
            ),
            (
                Choice::PickCombatCards {
                    pile: CombatPile::Hand,
                    positions: [1u8, 4, 9].into_iter().collect(),
                },
                r#"{"choice":"pick_combat_cards","pile":"hand","positions":[1,4,9]}"#,
            ),
            (
                Choice::PickDeckCards {
                    positions: Selection::one(3),
                },
                r#"{"choice":"pick_deck_cards","positions":[3]}"#,
            ),
            (
                Choice::PickPlayer { player: 1 },
                r#"{"choice":"pick_player","player":1}"#,
            ),
        ];
        for (choice, json) in cases {
            assert_eq!(serde_json::to_string(&choice).expect("serialises"), json);
            assert_eq!(
                serde_json::from_str::<Choice>(json).expect("parses"),
                choice
            );
        }
    }

    /// Declining is an empty selection, not a variant of its own.
    #[test]
    fn an_empty_selection_is_representable() {
        let json = r#"{"choice":"pick_options","positions":[]}"#;
        let c = Choice::PickOptions {
            positions: Selection::default(),
        };
        assert_eq!(serde_json::to_string(&c).expect("serialises"), json);
        assert_eq!(serde_json::from_str::<Choice>(json).expect("parses"), c);
    }

    /// Why the width is 16: a full hand must not reach the allocator.
    #[test]
    fn a_full_hand_selection_stays_inline() {
        let s: Selection = (0..10u8).collect();
        assert!(s.0.is_inline(), "a 10-card selection spilled to the heap");
        let big: Selection = (0..64u8).collect();
        assert!(!big.0.is_inline(), "expected a spill past the inline width");
        assert_eq!(
            big.positions().len(),
            64,
            "spilling must not lose positions"
        );
    }

    #[test]
    fn display_lists_every_position() {
        let c = Choice::PickCombatCards {
            pile: CombatPile::Hand,
            positions: [0u8, 3].into_iter().collect(),
        };
        assert_eq!(c.to_string(), "choose Hand card 0,3");
    }
}
