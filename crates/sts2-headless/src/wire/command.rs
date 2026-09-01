//! The commands the game accepts.
//! See [PROTOCOL.md](../../../../mod/sts2_headless/PROTOCOL.md) for what each argument means.

use nonmax::NonMaxU32;
use serde::{Serialize, Serializer, ser::SerializeMap};
use sts2_content::ContentVersion;
use sts2_rng::Seed;

/// One request.
///
/// Serializes to exactly the wire form: the variant name becomes `cmd`, and the fields sit beside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
// The associated types carry their own `Serialize` bound, so serde does not
// need one on `V`, which is a marker and does not implement it.
#[serde(tag = "cmd", rename_all = "snake_case", bound = "")]
pub enum Command<V: ContentVersion> {
    /// Starts a fresh run.
    Start {
        seed: Seed,
        character: V::Character,
        ascension: V::AscensionLevel,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        modifiers: Vec<V::Modifier>,
    },
    /// Jumps straight into a named encounter, off the map (H6).
    ///
    /// The encounter is named by type, so a name the tables do not have is a compile error rather than a refusal from the game. To send one anyway. Which is how the refusal path is tested, use `Game::send_raw`.
    ///
    /// The run lands at an unassigned map point, so map and room disagree by construction. That is what scenario construction is for, reaching a chosen fight by travelling means hunting for a seed that puts one where you want it.
    Fight {
        encounter: V::Encounter,
    },
    /// `room` is a `RoomType` name, matched case-insensitively.
    EnterRoom {
        #[serde(rename = "type")]
        room: V::RoomType,
    },
    Travel {
        col: i32,
        row: i32,
    },
    /// `index` is into the hand. `target` is a creature's `CombatId`, not an index into the living creatures, which will shift when one dies.
    PlayCard {
        index: u8,
        target: Option<NonMaxU32>,
    },
    EndTurn,
    /// Indexes into the options the *most recent* observation reported. The rest site list shrinks as it is chosen from, so an older index means nothing.
    ChooseOption {
        index: u8,
    },
    /// Indexes into the shop entries the most recent observation reported.
    Buy {
        index: u8,
    },
    /// At least one of `card` or `relic` must be set, or the game refuses it.
    Grant {
        #[serde(skip_serializing_if = "Option::is_none")]
        card: Option<V::Card>,
        #[serde(skip_serializing_if = "Option::is_none")]
        relic: Option<V::Relic>,
    },
    /// Responds to a [`crate::Response::Pending`].
    Answer(Answer),
    /// Asks for the current state without changing anything.
    ///
    /// One of the three commands a pending question does not block, and one of the three still accepted once a run is over. Therefore it is how a caller finds out where it stands when everything else is refused.
    Observe,

    Quit,
}

/// How to answer a parked game.
///
/// All three forms share the `answer` command and are told apart by which field they carry. Therefore this is a sum type rather than three optional fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Option indices, for a [`crate::Question::SelectCards`]. Repeating one is refused: the same card twice is one selection to the game and two to the client.
    Cards(Vec<usize>),
    /// Decline a [`crate::Question::Rewards`]. Refused if it is not skippable.
    Skip,
    /// Take rewards from a [`crate::Question::Rewards`].
    Take(Vec<TakeReward>),
}

impl Serialize for Answer {
    /// Written by hand because `skip` is the literal `true` rather than a value the caller supplies. Deriving it will need a `bool` field that can be set to `false`, which is not an answer the game accepts.
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
/// `card` is required when the reward is a card reward and meaningless otherwise. The game refuses a card reward without one rather than choosing the first card for you.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TakeReward {
    pub index: usize,
    pub card: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, to_value};
    use sts2_content::ContentVersion;
    use sts2_content::v0_107_1::V0_107_1;

    /// The build these tests spell commands for.
    ///
    /// Named once, and every id reached through the trait rather than out of the version module. The wire form is what the typed ids serialize to, and pointing this at another build is a one-line change.
    type V = V0_107_1;
    type Cmd = Command<V>;
    type AscensionLevel = <V as ContentVersion>::AscensionLevel;
    type Character = <V as ContentVersion>::Character;
    type Modifier = <V as ContentVersion>::Modifier;
    type RoomType = <V as ContentVersion>::RoomType;

    #[test]
    fn start_carries_what_was() {
        let full = Cmd::Start {
            seed: Seed::new("abc").expect("a seed"),
            character: Character::Silent,
            ascension: AscensionLevel::AscendersBane,
            modifiers: vec![Modifier::Draft],
        };
        assert_eq!(
            to_value(&full).unwrap(),
            json!({
                "cmd": "start", "ascension": "AscendersBane", "seed": "ABC",
                "character": "SILENT", "modifiers": ["DRAFT"],
            }),
            "the typed ids have to reach the wire as the game's own spellings"
        );
    }

    #[test]
    fn a_command_with_no_arguments_is_just_its_name() {
        assert_eq!(to_value(Cmd::EndTurn).unwrap(), json!({"cmd": "end_turn"}));
        assert_eq!(to_value(Cmd::Quit).unwrap(), json!({"cmd": "quit"}));
    }

    /// `type` is a Rust keyword, so this is the one field a derive has to rename.
    ///
    /// It also shows what typing the field bought. This used to send `"SHOP"`, hand-written, and worked because the mod parses with `ignoreCase: true`. The generated id says `"Shop"`, which is the game enum's actual name. The string is no longer a guess that leniency happened to rescue.
    #[test]
    fn enter_room_calls_it_type() {
        let cmd = Cmd::EnterRoom {
            room: RoomType::Shop,
        };
        assert_eq!(
            to_value(&cmd).unwrap(),
            json!({"cmd": "enter_room", "type": "Shop"})
        );
    }

    #[test]
    fn an_untargeted_card_sends_a_null_target() {
        let cmd = Cmd::PlayCard {
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
            let v = to_value(Cmd::Answer(answer)).unwrap();
            assert_eq!(v["cmd"], json!("answer"));
        }
    }

    #[test]
    fn each_answer_carries_only_its_own_field() {
        assert_eq!(
            to_value(Cmd::Answer(Answer::Cards(vec![0, 2]))).unwrap(),
            json!({"cmd": "answer", "cards": [0, 2]})
        );
        assert_eq!(
            to_value(Cmd::Answer(Answer::Skip)).unwrap(),
            json!({"cmd": "answer", "skip": true})
        );
        assert_eq!(
            to_value(Cmd::Answer(Answer::Take(vec![TakeReward {
                index: 1,
                card: Some(0),
            }])))
            .unwrap(),
            json!({"cmd": "answer", "take": [{"index": 1, "card": 0}]})
        );
    }
}
