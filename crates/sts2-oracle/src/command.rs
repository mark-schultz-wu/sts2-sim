//! The commands the oracle accepts.
//!
//! See [PROTOCOL.md](../../../mod/sts2_headless/PROTOCOL.md) for what each argument
//! means.

use serde::{Serialize, Serializer, ser::SerializeMap};

/// One request.
///
/// Serializes to exactly the wire form: the variant name becomes `cmd`, and the
/// fields sit beside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    /// Starts a fresh run. Omitted fields fall back to the oracle's defaults:
    /// seed `ORACLE1`, and the Ironclad.
    Start {
        #[serde(skip_serializing_if = "Option::is_none")]
        seed: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        character: Option<String>,
        ascension: u32,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        modifiers: Vec<String>,
    },
    /// Jumps straight into a named encounter, off the map (H6).
    ///
    /// `encounter` is a model entry name such as `BOWLBUGS_WEAK`. Unknown names
    /// are refused by the game with its own message rather than validated here:
    /// there is no id set on this side to check against, and the game's answer
    /// is better than a guess.
    ///
    /// The run lands at an unassigned map point, so map and room disagree by
    /// construction. That is what scenario construction is for — reaching a
    /// chosen fight by travelling means hunting for a seed that puts one where
    /// you want it.
    Fight {
        encounter: String,
    },
    /// `room` is a `RoomType` name, matched case-insensitively.
    EnterRoom {
        #[serde(rename = "type")]
        room: String,
    },
    Travel {
        col: i32,
        row: i32,
    },
    /// `index` is into the hand; `target` is a creature's `CombatId`, not an
    /// index into the living creatures, which would shift when one dies.
    PlayCard {
        index: usize,
        target: Option<u32>,
    },
    EndTurn,
    /// Indexes into the options the *most recent* observation reported. The rest
    /// site list shrinks as it is chosen from, so an older index means nothing.
    ChooseOption {
        index: usize,
    },
    /// Indexes into the shop entries the most recent observation reported.
    Buy {
        index: usize,
    },
    /// At least one of `card` or `relic` must be set, or the oracle refuses it.
    Grant {
        #[serde(skip_serializing_if = "Option::is_none")]
        card: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        relic: Option<String>,
    },
    /// Responds to a [`crate::Response::Pending`].
    Answer(Answer),
    Quit,
}

/// How to answer a parked game.
///
/// All three forms share the `answer` command and are told apart by which field
/// they carry, so this is a sum type rather than three optional fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Option indices, for a [`crate::Question::SelectCards`]. Repeating one is
    /// refused: the same card twice is one selection to the game and two to the
    /// client.
    Cards(Vec<usize>),
    /// Decline a [`crate::Question::Rewards`]. Refused if it is not skippable.
    Skip,
    /// Take rewards from a [`crate::Question::Rewards`].
    Take(Vec<TakeReward>),
}

impl Serialize for Answer {
    /// Written by hand because `skip` is the literal `true` rather than a value
    /// the caller supplies — deriving it would need a `bool` field that could be
    /// set to `false`, which is not an answer the oracle accepts.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            Self::Cards(cards) => map.serialize_entry("cards", cards)?,
            Self::Skip => map.serialize_entry("skip", &true)?,
            Self::Take(take) => map.serialize_entry("take", take)?,
        }
        map.end()
    }
}

/// One entry in an [`Answer::Take`].
///
/// `card` is required when the reward is a card reward and meaningless
/// otherwise; the oracle refuses a card reward without one rather than choosing
/// the first card for you.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TakeReward {
    pub index: usize,
    pub card: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};

    #[test]
    fn start_omits_what_was_not_asked_for() {
        let bare = Command::Start {
            seed: None,
            character: None,
            ascension: 0,
            modifiers: vec![],
        };
        assert_eq!(
            to_value(&bare).unwrap(),
            json!({"cmd": "start", "ascension": 0})
        );
    }

    #[test]
    fn start_carries_what_was() {
        let full = Command::Start {
            seed: Some("ABC".into()),
            character: Some("SILENT".into()),
            ascension: 5,
            modifiers: vec!["HEIRLOOM".into()],
        };
        assert_eq!(
            to_value(&full).unwrap(),
            json!({
                "cmd": "start", "ascension": 5, "seed": "ABC",
                "character": "SILENT", "modifiers": ["HEIRLOOM"],
            })
        );
    }

    #[test]
    fn a_command_with_no_arguments_is_just_its_name() {
        assert_eq!(
            to_value(Command::EndTurn).unwrap(),
            json!({"cmd": "end_turn"})
        );
        assert_eq!(to_value(Command::Quit).unwrap(), json!({"cmd": "quit"}));
    }

    /// `type` is a Rust keyword, so this is the one field a derive has to rename.
    #[test]
    fn enter_room_calls_it_type() {
        let cmd = Command::EnterRoom {
            room: "SHOP".into(),
        };
        assert_eq!(
            to_value(&cmd).unwrap(),
            json!({"cmd": "enter_room", "type": "SHOP"})
        );
    }

    #[test]
    fn an_untargeted_card_sends_a_null_target() {
        let cmd = Command::PlayCard {
            index: 2,
            target: None,
        };
        assert_eq!(
            to_value(&cmd).unwrap(),
            json!({"cmd": "play_card", "index": 2, "target": null})
        );
    }

    /// All three answers are one command, distinguished by the field they carry.
    #[test]
    fn every_answer_is_the_same_command() {
        for answer in [
            Answer::Cards(vec![0, 2]),
            Answer::Skip,
            Answer::Take(vec![]),
        ] {
            let v = to_value(Command::Answer(answer)).unwrap();
            assert_eq!(v["cmd"], json!("answer"));
        }
    }

    #[test]
    fn each_answer_carries_only_its_own_field() {
        assert_eq!(
            to_value(Command::Answer(Answer::Cards(vec![0, 2]))).unwrap(),
            json!({"cmd": "answer", "cards": [0, 2]})
        );
        assert_eq!(
            to_value(Command::Answer(Answer::Skip)).unwrap(),
            json!({"cmd": "answer", "skip": true})
        );
        assert_eq!(
            to_value(Command::Answer(Answer::Take(vec![TakeReward {
                index: 1,
                card: Some(0),
            }])))
            .unwrap(),
            json!({"cmd": "answer", "take": [{"index": 1, "card": 0}]})
        );
    }
}
