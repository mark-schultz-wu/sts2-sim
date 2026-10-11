//! A parked game's question, as the oracle spells it.
//!
//! The same split as [`crate::wire::observation`], and for the same reasons.
//! [`sts2_core::Question`] is the shape and this is the spelling. A second
//! implementation does not have to agree with the mod's JSON to be comparable
//! to it. A card is `"BASH+1/SMOG"` on the wire and a tool is `"Big"`. A reward
//! names a C# class beside the fields that class happens to fill in.
//!
//! # The reward is a sum wearing a struct
//!
//! `type` selects which of the remaining fields mean anything, which is the
//! defect this crate keeps finding. [`WireReward`] is the flat form and is
//! converted immediately, so nothing downstream can hold a `GoldReward` with a
//! relic in it.

use serde::Deserialize;
use sts2_core::{
    BundleOption, CardOption, Cell, FoundItem, Question, RelicTable, RewardCard, RewardDetail,
    RewardKind,
};

use super::command::WireReportedTool;
use super::ident::{self, named};
use super::observation::{Error, Why, at, ident as read_ident};

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum WireQuestion {
    SelectCards {
        min: u32,
        max: u32,
        asker: Option<String>,
        options: Vec<WireCardOption>,
        /// The options picked so far, in the order they were picked.
        picked: Vec<usize>,
    },
    Rewards {
        skippable: bool,
        rewards: Vec<WireRewardDetail>,
    },
    Divine {
        left: u32,
        width: u8,
        height: u8,
        tool: WireReportedTool,
        cleared: Vec<WireCell>,
        revealed: Vec<WireFoundItem>,
    },
    ChooseBundle {
        bundles: Vec<WireBundle>,
    },
    Confirm {},
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireBundle {
    index: usize,
    cards: Vec<WireRewardCard>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireCardOption {
    index: usize,
    /// Entry name, plus `+N` when upgraded, plus `/AFFLICTION`, plus
    /// `*ENCHANTMENT`.
    card: String,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireCell {
    x: u8,
    y: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireFoundItem {
    item: String,
    good: bool,
    x: u8,
    y: u8,
    width: u8,
    height: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireRewardCard {
    card: String,
    #[serde(default)]
    text: Option<String>,
}

/// The flat form the game sends, with `type` deciding which fields count.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireRewardDetail {
    index: usize,
    #[serde(rename = "type")]
    kind: String,
    populated: bool,
    #[serde(default)]
    taken: bool,
    #[serde(default)]
    amount: Option<i64>,
    #[serde(default)]
    cards: Option<Vec<WireRewardCard>>,
    /// A card reward's alternatives other than `Skip`, by `OptionId`. Absent
    /// where it has none.
    #[serde(default)]
    alternatives: Vec<String>,
    #[serde(default)]
    relic: Option<String>,
    #[serde(default)]
    rarity: Option<String>,
    #[serde(default)]
    potion: Option<String>,
}

impl WireQuestion {
    /// The question this spells.
    ///
    /// The path a failure carries starts at `question`, the way an
    /// observation's starts at `observation`. `question.options[3].card` is
    /// actionable, and "a card identity did not parse" is not.
    pub(crate) fn into_domain<V: sts2_content_wire::WireContentVersion>(
        self,
    ) -> Result<Question<V>, Error> {
        Ok(match self {
            WireQuestion::SelectCards {
                min,
                max,
                asker,
                options,
                picked,
            } => Question::SelectCards {
                min,
                max,
                asker,
                picked: sts2_core::Picks::try_from(&picked[..]).map_err(|why| {
                    at(
                        "question.picked",
                        Why::OutOfRange {
                            what: "a selection's picks",
                            value: why.to_string(),
                        },
                    )
                })?,
                options: options
                    .into_iter()
                    .enumerate()
                    .map(|(i, o)| {
                        Ok(CardOption {
                            index: o.index,
                            card: read_ident(
                                &format!("question.options[{i}].card"),
                                &o.card,
                                ident::card_ident::<V>,
                            )?,
                            text: o.text,
                        })
                    })
                    .collect::<Result<_, Error>>()?,
            },

            WireQuestion::Rewards { skippable, rewards } => Question::Rewards {
                skippable,
                rewards: rewards
                    .into_iter()
                    .enumerate()
                    .map(|(i, r)| r.into_domain(&format!("question.rewards[{i}]")))
                    .collect::<Result<_, Error>>()?,
            },

            WireQuestion::Divine {
                left,
                width,
                height,
                tool,
                cleared,
                revealed,
            } => Question::Divine {
                left,
                width,
                height,
                tool: tool.into(),
                cleared: cleared
                    .into_iter()
                    .map(|c| Cell { x: c.x, y: c.y })
                    .collect(),
                revealed: revealed
                    .into_iter()
                    .map(|f| FoundItem {
                        item: f.item,
                        good: f.good,
                        x: f.x,
                        y: f.y,
                        width: f.width,
                        height: f.height,
                    })
                    .collect(),
            },

            WireQuestion::Confirm {} => Question::Confirm,

            WireQuestion::ChooseBundle { bundles } => Question::ChooseBundle {
                bundles: bundles
                    .into_iter()
                    .enumerate()
                    .map(|(i, b)| {
                        Ok(BundleOption {
                            index: b.index,
                            cards: read_cards(&format!("question.bundles[{i}]"), b.cards)?,
                        })
                    })
                    .collect::<Result<_, Error>>()?,
            },
        })
    }
}

impl WireRewardDetail {
    fn into_domain<V: sts2_content_wire::WireContentVersion>(
        self,
        path: &str,
    ) -> Result<RewardDetail<V>, Error> {
        // A known type missing the field that defines it is a protocol
        // disagreement, not something to paper over by falling back to `Other` —
        // that would turn a broken game into a reward the client silently cannot
        // act on.
        let missing = |field: &'static str| {
            at(
                path,
                Why::Missing {
                    field,
                    from: self.kind.clone(),
                },
            )
        };

        let reward = match self.kind.as_str() {
            "GoldReward" => RewardKind::Gold {
                amount: self.amount.ok_or_else(|| missing("amount"))?,
            },

            "CardReward" => RewardKind::Card {
                cards: read_cards(path, self.cards.ok_or_else(|| missing("cards"))?)?,
                alternatives: self
                    .alternatives
                    .iter()
                    .map(|id| {
                        sts2_core::RewardAlternative::from_option_id(id).ok_or_else(|| {
                            at(
                                &format!("{path}.alternatives"),
                                Why::Unknown {
                                    what: "card reward alternative",
                                    value: id.clone(),
                                },
                            )
                        })
                    })
                    .collect::<Result<_, Error>>()?,
            },

            // The one card the game hands back, sent through the card
            // reward's own shape. `cards` is absent, rather than empty, when
            // the reward holds no card: the game populates the field only once
            // it has one.
            "SpecialCardReward" => RewardKind::SpecialCard {
                card: match self.cards {
                    Some(cards) => read_cards::<V>(path, cards)?.into_iter().next(),
                    None => None,
                },
            },

            "RelicReward" => {
                let rarity = self.rarity.ok_or_else(|| missing("rarity"))?;
                RewardKind::Relic {
                    relic: self
                        .relic
                        .as_deref()
                        .map(|id| {
                            named::<V::Relic>(id).ok_or_else(|| {
                                at(
                                    &format!("{path}.relic"),
                                    Why::Unknown {
                                        what: "relic",
                                        value: id.to_string(),
                                    },
                                )
                            })
                        })
                        .transpose()?,
                    rarity: named::<<V::Relic as RelicTable>::Rarity>(&rarity).ok_or_else(
                        || {
                            at(
                                &format!("{path}.rarity"),
                                Why::Unknown {
                                    what: "relic rarity",
                                    value: rarity.clone(),
                                },
                            )
                        },
                    )?,
                }
            }

            "PotionReward" => RewardKind::Potion {
                potion: self
                    .potion
                    .as_deref()
                    .map(|id| {
                        named::<V::Potion>(id).ok_or_else(|| {
                            at(
                                &format!("{path}.potion"),
                                Why::Unknown {
                                    what: "potion",
                                    value: id.to_string(),
                                },
                            )
                        })
                    })
                    .transpose()?,
            },

            _ => RewardKind::Other { kind: self.kind },
        };

        Ok(RewardDetail {
            index: self.index,
            populated: self.populated,
            taken: self.taken,
            reward,
        })
    }
}

/// The cards inside a reward, in the order they are addressed.
fn read_cards<V: sts2_content_wire::WireContentVersion>(
    path: &str,
    cards: Vec<WireRewardCard>,
) -> Result<Vec<RewardCard<V>>, Error> {
    cards
        .into_iter()
        .enumerate()
        .map(|(i, c)| {
            Ok(RewardCard {
                card: read_ident(
                    &format!("{path}.cards[{i}]"),
                    &c.card,
                    ident::card_ident::<V>,
                )?,
                text: c.text,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_core::{Applied, Table, Tool};

    type V = V0_107_1;
    type Relic = <V as sts2_core::ContentVersion>::Relic;
    type RelicRarity = <Relic as RelicTable>::Rarity;

    /// A pending, wrapped in the smallest envelope and state that will carry it.
    ///
    /// Through [`crate::Response::applied`] rather than by deserializing a
    /// [`WireQuestion`] on its own. A question only ever arrives flattened into
    /// a pending, and that is the shape worth testing.
    fn asked(question: serde_json::Value) -> Result<Question<V>, Error> {
        let mut line = serde_json::json!({
            "type": "pending", "seq": 1, "cmd": "play_card", "digest": "D1",
            "state": {
                "act": 0, "floor": 1, "hp": 80, "maxHp": 80, "alive": true,
                "runOver": false, "victory": null, "gold": 99,
                "relics": [], "potions": [], "potionSlots": 3, "deck": [],
                "inCombat": false,
                "map": {
                    "at": null, "visited": [], "travelable": [], "points": [],
                    "proceed": false
                },
                "rng": {}
            }
        });

        let object = line.as_object_mut().expect("an object");
        for (key, value) in question.as_object().expect("a question object") {
            object.insert(key.clone(), value.clone());
        }

        let response: crate::Response =
            serde_json::from_value(line).expect("the sample should parse");
        let applied: Applied<V> = response.applied::<V>()?.expect("a pending carries a run");

        Ok(applied.asking.expect("a pending is asking something"))
    }

    #[test]
    fn a_card_selection_names_its_cards() {
        let q = asked(serde_json::json!({
            "kind": "select_cards", "min": 1, "max": 2, "asker": "DiscardAction", "picked": [],
            // Indices that are not positions, because the game's are the game's:
            // the wire number is what an answer names, and a reader deriving it
            // from the list order would agree with the game right up until the
            // game stopped agreeing with itself.
            "options": [
                {"index": 4, "card": "STRIKE_IRONCLAD", "text": "Deal 6 damage."},
                {"index": 7, "card": "BASH+1/SMOG"}
            ]
        }))
        .expect("should parse");

        let Question::SelectCards {
            min,
            max,
            asker,
            options,
            ..
        } = q
        else {
            panic!("expected select_cards");
        };

        assert_eq!((min, max), (1, 2));
        assert_eq!(asker.as_deref(), Some("DiscardAction"));
        assert_eq!((options[0].index, options[1].index), (4, 7));
        assert_eq!(options[0].text.as_deref(), Some("Deal 6 damage."));
        // The text is what the game says the card does, and a game that says
        // nothing is not the same as one that says the card does nothing.
        assert_eq!(options[1].text, None);

        // The identity is unpacked, so an upgraded, afflicted card is three
        // facts rather than one string nothing can compare.
        assert_eq!(options[1].card.upgrade, 1);
        assert!(options[1].card.affliction.is_some());
        assert_ne!(options[0].card.card, options[1].card.card);
    }

    #[test]
    fn a_selector_with_no_attribution_is_allowed() {
        let q = asked(serde_json::json!({
            "kind": "select_cards", "min": 0, "max": 1, "asker": null, "options": [],
            "picked": []
        }))
        .expect("should parse");

        let Question::SelectCards { asker, .. } = q else {
            panic!("expected select_cards");
        };
        assert!(asker.is_none());
    }

    #[test]
    fn rewards_describe_each_type_differently() {
        let q = asked(serde_json::json!({
            "kind": "rewards", "skippable": true,
            // Again numbers that are not positions. A reward screen is asked
            // once per decision, and the ones still on offer keep the numbers
            // they were first offered under — so position and index part ways
            // the moment anything is taken.
            "rewards": [
                {"index": 5, "type": "GoldReward", "populated": true, "amount": 25},
                {"index": 6, "type": "CardReward", "populated": true, "taken": true,
                 "cards": [{"card": "STRIKE_IRONCLAD", "text": "Deal 6 damage."},
                           {"card": "BASH+1"}]},
                {"index": 9, "type": "RelicReward", "populated": false,
                 "relic": null, "rarity": "Common"}
            ]
        }))
        .expect("should parse");

        let Question::Rewards { skippable, rewards } = q else {
            panic!("expected rewards");
        };

        assert!(skippable);
        assert_eq!(rewards[0].reward, RewardKind::Gold { amount: 25 });

        let RewardKind::Card { cards, .. } = &rewards[1].reward else {
            panic!("expected a card reward");
        };
        assert_eq!(cards[0].text.as_deref(), Some("Deal 6 damage."));
        assert_eq!(cards[1].text, None);
        assert_eq!(cards[1].card.upgrade, 1);

        // A screen asked twice reports which of its rewards are already gone,
        // and the ones still on offer keep the numbers they were first offered
        // under.
        assert!(rewards[1].taken, "the wire said this one was taken");
        assert!(!rewards[0].taken);
        assert_eq!(
            rewards.iter().map(|r| r.index).collect::<Vec<_>>(),
            [5, 6, 9]
        );

        // An explicit null is what an unpopulated relic reward actually sends —
        // the mod sets no ignore condition, so nulls go on the wire. The rarity
        // is still there, because it is a fact about the slot.
        assert!(!rewards[2].populated);
        assert_eq!(
            rewards[2].reward,
            RewardKind::Relic {
                relic: None,
                rarity: rarity("Common"),
            }
        );
    }

    #[test]
    fn a_returned_card_is_named_rather_than_chosen_between() {
        let q = asked(serde_json::json!({
            "kind": "rewards", "skippable": false,
            "rewards": [
                {"index": 0, "type": "SpecialCardReward", "populated": true,
                 "cards": [{"card": "BASH", "text": "Deal 8 damage."}]},
                // Populated is a fact about the slot, and the game sends no
                // card list until it has one to send.
                {"index": 1, "type": "SpecialCardReward", "populated": false}
            ]
        }))
        .expect("should parse");

        let Question::Rewards { rewards, .. } = q else {
            panic!("expected rewards");
        };

        let RewardKind::SpecialCard { card: Some(card) } = &rewards[0].reward else {
            panic!("expected a returned card");
        };
        assert_eq!(card.text.as_deref(), Some("Deal 8 damage."));

        assert_eq!(
            rewards[1].reward,
            RewardKind::SpecialCard { card: None },
            "an unpopulated slot is a reward with no card, not a reward absent"
        );
    }

    /// The rarity by name, so this test spells it the way the wire does.
    fn rarity(name: &str) -> RelicRarity {
        named::<RelicRarity>(name).expect("a rarity this build has")
    }

    #[test]
    fn a_potion_reward_may_have_no_potion() {
        let q = asked(serde_json::json!({
            "kind": "rewards", "skippable": false,
            "rewards": [{"index": 0, "type": "PotionReward", "populated": false, "potion": null}]
        }))
        .expect("should parse");

        let Question::Rewards { rewards, .. } = q else {
            panic!("expected rewards");
        };
        assert_eq!(rewards[0].reward, RewardKind::Potion { potion: None });
    }

    /// A reward type this crate does not model keeps its name. A caller can say
    /// what it saw, not only that it saw something.
    #[test]
    fn an_unmodelled_reward_type_is_named() {
        let q = asked(serde_json::json!({
            "kind": "rewards", "skippable": false,
            "rewards": [{"index": 3, "type": "SapphireReward", "populated": true}]
        }))
        .expect("should parse");

        let Question::Rewards { rewards, .. } = q else {
            panic!("expected rewards");
        };
        assert_eq!(
            rewards[0].reward,
            RewardKind::Other {
                kind: "SapphireReward".into(),
            }
        );
        assert!(rewards[0].populated);
    }

    /// A known type missing the field that defines it is a disagreement with the
    /// game, and has to be loud rather than degrading into `Other`.
    #[test]
    fn a_known_type_missing_its_field_is_refused() {
        let missing = |reward: serde_json::Value| {
            asked(serde_json::json!({
                "kind": "rewards", "skippable": false, "rewards": [reward]
            }))
            .expect_err("should refuse")
            .to_string()
        };

        let err = missing(serde_json::json!(
            {"index": 0, "type": "GoldReward", "populated": true}
        ));
        assert!(
            err.contains("a GoldReward with no amount"),
            "unhelpful message: {err}"
        );

        let err = missing(serde_json::json!(
            {"index": 0, "type": "RelicReward", "populated": true, "relic": "AKABEKO"}
        ));
        assert!(
            err.contains("a RelicReward with no rarity"),
            "unhelpful message: {err}"
        );
    }

    /// An empty card reward is legal and must not be confused with a broken one.
    #[test]
    fn an_empty_card_reward_is_not_a_missing_one() {
        let q = asked(serde_json::json!({
            "kind": "rewards", "skippable": false,
            "rewards": [{"index": 0, "type": "CardReward", "populated": false, "cards": []}]
        }))
        .expect("should parse");

        let Question::Rewards { rewards, .. } = q else {
            panic!("expected rewards");
        };
        assert_eq!(
            rewards[0].reward,
            RewardKind::Card {
                cards: vec![],
                alternatives: vec![]
            }
        );
    }

    /// An id the build does not have is a failure that names the path.
    #[test]
    fn a_relic_this_build_does_not_have_is_refused() {
        let err = asked(serde_json::json!({
            "kind": "rewards", "skippable": false,
            "rewards": [{"index": 0, "type": "RelicReward", "populated": true,
                         "relic": "SAPPHIRE", "rarity": "Common"}]
        }))
        .expect_err("should refuse");

        assert_eq!(err.at, "question.rewards[0].relic");
        assert!(err.to_string().contains("SAPPHIRE"), "{err}");
    }

    /// The Crystal Sphere reports a tool an answer cannot name.
    ///
    /// `CrystalSphereToolType` has three members and a sphere starts at `None`.
    /// [`Tool`] has two, because `CellClicked` tests only for `Big`. So a
    /// reported `None` reads back as the move it is.
    #[test]
    fn a_bundle_choice_names_each_bundles_cards() {
        let question = asked(serde_json::json!({
            "kind": "choose_bundle",
            "bundles": [
                {"index": 0, "cards": [{"card": "TWIN_STRIKE", "text": "Deal 5 damage twice."},
                                       {"card": "POMMEL_STRIKE+1"}]},
                {"index": 1, "cards": []}
            ]
        }))
        .unwrap();
        let Question::ChooseBundle { bundles } = question else {
            panic!("a bundle choice read as {question:?}");
        };
        assert_eq!(bundles.len(), 2);
        assert_eq!(bundles[0].index, 0);
        assert_eq!(bundles[0].cards.len(), 2);
        assert_eq!(bundles[0].cards[1].card.upgrade, 1);
        assert_eq!(bundles[0].cards[1].text, None);
        assert!(bundles[1].cards.is_empty());
    }

    #[test]
    fn the_tool_a_sphere_starts_with_is_the_small_one() {
        let divine = |tool: &str| {
            let q = asked(serde_json::json!({
                "kind": "divine", "left": 6, "width": 11, "height": 11,
                "tool": tool,
                "cleared": [{"x": 1, "y": 2}],
                "revealed": [{"item": "CrystalSphereCurse", "good": false,
                              "x": 3, "y": 4, "width": 2, "height": 2}]
            }))
            .expect("should parse");

            let Question::Divine {
                tool,
                cleared,
                revealed,
                left,
                ..
            } = q
            else {
                panic!("expected divine");
            };
            assert_eq!(left, 6);
            assert_eq!(cleared, vec![Cell { x: 1, y: 2 }]);
            assert!(!revealed[0].good);
            assert_eq!(revealed[0].item, "CrystalSphereCurse");
            tool
        };

        assert_eq!(divine("None"), Tool::Small);
        assert_eq!(divine("Small"), Tool::Small);
        assert_eq!(divine("Big"), Tool::Big);
    }

    /// Every relic rarity the build has is a rarity a reward can report.
    #[test]
    fn the_rarities_a_reward_can_name_are_the_builds() {
        for relic in <Relic as Table>::ALL {
            let name = serde_json::to_value(relic.rarity()).expect("a rarity serializes");
            let name = name.as_str().expect("a rarity is a name");
            assert!(
                named::<RelicRarity>(name).is_some(),
                "{name} does not read back"
            );
        }
    }
}
