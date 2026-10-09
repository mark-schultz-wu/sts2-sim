//! A refused command, as the oracle spells it.
//!
//! `{"type":"error","cmd":…,"reason":{"kind":…,…}}`, and the reason is a tagged
//! object rather than a sentence. The English is [`sts2_core::Reason`]'s
//! `Display`, written once, from the data. `sts2-core`'s README says why.
//!
//! There is no `message` beside the reason. Every field in one is a fact the
//! client already had, out of the observation it was acting on. That is what
//! makes a refusal checkable rather than merely informative.
//!
//! [`WireFault`] is the mod's catch-all: an exception inside the adapter, which
//! says nothing about whether the move was legal. It becomes an
//! [`crate::Error`] rather than a `Reason`, because a simulator cannot be
//! expected to reproduce someone else's stack trace.

use serde::Deserialize;
use sts2_core::{FlagSet, QuestionKind, Reason, Refusal};

use super::ident;
use super::observation::{Error, blocked, ident as read_ident, name, reasons, unknown};

/// The envelope a refusal arrives in.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireRefusal {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    kind: Discriminator,
    cmd: String,
    reason: WireReason,
}

/// Present so the envelope can be strict about a field it already routed on.
///
/// [`crate::parse_reply`] reads `type` off the raw value to decide which struct
/// to build. Without a field to receive it, `deny_unknown_fields` refuses every
/// refusal.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Discriminator {
    Error,
}

/// The reason, with every id still a string.
///
/// The same split as [`crate::wire::question`]. This is the spelling, and
/// [`sts2_core::Reason`] is the shape. A second implementation does not have to
/// agree with the mod's JSON to be comparable to it.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum WireReason {
    // Eleven of these carry nothing, and are written `{}` rather than as unit
    // variants on purpose: `deny_unknown_fields` refuses a key against a
    // variant's field list, and a unit variant has none, so it lets anything
    // past. A mod that grows a field on a reason this client already knows is
    // exactly the drift that would otherwise be dropped in silence.
    NoRun {},
    RunOver {
        victory: bool,
    },
    NotInCombat {},
    InCombat {},
    WrongRoom {
        wanted: String,
        found: Option<String>,
    },
    NothingToChoose {
        found: Option<String>,
    },
    /// `asking` and not `kind`, which the tag has already spent.
    Pending {
        asking: String,
    },
    NothingPending {},
    OutOfRange {
        index: i64,
        count: usize,
    },
    NoSuchCreature {
        id: sts2_core::CombatId,
    },
    Unplayable {
        /// The packed spelling, as a card in hand is written.
        card: String,
        /// The `[Flags]` members joined with `", "`, or null for none of them.
        why: Option<String>,
        preventer: Option<String>,
    },
    WrongTarget {
        card: String,
        target: Option<sts2_core::CombatId>,
        takes: String,
    },
    Undrinkable {
        potion: String,
        reason: sts2_core::Undrinkable,
    },
    WrongPotionTarget {
        potion: String,
        target: Option<sts2_core::CombatId>,
        takes: String,
    },
    NotTravelable {
        at: String,
    },
    CannotProceed {},
    OptionDisabled {},
    OptionLocked {},
    AlreadyAnswered {},
    Unpurchasable {
        index: u8,
        cost: i32,
        gold: i32,
        stocked: bool,
        blocked: Option<String>,
    },
    Duplicate {
        index: usize,
    },
    WrongCount {
        chosen: usize,
        min: u32,
        max: u32,
    },
    NotSkippable {},
    AlreadyTaken {
        index: usize,
    },
    NeedsCardChoice {
        index: usize,
        count: usize,
    },
    EmptyTake {},
    OffGrid {
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    },
    AlreadyUncovered {
        x: usize,
        y: usize,
    },
    Console {
        message: String,
    },
    Malformed {
        message: String,
    },
}

impl WireRefusal {
    /// The refusal this spells.
    ///
    /// The path a failure carries starts at `reason`, the way an observation's
    /// starts at `observation`. `reason.card` is actionable, and "a card
    /// identity did not parse" is not.
    pub(crate) fn into_domain<V: sts2_content_wire::WireContentVersion>(
        self,
    ) -> Result<Refusal<V>, Error> {
        Ok(Refusal {
            what: self.cmd,
            reason: self.reason.into_domain::<V>()?,
        })
    }
}

impl WireReason {
    fn into_domain<V: sts2_content_wire::WireContentVersion>(self) -> Result<Reason<V>, Error> {
        Ok(match self {
            WireReason::NoRun {} => Reason::NoRun,
            WireReason::RunOver { victory } => Reason::RunOver { victory },
            WireReason::NotInCombat {} => Reason::NotInCombat,
            WireReason::InCombat {} => Reason::InCombat,
            WireReason::WrongRoom { wanted, found } => Reason::WrongRoom {
                wanted: name(&p("wanted"), "room type", &wanted)?,
                found: room::<V>(found)?,
            },
            WireReason::NothingToChoose { found } => Reason::NothingToChoose {
                found: room::<V>(found)?,
            },
            WireReason::Pending { asking } => Reason::Pending {
                kind: match asking.as_str() {
                    "select_cards" => QuestionKind::SelectCards,
                    "rewards" => QuestionKind::Rewards,
                    "divine" => QuestionKind::Divine,
                    "choose_bundle" => QuestionKind::ChooseBundle,
                    "confirm" => QuestionKind::Confirm,
                    other => return Err(unknown(&p("asking"), "kind of question", other)),
                },
            },
            WireReason::NothingPending {} => Reason::NothingPending,

            WireReason::OutOfRange { index, count } => Reason::OutOfRange { index, count },
            WireReason::NoSuchCreature { id } => Reason::NoSuchCreature { id },

            WireReason::Unplayable {
                card,
                why,
                preventer,
            } => Reason::Unplayable {
                card: read_ident(&p("card"), &card, ident::card_ident::<V>)?,
                // Null is the empty set, which is what the oracle writes for
                // no members -- the same reading the observation gives the same
                // field. `reasons` is never handed the empty string, which
                // splits into one nameless member rather than none.
                why: match why {
                    Some(text) => reasons::<V>(&p("why"), &text)?,
                    None => V::UnplayableReasons::from_members(&[]),
                },
                preventer: preventer
                    .map(|text| read_ident(&p("preventer"), &text, ident::model_id::<V>))
                    .transpose()?,
            },
            WireReason::WrongTarget {
                card,
                target,
                takes,
            } => Reason::WrongTarget {
                card: read_ident(&p("card"), &card, ident::card_ident::<V>)?,
                target,
                takes: name(&p("takes"), "target type", &takes)?,
            },
            WireReason::Undrinkable { potion, reason } => Reason::Undrinkable {
                potion: name(&p("potion"), "potion", &potion)?,
                why: reason,
            },
            WireReason::WrongPotionTarget {
                potion,
                target,
                takes,
            } => Reason::WrongPotionTarget {
                potion: name(&p("potion"), "potion", &potion)?,
                target,
                takes: name(&p("takes"), "target type", &takes)?,
            },
            WireReason::NotTravelable { at } => Reason::NotTravelable {
                at: read_ident(&p("at"), &at, ident::coord)?,
            },
            WireReason::CannotProceed {} => Reason::CannotProceed,
            WireReason::OptionDisabled {} => Reason::OptionDisabled,
            WireReason::OptionLocked {} => Reason::OptionLocked,
            WireReason::AlreadyAnswered {} => Reason::AlreadyAnswered,
            WireReason::Unpurchasable {
                index,
                cost,
                gold,
                stocked,
                blocked: why,
            } => Reason::Unpurchasable {
                index,
                cost,
                gold,
                stocked,
                blocked: blocked(&p("blocked"), why.as_deref())?,
            },

            WireReason::Duplicate { index } => Reason::Duplicate { index },
            WireReason::WrongCount { chosen, min, max } => Reason::WrongCount { chosen, min, max },
            WireReason::NotSkippable {} => Reason::NotSkippable,
            WireReason::AlreadyTaken { index } => Reason::AlreadyTaken { index },
            WireReason::NeedsCardChoice { index, count } => {
                Reason::NeedsCardChoice { index, count }
            }
            WireReason::EmptyTake {} => Reason::EmptyTake,
            WireReason::OffGrid {
                x,
                y,
                width,
                height,
            } => Reason::OffGrid {
                x,
                y,
                width,
                height,
            },
            WireReason::AlreadyUncovered { x, y } => Reason::AlreadyUncovered { x, y },

            WireReason::Console { message } => Reason::Console { message },
            WireReason::Malformed { message } => Reason::Malformed { message },
        })
    }
}

/// A room type the protocol names, or nothing.
///
/// Null for a room this protocol has no `RoomType` for, such as a combat room
/// or no room at all. That is a fact, and not a failure to read one.
fn room<V: sts2_content_wire::WireContentVersion>(
    found: Option<String>,
) -> Result<Option<V::RoomType>, Error> {
    found
        .map(|text| name(&p("found"), "room type", &text))
        .transpose()
}

/// A dotted path into the reason, for the failure to carry.
fn p(field: &str) -> String {
    format!("reason.{field}")
}

/// The adapter fell over, which says nothing about whether the move was legal.
///
/// Its own message type on the wire, so it cannot be mistaken for one. A
/// refusal is the game's answer, and this is the mod's failure to ask it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireFault {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    kind: FaultDiscriminator,
    pub(crate) cmd: String,
    /// The exception's type name.
    pub(crate) error: String,
    pub(crate) message: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum FaultDiscriminator {
    Fault,
}

#[cfg(test)]
mod tests {
    use sts2_core::fixture::at;

    use super::*;
    use serde_json::json;
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_core::{Blocked, Coord, ModelId};

    type V = V0_107_1;
    type RoomType = <V as sts2_core::ContentVersion>::RoomType;
    type TargetType = <V as sts2_core::ContentVersion>::TargetType;
    type Reasons = <V as sts2_core::ContentVersion>::UnplayableReasons;
    type UnplayableReason = <V as sts2_core::ContentVersion>::UnplayableReason;

    fn read(cmd: &str, reason: serde_json::Value) -> Result<Refusal<V>, Error> {
        let wire: WireRefusal =
            serde_json::from_value(json!({"type": "error", "cmd": cmd, "reason": reason}))
                .expect("the sample should parse");
        wire.into_domain::<V>()
    }

    fn reason(reason: serde_json::Value) -> Reason<V> {
        read("play_card", reason).expect("should convert").reason
    }

    /// A card by the name this build gives it.
    ///
    /// Serialized rather than parsed, so the assertion does not lean on the
    /// same reader it is checking.
    fn spelled(card: &<V as sts2_core::ContentVersion>::Card) -> String {
        serde_json::to_value(card)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .expect("a card serializes as its name")
    }

    /// A model id by the packing the observation uses, so a preventer is
    /// compared against one name rather than against a nested constructor.
    fn model(packed: &str) -> ModelId<V> {
        ident::model_id::<V>(packed).expect("should be a model id")
    }

    #[test]
    fn the_command_it_refused_comes_off_the_envelope() {
        let refusal = read("travel", json!({"kind": "no_run"})).expect("should convert");
        assert_eq!(refusal.what, "travel");
        assert_eq!(refusal.reason, Reason::NoRun);
    }

    /// A reason with no fields is still an object with a `kind`, not a bare
    /// string. The tag is the whole message for eleven of them.
    #[test]
    fn a_reason_with_no_fields_is_still_tagged() {
        assert_eq!(
            reason(json!({"kind": "cannot_proceed"})),
            Reason::CannotProceed
        );
        assert_eq!(reason(json!({"kind": "empty_take"})), Reason::EmptyTake);
        assert_eq!(
            reason(json!({"kind": "nothing_pending"})),
            Reason::NothingPending
        );
    }

    #[test]
    fn a_card_is_read_the_way_a_card_in_hand_is() {
        let Reason::Unplayable {
            card,
            why,
            preventer,
        } = reason(json!({
            "kind": "unplayable",
            "card": "BASH+1",
            "why": "EnergyCostTooHigh, BlockedByHook",
            "preventer": "RELIC/BURNING_BLOOD",
        }))
        else {
            panic!("expected unplayable");
        };

        // The packing, read apart: `card_ident` is what split `BASH+1` into a
        // card and an upgrade, and both halves are what this is about.
        assert_eq!(spelled(&card.card), "BASH");
        assert_eq!(card.upgrade, 1);
        assert_eq!(preventer, Some(model("RELIC/BURNING_BLOOD")));
        assert!(why.contains(UnplayableReason::EnergyCostTooHigh));
        assert!(why.contains(UnplayableReason::BlockedByHook));
    }

    /// The oracle writes null where the observation writes null: no members,
    /// not a member named `None`.
    #[test]
    fn no_reason_is_the_empty_set() {
        let Reason::Unplayable { why, .. } = reason(json!({
            "kind": "unplayable", "card": "STRIKE_IRONCLAD", "why": null, "preventer": null,
        })) else {
            panic!("expected unplayable");
        };
        assert_eq!(why, Reasons::from_members(&[]));
    }

    #[test]
    fn a_room_the_protocol_does_not_name_is_a_fact_not_a_failure() {
        assert_eq!(
            reason(json!({"kind": "wrong_room", "wanted": "Shop", "found": null})),
            Reason::WrongRoom {
                wanted: RoomType::Shop,
                found: None,
            }
        );
        assert_eq!(
            reason(json!({"kind": "nothing_to_choose", "found": "RestSite"})),
            Reason::NothingToChoose {
                found: Some(RoomType::RestSite),
            }
        );
    }

    #[test]
    fn a_coordinate_is_read_the_way_the_map_writes_one() {
        assert_eq!(
            reason(json!({"kind": "not_travelable", "at": "3,7"})),
            Reason::NotTravelable {
                at: Coord { col: 3, row: 7 },
            }
        );
    }

    #[test]
    fn a_purchase_carries_every_number_it_turned_on() {
        assert_eq!(
            reason(json!({
                "kind": "unpurchasable", "index": 2, "cost": 75, "gold": 30,
                "stocked": true, "blocked": "no potion slot",
            })),
            Reason::Unpurchasable {
                index: 2,
                cost: 75,
                gold: 30,
                stocked: true,
                blocked: Some(Blocked::NoPotionSlot),
            }
        );
    }

    #[test]
    fn a_target_type_is_a_name_this_build_has() {
        let Reason::WrongTarget {
            card,
            target,
            takes,
        } = reason(json!({
            "kind": "wrong_target", "card": "STRIKE_IRONCLAD", "target": 4,
            "takes": "AnyEnemy",
        }))
        else {
            panic!("expected wrong_target");
        };
        assert_eq!(spelled(&card.card), "STRIKE_IRONCLAD");
        assert_eq!(card.upgrade, 0);
        assert_eq!(target, Some(at(4)));
        assert_eq!(takes, TargetType::AnyEnemy);

        let err = read(
            "play_card",
            json!({
                "kind": "wrong_target", "card": "STRIKE_IRONCLAD", "target": null,
                "takes": "SingleEnemy",
            }),
        )
        .expect_err("SingleEnemy is no target type this build has");
        assert_eq!(err.at, "reason.takes");
    }

    #[test]
    fn the_question_a_pending_names_is_one_of_three() {
        assert_eq!(
            reason(json!({"kind": "pending", "asking": "rewards"})),
            Reason::Pending {
                kind: QuestionKind::Rewards,
            }
        );

        let err = read("answer", json!({"kind": "pending", "asking": "haggle"}))
            .expect_err("should refuse");
        assert_eq!(err.at, "reason.asking");
    }

    /// The path is the point of reading a refusal at all: `reason.card` is
    /// actionable and "a card identity did not parse" is not.
    #[test]
    fn a_name_this_build_lacks_carries_where_it_was() {
        let err = read(
            "play_card",
            json!({"kind": "wrong_room", "wanted": "Casino", "found": null}),
        )
        .expect_err("should refuse");
        assert_eq!(err.at, "reason.wanted");
        assert!(err.to_string().contains("Casino"), "unhelpful: {err}");

        // And in `found`, where null is already a legitimate answer -- so a
        // name this build lacks must not be quietly folded into it.
        let err = read(
            "buy",
            json!({"kind": "nothing_to_choose", "found": "Casino"}),
        )
        .expect_err("should refuse");
        assert_eq!(err.at, "reason.found");
    }

    /// A kind this client does not know is a divergence, not something to
    /// degrade past: the mod grew a reason and this side did not.
    #[test]
    fn a_kind_this_client_does_not_know_is_refused() {
        let err = serde_json::from_value::<WireRefusal>(json!({
            "type": "error", "cmd": "buy", "reason": {"kind": "too_expensive"},
        }))
        .expect_err("should refuse");
        assert!(
            err.to_string().contains("too_expensive"),
            "unhelpful: {err}"
        );
    }

    /// A field the client does not know is the same divergence seen from the
    /// other side, and `deny_unknown_fields` is what catches it.
    #[test]
    fn a_field_this_client_does_not_know_is_refused() {
        let err = serde_json::from_value::<WireRefusal>(json!({
            "type": "error", "cmd": "buy",
            "reason": {"kind": "empty_take", "because": "reasons"},
        }))
        .expect_err("should refuse");
        assert!(err.to_string().contains("because"), "unhelpful: {err}");
    }

    /// The sentence is gone. A `message` beside the reason is a second spelling
    /// of one fact, free to disagree with the first.
    #[test]
    fn a_message_beside_the_reason_is_refused() {
        serde_json::from_value::<WireRefusal>(json!({
            "type": "error", "cmd": "buy", "message": "nope",
            "reason": {"kind": "empty_take"},
        }))
        .expect_err("should refuse");
    }

    #[test]
    fn a_fault_is_its_own_message() {
        let fault: WireFault = serde_json::from_value(json!({
            "type": "fault", "cmd": "play_card",
            "error": "NullReferenceException", "message": "Object reference not set",
        }))
        .expect("should parse");
        assert_eq!(fault.cmd, "play_card");
        assert_eq!(fault.error, "NullReferenceException");
    }
}
