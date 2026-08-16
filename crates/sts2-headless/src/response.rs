//! What the game sends back.
//!
//! The envelope is typed; the observation payload is not. `state` stays a
//! [`serde_json::Value`] on purpose — narrowing it into Rust structs is a large
//! job whose right shape depends on what the simulator turns out to need, and
//! doing it now would mean designing against a consumer that does not exist yet.
//! Nothing is lost by waiting: the raw value carries every field the game
//! reported, so no state can go missing while it is still untyped.
//!
//! That argument covers the payload and not the envelope, and the difference
//! once mattered: [`Response::Pending`] carried `digest` and `state` on the wire
//! with no fields to receive them, so serde discarded both on every pending.
//! Envelope drift is what the protocol version in the handshake is for, and
//! [`crate::Game`] checks it.

use serde::Deserialize;
use serde_json::Value;

/// A request that the game accepted.
///
/// Exactly one reply arrives per request, though not always the kind the command
/// implies — anything can be answered with [`Response::Pending`].
///
/// Two messages are deliberately absent. A [`Refusal`] is the inner `Err` of a
/// [`crate::Reply`] rather than a variant here, since it is the one reply that
/// means nothing happened. And the greeting arrives once, before any request,
/// and [`crate::Game`] consumes it during the handshake — so a variant for it
/// would be one every caller must match and none could ever receive.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    /// The command advanced the game, and it is now idle.
    Observation {
        /// Increments per observation or pending, not per request.
        seq: u64,
        cmd: String,
        /// False when combat ended mid-cascade and the decision was abandoned
        /// rather than completed. Not an error — but a simulator that resolved
        /// it fully has diverged, even if the resulting state looks agreeable.
        resolved: bool,
        digest: String,
        /// The whole state the digest was taken over.
        state: Value,
        /// Errors the game logged while running the command. Never suppressed:
        /// the command may have half-happened, and only the caller can decide
        /// whether to trust the state.
        #[serde(rename = "gameErrors", default)]
        game_errors: Option<Vec<String>>,
    },

    /// The game stopped mid-action to ask something.
    ///
    /// Until it is answered the game accepts only [`crate::Command::Answer`],
    /// [`crate::Command::Observe`] and [`crate::Command::Quit`].
    Pending {
        seq: u64,
        cmd: String,
        /// A parked game still reports where it is: `digest` and `state` are
        /// attached to a pending exactly as they are to an observation.
        digest: String,
        state: Value,
        #[serde(flatten)]
        question: Question,
    },

    /// The reply to [`crate::Command::Quit`], and the last line the game sends.
    Bye,
}

/// A refused command.
///
/// Its own type rather than a [`Response`] variant, because it is the one reply
/// that means the game did not move. [`crate::Game::send`] returns it in the
/// inner `Err` of a [`crate::Reply`]: a refusal fails the command while leaving
/// the session healthy, which is a different thing from the transport failing.
///
/// There is no state on one, for the same reason — nothing advanced, so whatever
/// the caller already had is still current.
#[derive(Debug, Clone, Deserialize, thiserror::Error)]
#[error("refused {cmd}: {message}")]
pub struct Refusal {
    pub cmd: String,
    /// Written to be read: an illegal `travel` lists the legal coords, an
    /// unplayable card names the reason and what blocked it.
    pub message: String,
}

/// What a parked game is asking.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Question {
    /// Answer with [`crate::Answer::Cards`].
    SelectCards {
        min: u32,
        max: u32,
        /// The running action, not the game's own prompt text — a selector never
        /// receives the prompt, so this is attribution rather than wording.
        asker: Option<String>,
        options: Vec<CardOption>,
    },
    /// Answer with [`crate::Answer::Skip`] or [`crate::Answer::Take`].
    Rewards {
        skippable: bool,
        rewards: Vec<RewardDetail>,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct CardOption {
    pub index: usize,
    /// Entry name, plus `+N` when upgraded, plus `/AFFLICTION` when afflicted.
    pub card: String,
}

/// One reward on offer.
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "WireReward")]
pub struct RewardDetail {
    pub index: usize,
    /// A reward can exist in the set without content, which is not the same as
    /// the reward being absent. Reported for every kind, including
    /// [`RewardKind::Other`].
    pub populated: bool,
    pub reward: RewardKind,
}

/// What a reward actually offers.
///
/// A sum rather than a struct of mutually exclusive `Option`s: the game sends
/// exactly one shape per reward, so anything else should not be expressible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RewardKind {
    Gold {
        amount: i64,
    },
    /// Entry names, in the form [`CardOption::card`] uses. Empty is possible —
    /// an unpopulated card reward still reports the field.
    Card {
        cards: Vec<String>,
    },
    Relic {
        /// Absent when the reward exists but `RelicReward.Populate` found nothing.
        relic: Option<String>,
        rarity: String,
    },
    Potion {
        /// Absent for the same reason a relic can be.
        potion: Option<String>,
    },
    /// A reward type this crate does not unpack, named rather than anonymous.
    ///
    /// Deliberately not a catch-all meaning "cannot say": an earlier attempt in
    /// this repo used one of those for screens, and a value indistinguishable
    /// from a real answer is worse than no value. This says only that the game
    /// described a reward whose per-type fields are not modelled here.
    Other {
        kind: String,
    },
}

/// The flat form the game actually sends.
///
/// Kept private and converted immediately: `type` selects which of the remaining
/// fields are meaningful, which is a sum type wearing a struct's clothes.
#[derive(Deserialize)]
struct WireReward {
    index: usize,
    #[serde(rename = "type")]
    kind: String,
    populated: bool,
    #[serde(default)]
    amount: Option<i64>,
    #[serde(default)]
    cards: Option<Vec<String>>,
    #[serde(default)]
    relic: Option<String>,
    #[serde(default)]
    rarity: Option<String>,
    #[serde(default)]
    potion: Option<String>,
}

impl TryFrom<WireReward> for RewardDetail {
    type Error = String;

    fn try_from(w: WireReward) -> Result<Self, Self::Error> {
        // A known type missing the field that defines it is a protocol
        // disagreement, not something to paper over by falling back to `Other` —
        // that would turn a broken game into a reward the client silently
        // cannot act on.
        let missing = |field: &str| format!("{} is missing '{field}'", w.kind);

        let reward = match w.kind.as_str() {
            "GoldReward" => RewardKind::Gold {
                amount: w.amount.ok_or_else(|| missing("amount"))?,
            },
            "CardReward" => RewardKind::Card {
                cards: w.cards.ok_or_else(|| missing("cards"))?,
            },
            "RelicReward" => RewardKind::Relic {
                relic: w.relic,
                rarity: w.rarity.ok_or_else(|| missing("rarity"))?,
            },
            "PotionReward" => RewardKind::Potion { potion: w.potion },
            _ => RewardKind::Other { kind: w.kind },
        };

        Ok(Self {
            index: w.index,
            populated: w.populated,
            reward,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Response {
        serde_json::from_str(line).expect("sample should parse")
    }

    /// The client always asks for full observations, so one arriving without
    /// state means the game disagreed about that. Refusing it is the point of
    /// `state` not being an `Option`.
    #[test]
    fn an_observation_without_state_is_refused() {
        let err = serde_json::from_str::<Response>(
            r#"{"type":"observation","seq":3,"cmd":"play_card",
                "resolved":true,"digest":"A21E"}"#,
        )
        .expect_err("should refuse");
        assert!(
            err.to_string().contains("state"),
            "unhelpful message: {err}"
        );
    }

    #[test]
    fn an_observation_carries_its_state_untouched() {
        let r = parse(
            r#"{"type":"observation","seq":4,"cmd":"observe","resolved":true,
                "digest":"B0","state":{"gold":99,"nested":{"hp":[1,2]}}}"#,
        );
        let Response::Observation {
            seq,
            resolved,
            state,
            game_errors,
            ..
        } = r
        else {
            panic!("expected observation");
        };
        assert_eq!(seq, 4);
        assert!(resolved);
        assert!(game_errors.is_none());
        assert_eq!(state["gold"], 99);
        assert_eq!(state["nested"]["hp"][1], 2);
    }

    #[test]
    fn game_errors_survive() {
        let r = parse(
            r#"{"type":"observation","seq":5,"cmd":"end_turn","resolved":false,
                "digest":"C1","state":{},"gameErrors":["boom"]}"#,
        );
        let Response::Observation {
            resolved,
            game_errors,
            ..
        } = r
        else {
            panic!("expected observation");
        };
        assert!(!resolved);
        assert_eq!(game_errors.as_deref(), Some(&["boom".to_string()][..]));
    }

    /// Regression. `Pending` had no fields for `digest` or `state`, and serde
    /// drops what it has nowhere to put — so every pending silently threw away
    /// the state of a parked game, which is when you most want it.
    #[test]
    fn a_pending_reports_where_the_game_is() {
        let r = parse(
            r#"{"type":"pending","seq":7,"cmd":"play_card","kind":"select_cards",
                "digest":"DEADBEEF","state":{"gold":99},
                "min":1,"max":1,"asker":null,"options":[]}"#,
        );
        let Response::Pending { digest, state, .. } = r else {
            panic!("expected pending");
        };
        assert_eq!(digest, "DEADBEEF");
        assert_eq!(state["gold"], 99);
    }

    /// The envelope and the question arrive flat in one object, so this pins
    /// that `flatten` plus an internally tagged enum works inside a variant.
    #[test]
    fn select_cards_flattens_into_the_envelope() {
        let r = parse(
            r#"{"type":"pending","seq":7,"cmd":"play_card","kind":"select_cards",
                "digest":"D1","state":{},"min":1,"max":2,"asker":"DiscardAction",
                "options":[{"index":0,"card":"STRIKE"},{"index":1,"card":"BASH+1"}]}"#,
        );
        let Response::Pending {
            seq, cmd, question, ..
        } = r
        else {
            panic!("expected pending");
        };
        assert_eq!(seq, 7);
        assert_eq!(cmd, "play_card");
        let Question::SelectCards {
            min, max, options, ..
        } = question
        else {
            panic!("expected select_cards");
        };
        assert_eq!((min, max), (1, 2));
        assert_eq!(options[1].card, "BASH+1");
    }

    #[test]
    fn a_selector_with_no_attribution_is_allowed() {
        let r = parse(
            r#"{"type":"pending","seq":8,"cmd":"answer","kind":"select_cards",
                "digest":"D2","state":{},"min":0,"max":1,"asker":null,"options":[]}"#,
        );
        let Response::Pending { question, .. } = r else {
            panic!("expected pending");
        };
        let Question::SelectCards { asker, .. } = question else {
            panic!("expected select_cards");
        };
        assert!(asker.is_none());
    }

    #[test]
    fn rewards_describe_each_type_differently() {
        let r = parse(
            r#"{"type":"pending","seq":9,"cmd":"end_turn","kind":"rewards",
                "digest":"D3","state":{},"skippable":true,
                "rewards":[
                  {"index":0,"type":"GoldReward","populated":true,"amount":25},
                  {"index":1,"type":"CardReward","populated":true,"cards":["STRIKE","BASH+1"]},
                  {"index":2,"type":"RelicReward","populated":false,"relic":null,"rarity":"COMMON"}]}"#,
        );
        let Response::Pending { question, .. } = r else {
            panic!("expected pending");
        };
        let Question::Rewards { skippable, rewards } = question else {
            panic!("expected rewards");
        };
        assert!(skippable);
        assert_eq!(rewards[0].reward, RewardKind::Gold { amount: 25 });
        assert_eq!(
            rewards[1].reward,
            RewardKind::Card {
                cards: vec!["STRIKE".into(), "BASH+1".into()],
            }
        );

        // An explicit null is what an unpopulated relic reward actually sends —
        // the mod sets no ignore condition, so nulls go on the wire.
        assert!(!rewards[2].populated);
        assert_eq!(
            rewards[2].reward,
            RewardKind::Relic {
                relic: None,
                rarity: "COMMON".into(),
            }
        );
    }

    fn reward(json: &str) -> Result<RewardDetail, serde_json::Error> {
        serde_json::from_str(json)
    }

    #[test]
    fn a_potion_reward_may_have_no_potion() {
        let r = reward(r#"{"index":0,"type":"PotionReward","populated":false,"potion":null}"#)
            .expect("should parse");
        assert_eq!(r.reward, RewardKind::Potion { potion: None });
    }

    /// A reward type this crate does not model keeps its name, so a caller can
    /// say what it saw rather than only that it saw something.
    #[test]
    fn an_unmodelled_reward_type_is_named() {
        let r = reward(r#"{"index":3,"type":"SapphireReward","populated":true}"#)
            .expect("should parse");
        assert_eq!(
            r.reward,
            RewardKind::Other {
                kind: "SapphireReward".into(),
            }
        );
        assert!(r.populated);
    }

    /// A known type missing the field that defines it is a disagreement with the
    /// game, and has to be loud rather than degrading into `Other`.
    #[test]
    fn a_known_type_missing_its_field_is_refused() {
        let err = reward(r#"{"index":0,"type":"GoldReward","populated":true}"#)
            .expect_err("should refuse");
        assert!(
            err.to_string().contains("GoldReward is missing 'amount'"),
            "unhelpful message: {err}"
        );

        let err = reward(r#"{"index":0,"type":"RelicReward","populated":true,"relic":"BURNING"}"#)
            .expect_err("should refuse");
        assert!(
            err.to_string().contains("RelicReward is missing 'rarity'"),
            "unhelpful message: {err}"
        );
    }

    /// An empty card reward is legal and must not be confused with a broken one.
    #[test]
    fn an_empty_card_reward_is_not_a_missing_one() {
        let r = reward(r#"{"index":0,"type":"CardReward","populated":false,"cards":[]}"#)
            .expect("should parse");
        assert_eq!(r.reward, RewardKind::Card { cards: vec![] });
    }

    #[test]
    fn a_refusal_carries_the_command_it_refused() {
        let r: Refusal =
            serde_json::from_str(r#"{"type":"error","cmd":"travel","message":"not travelable"}"#)
                .expect("should parse");
        assert_eq!(r.cmd, "travel");
        assert_eq!(r.message, "not travelable");
        assert_eq!(r.to_string(), "refused travel: not travelable");
    }

    #[test]
    fn bye_needs_nothing_else() {
        assert!(matches!(parse(r#"{"type":"bye"}"#), Response::Bye));
    }
}
