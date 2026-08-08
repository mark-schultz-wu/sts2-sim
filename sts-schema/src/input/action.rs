//! What a player initiates.
//!
//! One variant per `GameAction` subclass in `MegaCrit.Sts2.Core.GameActions`
//! that also has a `Net*Action` struct — the form it takes on the wire in
//! co-op. That pairing is the test for "a player can send this", and it admits
//! exactly ten; `GenericHookGameAction` is the eleventh subclass and has no
//! `Net*` struct, being the hook mechanism rather than a player verb.
//!
//! Four of the ten are co-op-shaped and modelled regardless. An adapter that
//! never emits one costs nothing, whereas guessing which single-player omits
//! would drift from the game's list.

use nonmax::NonMaxU8;
use serde::{Deserialize, Serialize};

/// A player-initiated action.
///
/// `Copy` and pointer-free: enumerating the legal set is on the hot path of any
/// search.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    /// `PlayCardAction`. `target` is absent for cards that do not take one,
    /// and [`NonMaxU8`] so `Option` occupies its niche rather than a second
    /// byte — at the cost of 255 being unrepresentable.
    PlayCard { card: u8, target: Option<NonMaxU8> },
    /// `UsePotionAction`.
    UsePotion {
        potion: u8,
        target: Option<NonMaxU8>,
    },
    /// `DiscardPotionGameAction`. Its own action in the game, not a choice.
    DiscardPotion { potion: u8 },
    /// `EndPlayerTurnAction`.
    EndTurn,
    /// `UndoEndPlayerTurnAction`. STS2 lets a turn be un-ended before the
    /// enemies act; STS1 has no equivalent.
    UndoEndTurn,
    /// `MoveToMapCoordAction`.
    MoveToMapCoord { col: u8, row: u8 },
    /// `PickRelicAction`.
    PickRelic { relic: u8 },
    /// `ReadyToBeginEnemyTurnAction`. A synchronisation ack rather than a
    /// decision, and possibly co-op only.
    ReadyToBeginEnemyTurn,
    /// `VoteForMapCoordAction`. Co-op's [`MoveToMapCoord`](Action::MoveToMapCoord):
    /// players vote and the winning coordinate becomes the move.
    VoteForMapCoord { col: u8, row: u8 },
    /// `VoteToMoveToNextActAction`. The only `GameAction` that advances the
    /// act, so it is the verb for leaving one in single-player too.
    VoteToMoveToNextAct,
}

impl core::fmt::Display for Action {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match *self {
            Action::PlayCard {
                card,
                target: Some(t),
            } => write!(f, "play card {card} at {t}"),
            Action::PlayCard { card, target: None } => write!(f, "play card {card}"),
            Action::UsePotion {
                potion,
                target: Some(t),
            } => {
                write!(f, "use potion {potion} at {t}")
            }
            Action::UsePotion {
                potion,
                target: None,
            } => write!(f, "use potion {potion}"),
            Action::DiscardPotion { potion } => write!(f, "discard potion {potion}"),
            Action::EndTurn => write!(f, "end turn"),
            Action::UndoEndTurn => write!(f, "undo end turn"),
            Action::MoveToMapCoord { col, row } => write!(f, "move to map ({col},{row})"),
            Action::PickRelic { relic } => write!(f, "pick relic {relic}"),
            Action::ReadyToBeginEnemyTurn => write!(f, "ready for enemy turn"),
            Action::VoteForMapCoord { col, row } => write!(f, "vote for map ({col},{row})"),
            Action::VoteToMoveToNextAct => write!(f, "vote to move to next act"),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::*;

    /// The wire form is a contract with adapters and recorded traces, so it is
    /// pinned against literals rather than our own serialiser.
    #[test]
    fn wire_form_is_tagged_and_snake_case() {
        let cases = [
            (
                Action::PlayCard {
                    card: 2,
                    target: NonMaxU8::new(0),
                },
                r#"{"action":"play_card","card":2,"target":0}"#,
            ),
            (Action::EndTurn, r#"{"action":"end_turn"}"#),
            (
                Action::MoveToMapCoord { col: 3, row: 1 },
                r#"{"action":"move_to_map_coord","col":3,"row":1}"#,
            ),
        ];
        for (action, json) in cases {
            assert_eq!(serde_json::to_string(&action).expect("serialises"), json);
            assert_eq!(
                serde_json::from_str::<Action>(json).expect("parses"),
                action
            );
        }
    }

    /// An absent target must not be confused with targeting monster 0.
    #[test]
    fn absent_target_differs_from_target_zero() {
        let none = Action::PlayCard {
            card: 1,
            target: None,
        };
        let zero = Action::PlayCard {
            card: 1,
            target: NonMaxU8::new(0),
        };
        assert_ne!(none, zero);
        assert_ne!(
            serde_json::to_string(&none).unwrap(),
            serde_json::to_string(&zero).unwrap()
        );
    }

    /// 3 bytes, not 4: `Option<NonMaxU8>` fits in the niche.
    #[test]
    fn stays_small_and_copy() {
        assert_eq!(core::mem::size_of::<Action>(), 3);
        assert_eq!(core::mem::size_of::<Option<NonMaxU8>>(), 1);
        fn assert_copy<T: Copy>() {}
        assert_copy::<Action>();
    }

    /// What the niche costs: `NonMaxU8::new(255)` is `None`, so an adapter
    /// converting a raw index must reject 255 rather than let it read as "no
    /// target".
    #[test]
    fn target_255_is_not_representable() {
        assert_eq!(NonMaxU8::new(255), None);
        assert_eq!(NonMaxU8::new(254).map(|t| t.get()), Some(254));
    }

    #[test]
    fn display_distinguishes_targeted_from_untargeted() {
        assert_eq!(
            Action::PlayCard {
                card: 2,
                target: None
            }
            .to_string(),
            "play card 2"
        );
        assert_eq!(
            Action::PlayCard {
                card: 2,
                target: NonMaxU8::new(1)
            }
            .to_string(),
            "play card 2 at 1"
        );
    }
}
