//! The commands the oracle accepts.
//!
//! Built as JSON by hand rather than derived. The protocol has three different
//! `answer` shapes sharing one `cmd` value, and an `enter_room` argument called
//! `type`; expressing that through serde attributes costs more attention to read
//! than the `json!` calls do, and this way each variant sits next to the line of
//! [PROTOCOL.md](../../../mod/sts_oracle/PROTOCOL.md) it implements.

use serde_json::{Value, json};

/// One request. See `PROTOCOL.md` for what each argument means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Starts a fresh run. `ascension` defaults to 0 and `modifiers` may be empty.
    Start {
        seed: Option<String>,
        character: Option<String>,
        ascension: u32,
        modifiers: Vec<String>,
    },
    EnterFight,
    /// `room` is a `RoomType` name, matched case-insensitively.
    EnterRoom {
        room: String,
    },
    Travel {
        col: i32,
        row: i32,
    },
    /// `index` is into the hand; `target` is a creature's `CombatId`, not an index.
    PlayCard {
        index: usize,
        target: Option<u32>,
    },
    EndTurn,
    /// Indexes into the options the *most recent* observation reported.
    ChooseOption {
        index: usize,
    },
    /// Indexes into the shop entries the *most recent* observation reported.
    Buy {
        index: usize,
    },
    /// At least one of `card` or `relic` must be set, or the oracle refuses it.
    Grant {
        card: Option<String>,
        relic: Option<String>,
    },
    /// Answers a `select_cards` pending with option indices.
    AnswerCards {
        cards: Vec<usize>,
    },
    /// Answers a `rewards` pending by declining. Refused if it is not skippable.
    AnswerSkip,
    /// Answers a `rewards` pending by taking rewards.
    AnswerTake {
        take: Vec<TakeReward>,
    },
    Observe,
    Quit,
}

/// One entry in an `answer`'s `take` array.
///
/// `card` is required when the reward is a `CardReward` and meaningless
/// otherwise; the oracle refuses a card reward without one rather than choosing
/// the first card for you.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TakeReward {
    pub index: usize,
    pub card: Option<usize>,
}

impl Command {
    /// The wire form, one JSON object.
    pub fn to_json(&self) -> Value {
        match self {
            Self::Start {
                seed,
                character,
                ascension,
                modifiers,
            } => {
                let mut v = json!({ "cmd": "start", "ascension": ascension });
                let map = v.as_object_mut().expect("just built an object");
                if let Some(seed) = seed {
                    map.insert("seed".into(), json!(seed));
                }
                if let Some(character) = character {
                    map.insert("character".into(), json!(character));
                }
                if !modifiers.is_empty() {
                    map.insert("modifiers".into(), json!(modifiers));
                }
                v
            }
            Self::EnterFight => json!({ "cmd": "enter_fight" }),
            Self::EnterRoom { room } => json!({ "cmd": "enter_room", "type": room }),
            Self::Travel { col, row } => json!({ "cmd": "travel", "col": col, "row": row }),
            Self::PlayCard { index, target } => {
                // A null target is the same as an absent one to the oracle, so
                // this can stay unconditional.
                json!({ "cmd": "play_card", "index": index, "target": target })
            }
            Self::EndTurn => json!({ "cmd": "end_turn" }),
            Self::ChooseOption { index } => json!({ "cmd": "choose_option", "index": index }),
            Self::Buy { index } => json!({ "cmd": "buy", "index": index }),
            Self::Grant { card, relic } => {
                let mut v = json!({ "cmd": "grant" });
                let map = v.as_object_mut().expect("just built an object");
                if let Some(card) = card {
                    map.insert("card".into(), json!(card));
                }
                if let Some(relic) = relic {
                    map.insert("relic".into(), json!(relic));
                }
                v
            }
            Self::AnswerCards { cards } => json!({ "cmd": "answer", "cards": cards }),
            Self::AnswerSkip => json!({ "cmd": "answer", "skip": true }),
            Self::AnswerTake { take } => {
                let take: Vec<Value> = take
                    .iter()
                    .map(|t| json!({ "index": t.index, "card": t.card }))
                    .collect();
                json!({ "cmd": "answer", "take": take })
            }
            Self::Observe => json!({ "cmd": "observe" }),
            Self::Quit => json!({ "cmd": "quit" }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_omits_what_was_not_asked_for() {
        let bare = Command::Start {
            seed: None,
            character: None,
            ascension: 0,
            modifiers: vec![],
        };
        assert_eq!(bare.to_json(), json!({ "cmd": "start", "ascension": 0 }));
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
            full.to_json(),
            json!({
                "cmd": "start", "ascension": 5, "seed": "ABC",
                "character": "SILENT", "modifiers": ["HEIRLOOM"],
            })
        );
    }

    /// `type` is a Rust keyword, so this is the one field name a derive would
    /// have had to rename. Worth pinning.
    #[test]
    fn enter_room_calls_it_type() {
        let cmd = Command::EnterRoom {
            room: "SHOP".into(),
        };
        assert_eq!(
            cmd.to_json(),
            json!({ "cmd": "enter_room", "type": "SHOP" })
        );
    }

    /// All three answer forms share one `cmd`, which is the reason this module
    /// builds JSON by hand.
    #[test]
    fn every_answer_form_is_the_same_command() {
        for cmd in [
            Command::AnswerCards { cards: vec![0, 2] },
            Command::AnswerSkip,
            Command::AnswerTake { take: vec![] },
        ] {
            assert_eq!(cmd.to_json()["cmd"], json!("answer"));
        }
    }

    #[test]
    fn skip_is_the_flag_the_oracle_looks_for() {
        assert_eq!(
            Command::AnswerSkip.to_json(),
            json!({ "cmd": "answer", "skip": true })
        );
    }

    #[test]
    fn a_taken_card_reward_carries_its_card_index() {
        let cmd = Command::AnswerTake {
            take: vec![TakeReward {
                index: 1,
                card: Some(0),
            }],
        };
        assert_eq!(
            cmd.to_json(),
            json!({ "cmd": "answer", "take": [{ "index": 1, "card": 0 }] })
        );
    }

    #[test]
    fn an_untargeted_card_sends_a_null_target() {
        let cmd = Command::PlayCard {
            index: 2,
            target: None,
        };
        assert_eq!(
            cmd.to_json(),
            json!({ "cmd": "play_card", "index": 2, "target": null })
        );
    }
}
