//! Everything a player can send to the game, in the two tiers the game itself
//! uses:
//!
//! * An [`Action`] is initiated by the player — a `GameAction` subclass.
//! * A [`Choice`] answers a request raised by an action already running, which
//!   parks in `GameActionState::GatheringPlayerChoice` until it gets one.
//!
//! ```text
//! WaitingForExecution → Executing
//!                         → GatheringPlayerChoice ⇄ ReadyToResumeExecuting
//!                         → Finished | Canceled
//! ```
//!
//! So a decision point is in exactly one of two situations, never both: idle
//! and awaiting an action, or mid-action and awaiting a choice.

mod action;
mod choice;

pub use action::Action;
pub use choice::{Choice, Selection};

use serde::{Deserialize, Serialize};

/// One thing a player may send: an action, or an answer to a pending request.
///
/// Tagged, so that a malformed payload produces an error naming the tier.
/// Untagged would parse — the inner tags differ — but would report only "data
/// did not match any variant".
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "input", rename_all = "snake_case")]
pub enum Input {
    Action(Action),
    Choice(Choice),
}

impl core::fmt::Display for Input {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Input::Action(a) => a.fmt(f),
            Input::Choice(c) => c.fmt(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The wire form, pinned against literals. A round trip cannot test this,
    /// since both directions share a derive.
    #[test]
    fn both_tiers_are_tagged_and_nest_their_inner_tag() {
        let cases = [
            (
                Input::Action(Action::EndTurn),
                r#"{"input":"action","action":"end_turn"}"#,
            ),
            (
                Input::Choice(Choice::PickPlayer { player: 1 }),
                r#"{"input":"choice","choice":"pick_player","player":1}"#,
            ),
        ];
        for (input, json) in cases {
            assert_eq!(serde_json::to_string(&input).expect("serialises"), json);
            assert_eq!(serde_json::from_str::<Input>(json).expect("parses"), input);
        }
    }

    /// A wire format that must survive new game versions is under standing
    /// pressure to grow a `#[serde(other)]` catch-all. This pins that it should
    /// fail instead.
    #[test]
    fn an_unknown_verb_is_refused_by_both_tiers() {
        assert!(
            serde_json::from_str::<Input>(r#"{"input":"action","action":"fly_away"}"#).is_err()
        );
        assert!(
            serde_json::from_str::<Input>(r#"{"input":"choice","choice":"fly_away"}"#).is_err()
        );
    }
}
