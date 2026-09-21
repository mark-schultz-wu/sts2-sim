//! How a decision is spelled on the wire.
//!
//! The mod's protocol, and only the mod's. `sts2_core::Decision` is the domain
//! type and carries no serde at all, so this is where `{"cmd":"play_card"}`
//! lives. Same rule the observation follows: the oracle's spelling stays in the
//! oracle's crate.
//!
//! The two vocabularies are not the same set, which is the clearest sign they
//! are not the same type. `observe` and `quit` exist here and have no domain
//! counterpart. One reports state without advancing, and the other ends the
//! session rather than the run.

use num_traits::ToPrimitive as _;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeMap};
use sts2_core::Seed;
use sts2_core::{
    Answer, Coord, Decision, Gift, PowerAmount, PowerTable as _, Setup, Start, Step, TakeReward,
    Tool,
};

/// One request, in the protocol's own shape.
///
/// The variant name becomes `cmd` and the fields sit beside it. Readable as
/// well as writable, because a recorded run is a file of these and replaying
/// one means reading them back. That is also what makes the round trip
/// testable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// The associated types carry their own bounds, so serde does not need any on
// `V` — which is a marker and implements neither.
#[serde(tag = "cmd", rename_all = "snake_case", bound = "")]
pub enum Command<V: sts2_content_wire::WireContentVersion> {
    Start {
        #[serde(with = "seed")]
        seed: Seed,
        character: V::Character,
        ascension: V::AscensionLevel,
        // Omitted from the wire when empty, so absent has to read back as
        // empty — otherwise a recorded `start` with no modifiers is a file this
        // cannot reopen.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        modifiers: Vec<V::Modifier>,
    },
    Fight {
        encounter: V::Encounter,
    },
    Event {
        event: V::Event,
    },
    /// The amount is the game's own overloaded number. Below zero is a single
    /// endless instance for the one power that decides its stacking at the
    /// time, and a count for every other. See [`PowerTable::endless_below_zero`].
    Power {
        power: V::Power,
        amount: i32,
        target: sts2_core::CombatId,
    },
    /// `type` is a Rust keyword, so this is the one field a derive has to
    /// rename. The generated id says `"Shop"`, which is the game enum's actual
    /// name. A hand-written `"SHOP"` used to work only because the mod parses
    /// with `ignoreCase: true`.
    EnterRoom {
        #[serde(rename = "type")]
        room: V::RoomType,
    },
    /// `u8` because the game's own `MapCoord.Serialize` writes each coordinate
    /// as one byte. A number outside that is not a map point the mod refuses.
    /// It is one no map has, so serde refusing it here keeps
    /// [`Decision::Travel`] total.
    Travel {
        col: u8,
        row: u8,
    },
    Proceed,
    PlayCard {
        index: sts2_core::HandIndex,
        target: Option<sts2_core::CombatId>,
    },
    EndTurn,
    UsePotion {
        index: sts2_core::BeltIndex,
        target: Option<sts2_core::CombatId>,
    },
    ChooseOption {
        index: u8,
    },
    Buy {
        index: u8,
    },
    Grant(WireGift<V>),
    Answer(WireAnswer),
    /// No domain counterpart: the state without advancing.
    Observe,
    /// No domain counterpart: ends the session, not the run.
    Quit,
}

impl<V: sts2_content_wire::WireContentVersion> From<&Start<V>> for Command<V> {
    /// The one command that is not a decision. See [`sts2_core::Start`].
    fn from(s: &Start<V>) -> Self {
        Self::Start {
            seed: s.seed.clone(),
            character: s.character,
            ascension: s.ascension,
            modifiers: s.modifiers.clone(),
        }
    }
}

impl<V: sts2_content_wire::WireContentVersion> From<&Decision<V>> for Command<V> {
    fn from(d: &Decision<V>) -> Self {
        match d {
            Decision::EnterRoom { room } => Self::EnterRoom { room: *room },
            Decision::Travel { to } => Self::Travel {
                col: to.col,
                row: to.row,
            },
            Decision::Proceed => Self::Proceed,
            Decision::PlayCard { index, target } => Self::PlayCard {
                index: *index,
                target: *target,
            },
            Decision::EndTurn => Self::EndTurn,
            Decision::UsePotion { index, target } => Self::UsePotion {
                index: *index,
                target: *target,
            },
            Decision::ChooseOption { index } => Self::ChooseOption { index: *index },
            Decision::Buy { index } => Self::Buy { index: *index },
            Decision::Answer(a) => Self::Answer(WireAnswer(a.clone())),
        }
    }
}

impl<V: sts2_content_wire::WireContentVersion> From<&Setup<V>> for Command<V> {
    fn from(s: &Setup<V>) -> Self {
        match s {
            Setup::Fight { encounter } => Self::Fight {
                encounter: *encounter,
            },
            Setup::Event { event } => Self::Event { event: *event },
            Setup::Power { at, power, amount } => Self::Power {
                power: *power,
                amount: match amount {
                    PowerAmount::Counted(amount) => amount
                        .to_i32()
                        .expect("a power's amount fits the width the game sends"),
                    // What the game writes an endless instance down as.
                    PowerAmount::Endless => -1,
                },
                target: *at,
            },
            Setup::Grant(gift) => Self::Grant(gift.into()),
        }
    }
}

impl<V: sts2_content_wire::WireContentVersion> From<&Step<V>> for Command<V> {
    fn from(step: &Step<V>) -> Self {
        match step {
            Step::Decision(d) => d.into(),
            Step::Setup(s) => s.into(),
        }
    }
}

impl<V: sts2_content_wire::WireContentVersion> Command<V> {
    /// The run this starts, for the first line of a recording.
    ///
    /// `None` for everything else, which is every command that needs a run to
    /// already be there.
    pub fn into_start(self) -> Option<Start<V>> {
        match self {
            Self::Start {
                seed,
                character,
                ascension,
                modifiers,
            } => Some(Start {
                seed,
                character,
                ascension,
                modifiers,
            }),
            _ => None,
        }
    }

    /// The step this is, for a command read back off disk.
    ///
    /// `None` for `start`, which makes the run rather than moving it, and for
    /// `observe` and `quit`, which are not steps in a run. The daemon has
    /// always filtered the last two out of recordings, and this is that rule
    /// where the types can see it.
    pub fn into_step(self) -> Option<Step<V>> {
        Some(match self {
            // Not a step. `into_start` is what reads one.
            Self::Start { .. } => return None,
            Self::EnterRoom { room } => Decision::EnterRoom { room }.into(),
            Self::Travel { col, row } => Decision::Travel {
                to: Coord::new(col, row),
            }
            .into(),
            Self::Proceed => Decision::Proceed.into(),
            Self::PlayCard { index, target } => Decision::PlayCard { index, target }.into(),
            Self::EndTurn => Decision::EndTurn.into(),
            Self::UsePotion { index, target } => Decision::UsePotion { index, target }.into(),
            Self::ChooseOption { index } => Decision::ChooseOption { index }.into(),
            Self::Buy { index } => Decision::Buy { index }.into(),
            Self::Answer(WireAnswer(a)) => Decision::Answer(a).into(),
            Self::Fight { encounter } => Setup::Fight { encounter }.into(),
            Self::Event { event } => Setup::Event { event }.into(),
            Self::Power {
                power,
                amount,
                target,
            } => Setup::Power {
                at: target,
                power,
                amount: match amount < 0 && power.endless_below_zero() {
                    true => PowerAmount::Endless,
                    false => {
                        PowerAmount::Counted(num_traits::NumCast::from(amount).unwrap_or_else(
                            || unimplemented!("a power amount this build cannot hold"),
                        ))
                    }
                },
            }
            .into(),
            Self::Grant(gift) => Setup::Grant(gift.into_domain()).into(),
            Self::Observe | Self::Quit => return None,
        })
    }
}

/// An [`Answer`] in the protocol's shape.
///
/// A newtype because the serde is hand-written on both sides and the domain
/// type has none. All three forms share the `answer` command and are told apart
/// by which field they carry, which no derive spells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireAnswer(pub Answer);

impl Serialize for WireAnswer {
    /// Written by hand because `skip` is the literal `true` rather than a value
    /// the caller supplies. Deriving it needs a `bool` field that can be set to
    /// `false`, which is not an answer the game accepts.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        match &self.0 {
            Answer::Cards(cards) => map.serialize_entry("cards", cards)?,
            Answer::Skip => map.serialize_entry("skip", &true)?,
            Answer::Take(take) => {
                let wire: Vec<WireTake> = take.iter().map(WireTake::from).collect();
                map.serialize_entry("take", &wire)?;
            }
            Answer::Divine { x, y, tool } => map.serialize_entry(
                "divine",
                &WireDivine {
                    x: *x,
                    y: *y,
                    tool: (*tool).into(),
                },
            )?,
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for WireAnswer {
    /// Told apart by which field is present, the way the game tells them apart.
    ///
    /// `skip` is accepted only as `true`. `{"skip": false}` is not an answer,
    /// and reading it as one silently turns a malformed recording into a
    /// different run.
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct OneField;

        impl<'de> Visitor<'de> for OneField {
            type Value = WireAnswer;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an answer carrying one of 'cards', 'skip', 'take' or 'divine'")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<WireAnswer, M::Error> {
                let mut answer: Option<Answer> = None;

                while let Some(key) = map.next_key::<String>()? {
                    let found = match key.as_str() {
                        "cards" => Answer::Cards(map.next_value()?),
                        "take" => Answer::Take(
                            map.next_value::<Vec<WireTake>>()?
                                .into_iter()
                                .map(WireTake::into_domain)
                                .collect(),
                        ),
                        "divine" => {
                            let WireDivine { x, y, tool } = map.next_value()?;
                            Answer::Divine {
                                x,
                                y,
                                tool: tool.into(),
                            }
                        }
                        "skip" => match map.next_value::<bool>()? {
                            true => Answer::Skip,
                            false => {
                                return Err(serde::de::Error::custom(
                                    "'skip' is only an answer when it is true",
                                ));
                            }
                        },
                        other => {
                            return Err(serde::de::Error::unknown_field(
                                other,
                                &["cards", "skip", "take", "divine"],
                            ));
                        }
                    };

                    if answer.is_some() {
                        return Err(serde::de::Error::custom(
                            "an answer carries exactly one of 'cards', 'skip', 'take' and 'divine'",
                        ));
                    }
                    answer = Some(found);
                }

                answer.map(WireAnswer).ok_or_else(|| {
                    serde::de::Error::custom(
                        "an answer needs one of 'cards', 'skip', 'take' or 'divine'",
                    )
                })
            }
        }

        deserializer.deserialize_map(OneField)
    }
}

/// An [`Answer::Divine`] on the wire.
///
/// A struct, not a mirror enum: the three fields are one move, and none of them
/// is optional. [`Tool`] spells itself as the game's own enum member, which is
/// what `SetTool` is given.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WireDivine {
    x: u8,
    y: u8,
    tool: WireTool,
}

/// `CrystalSphereToolType` has a third member the mod can report and no answer
/// can name.
///
/// `_game.CrystalSphereTool.ToString()` writes whatever the game holds, and the
/// game starts a sphere at `None`. [`Tool`] deliberately has two members.
/// `CellClicked` tests only for `Big`, so `None` and `Small` are one move under
/// two names. A reported `None` reads back as `Small`, which is the move it
/// is.
impl From<WireReportedTool> for Tool {
    fn from(t: WireReportedTool) -> Self {
        match t {
            WireReportedTool::None | WireReportedTool::Small => Self::Small,
            WireReportedTool::Big => Self::Big,
        }
    }
}

/// The tool as the mod *reports* it, which is a wider set than the tool an
/// answer can *name*.
#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) enum WireReportedTool {
    None,
    Small,
    Big,
}

/// A [`Tool`] on the wire, spelled as the game's own enum member.
///
/// `CrystalSphereToolType.ToString()` is what the mod reports and what it parses
/// back, so these are its names and not a spelling of our own.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) enum WireTool {
    Small,
    Big,
}

impl From<Tool> for WireTool {
    fn from(t: Tool) -> Self {
        match t {
            Tool::Small => Self::Small,
            Tool::Big => Self::Big,
        }
    }
}

impl From<WireTool> for Tool {
    fn from(t: WireTool) -> Self {
        match t {
            WireTool::Small => Self::Small,
            WireTool::Big => Self::Big,
        }
    }
}

/// A [`TakeReward`] on the wire.
///
/// Untagged, so each variant serializes flat and a card reward is the only one
/// that carries a `card`. That is exactly what the protocol reads. `Pending`
/// refuses a card reward with no `card` index, and ignores the field on every
/// other kind of reward.
///
/// `Card` comes first because untagged tries the variants in order, and a
/// `NonCard` also accepts an object that carries a card.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum WireTake {
    Card { index: usize, card: usize },
    NonCard { index: usize },
}

impl From<&TakeReward> for WireTake {
    fn from(t: &TakeReward) -> Self {
        match t {
            TakeReward::Card { index, card } => Self::Card {
                index: *index,
                card: *card,
            },
            TakeReward::NonCard { index } => Self::NonCard { index: *index },
        }
    }
}

impl WireTake {
    fn into_domain(self) -> TakeReward {
        match self {
            Self::Card { index, card } => TakeReward::Card { index, card },
            Self::NonCard { index } => TakeReward::NonCard { index },
        }
    }
}

/// A [`Gift`] on the wire.
///
/// Untagged, so the one variant a [`Gift`] is becomes the one key the mod
/// reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, bound = "")]
pub enum WireGift<V: sts2_content_wire::WireContentVersion> {
    Card { card: V::Card },
    Relic { relic: V::Relic },
    Potion { potion: V::Potion },
}

impl<V: sts2_content_wire::WireContentVersion> From<&Gift<V>> for WireGift<V> {
    fn from(g: &Gift<V>) -> Self {
        match g {
            Gift::Card(card) => Self::Card { card: *card },
            Gift::Relic(relic) => Self::Relic { relic: *relic },
            Gift::Potion(potion) => Self::Potion { potion: *potion },
        }
    }
}

impl<V: sts2_content_wire::WireContentVersion> WireGift<V> {
    fn into_domain(self) -> Gift<V> {
        match self {
            Self::Card { card } => Gift::Card(card),
            Self::Relic { relic } => Gift::Relic(relic),
            Self::Potion { potion } => Gift::Potion(potion),
        }
    }
}

#[cfg(test)]
mod tests {
    use sts2_core::fixture::{at, belt, hand};

    use super::*;
    use serde_json::{json, to_value};
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_content::v0_107_1::cards::IroncladCard;

    type V = V0_107_1;
    type Wire = Command<V>;
    type AscensionLevel = <V as sts2_core::ContentVersion>::AscensionLevel;
    type Character = <V as sts2_core::ContentVersion>::Character;
    type Card = <V as sts2_core::ContentVersion>::Card;
    type Encounter = <V as sts2_core::ContentVersion>::Encounter;
    type Event = <V as sts2_core::ContentVersion>::Event;
    type Modifier = <V as sts2_core::ContentVersion>::Modifier;
    type Potion = <V as sts2_core::ContentVersion>::Potion;
    type Relic = <V as sts2_core::ContentVersion>::Relic;
    type RoomType = <V as sts2_core::ContentVersion>::RoomType;

    fn wire(step: impl Into<Step<V>>) -> serde_json::Value {
        let step = step.into();
        to_value(Wire::from(&step)).expect("a step serializes")
    }

    /// The one command that is not a step. See [`sts2_core::Start`].
    fn wire_start(start: &Start<V>) -> serde_json::Value {
        to_value(Wire::from(start)).expect("a start serializes")
    }

    #[test]
    fn the_typed_ids_reach_the_wire_as_the_games_own_spellings() {
        assert_eq!(
            wire_start(&Start {
                seed: Seed::new("ABC").expect("a seed"),
                character: Character::Silent,
                ascension: AscensionLevel::AscendersBane,
                modifiers: vec![Modifier::Draft],
            }),
            json!({
                "cmd": "start", "ascension": "AscendersBane", "seed": "ABC",
                "character": "SILENT", "modifiers": ["DRAFT"],
            })
        );
    }

    #[test]
    fn a_decision_with_no_arguments_is_just_its_name() {
        assert_eq!(wire(Decision::EndTurn), json!({"cmd": "end_turn"}));
        assert_eq!(wire(Decision::Proceed), json!({"cmd": "proceed"}));
    }

    /// `type` is a Rust keyword, which is why the domain field is called `room`
    /// and only the wire says `type`. Exactly the sort of thing that has no
    /// business being visible in `sts2-core`.
    #[test]
    fn enter_room_calls_it_type_on_the_wire_only() {
        assert_eq!(
            wire(Decision::EnterRoom {
                room: RoomType::Shop
            }),
            json!({"cmd": "enter_room", "type": "Shop"})
        );
    }

    #[test]
    fn an_untargeted_card_sends_a_null_target() {
        assert_eq!(
            wire(Decision::PlayCard {
                index: hand(2),
                target: None
            }),
            json!({"cmd": "play_card", "index": 2, "target": null})
        );
    }

    /// Spelled the way `play_card` is, because it is the same shape of move:
    /// an index into something the observation listed, and a creature id.
    #[test]
    fn a_potion_is_addressed_like_a_card() {
        assert_eq!(
            wire(Decision::UsePotion {
                index: belt(1),
                target: Some(at(4)),
            }),
            json!({"cmd": "use_potion", "index": 1, "target": 4})
        );
        assert_eq!(
            wire(Decision::UsePotion {
                index: belt(0),
                target: None,
            }),
            json!({"cmd": "use_potion", "index": 0, "target": null})
        );
    }

    #[test]
    fn each_answer_carries_only_its_own_field() {
        assert_eq!(
            wire(Decision::Answer(Answer::Cards(vec![0, 2]))),
            json!({"cmd": "answer", "cards": [0, 2]})
        );
        assert_eq!(
            wire(Decision::Answer(Answer::Skip)),
            json!({"cmd": "answer", "skip": true})
        );
        assert_eq!(
            wire(Decision::Answer(Answer::Take(vec![TakeReward::Card {
                index: 1,
                card: 0,
            }]))),
            json!({"cmd": "answer", "take": [{"index": 1, "card": 0}]})
        );
    }

    /// Each gift is one key, and the key names which gift it is.
    #[test]
    fn a_grant_carries_exactly_one_key() {
        assert_eq!(
            wire(Setup::Grant(Gift::Relic(Relic::Vajra))),
            json!({"cmd": "grant", "relic": "VAJRA"})
        );
        assert_eq!(
            wire(Setup::Grant(Gift::Card(Card::Ironclad(
                IroncladCard::Strike
            )))),
            json!({"cmd": "grant", "card": "STRIKE_IRONCLAD"})
        );
        assert_eq!(
            wire(Setup::Grant(Gift::Potion(Potion::FirePotion))),
            json!({"cmd": "grant", "potion": "FIRE_POTION"})
        );
    }

    /// Everything that can be sent can be read back as the step it was, or a
    /// recording is a write-only file.
    #[test]
    fn every_step_round_trips_through_the_wire_and_back() {
        let steps: Vec<Step<V>> = vec![
            Decision::EnterRoom {
                room: RoomType::Shop,
            }
            .into(),
            Decision::Travel {
                to: Coord::new(1, 1),
            }
            .into(),
            Decision::PlayCard {
                index: hand(3),
                target: Some(at(1)),
            }
            .into(),
            Decision::PlayCard {
                index: hand(0),
                target: None,
            }
            .into(),
            Decision::EndTurn.into(),
            Decision::UsePotion {
                index: belt(1),
                target: Some(at(4)),
            }
            .into(),
            Decision::UsePotion {
                index: belt(0),
                target: None,
            }
            .into(),
            Decision::Proceed.into(),
            Decision::ChooseOption { index: 2 }.into(),
            Decision::Buy { index: 0 }.into(),
            Decision::Answer(Answer::Cards(vec![0, 2])).into(),
            Decision::Answer(Answer::Skip).into(),
            Decision::Answer(Answer::Take(vec![TakeReward::Card { index: 1, card: 0 }])).into(),
            Decision::Answer(Answer::Take(vec![TakeReward::NonCard { index: 2 }])).into(),
            Setup::Fight {
                encounter: Encounter::ShrinkerBeetleWeak,
            }
            .into(),
            Setup::Grant(Gift::Relic(Relic::Vajra)).into(),
            Setup::Grant(Gift::Card(Card::Ironclad(IroncladCard::Strike))).into(),
            Setup::Grant(Gift::Potion(Potion::FirePotion)).into(),
            Setup::Event {
                event: Event::CrystalSphere,
            }
            .into(),
            Decision::Answer(Answer::Divine {
                x: 3,
                y: 7,
                tool: Tool::Small,
            })
            .into(),
            Decision::Answer(Answer::Divine {
                x: 0,
                y: 0,
                tool: Tool::Big,
            })
            .into(),
        ];

        for step in steps {
            let sent = serde_json::to_string(&Wire::from(&step)).expect("a step serializes");
            let back: Wire = serde_json::from_str(&sent).unwrap_or_else(|e| {
                panic!("{sent} does not read back: {e}");
            });
            assert_eq!(back.into_step(), Some(step), "{sent}");
        }
    }

    /// Both parse, and neither is a step in a run.
    #[test]
    fn observe_and_quit_are_commands_but_not_steps() {
        for line in [r#"{"cmd":"observe"}"#, r#"{"cmd":"quit"}"#] {
            let command: Wire = serde_json::from_str(line).expect("the protocol has these");
            assert!(command.into_step().is_none(), "{line}");
        }
    }

    /// A reward that is not a card reward carries no `card` at all.
    #[test]
    fn only_a_card_reward_carries_a_card() {
        assert_eq!(
            wire(Decision::Answer(Answer::Take(vec![TakeReward::NonCard {
                index: 2
            }]))),
            json!({"cmd": "answer", "take": [{"index": 2}]})
        );
    }

    /// Recordings written before the split carry the explicit null. Untagged
    /// falls through to `NonCard`, which ignores it.
    #[test]
    fn an_older_recording_with_an_explicit_null_still_reads() {
        let line = r#"{"cmd":"answer","take":[{"index":2,"card":null}]}"#;
        let back: Wire = serde_json::from_str(line).expect("an older file still opens");
        assert_eq!(
            back.into_step(),
            Some(Decision::Answer(Answer::Take(vec![TakeReward::NonCard { index: 2 }])).into())
        );
    }

    /// `{"skip": false}` is not "do not skip", it is not an answer at all.
    #[test]
    fn skip_is_only_an_answer_when_it_is_true() {
        assert!(serde_json::from_str::<Wire>(r#"{"cmd":"answer","skip":false}"#).is_err());
        assert!(serde_json::from_str::<Wire>(r#"{"cmd":"answer"}"#).is_err());
        assert!(
            serde_json::from_str::<Wire>(r#"{"cmd":"answer","skip":true,"cards":[0]}"#).is_err(),
            "two answers in one is not one of them"
        );
    }
}

/// A [`Seed`] on the wire.
///
/// `Seed` carries no serde of its own -- `sts2-core` has none at all -- so the
/// spelling is here with every other spelling.
///
/// Reads through `Seed::new`, not `Seed::fold`: whatever wrote the file already
/// held a `Seed`, so `"abcdef"` in a recording is a broken file.
mod seed {
    use serde::{Deserialize, Deserializer, Serializer};
    use sts2_core::Seed;

    pub fn serialize<S: Serializer>(seed: &Seed, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(seed.as_str())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Seed, D::Error> {
        let typed = <std::borrow::Cow<'_, str> as Deserialize>::deserialize(d)?;
        Seed::new(&typed).map_err(serde::de::Error::custom)
    }
}
