//! The one line that says what to do.
//!
//! There were three: a card selection printed its answer, a room printed its
//! options' command, a hand printed a play. Each decided independently, so a
//! reward pending and a Neow room could both claim to be actionable in one
//! observation — and only one of them was, because a pending blocks everything
//! but `answer`, `observe` and `quit`.
//!
//! So it is computed once, from the whole response, in the protocol's own order
//! of precedence. Sections still list what they hold; none of them offers
//! advice.

use super::Field;
use serde_json::Value;
use sts2_headless::{Question, Response, RewardKind};

/// The command to run next, or nothing when the game is not waiting on anyone.
pub(super) fn next(response: &Response) -> Option<String> {
    match response {
        Response::Bye => None,
        Response::Pending { question, .. } => Some(answering(question)),
        Response::Observation { state, .. } => observing(state),
    }
}

/// What answers the question a parked game is asking.
fn answering(question: &Question) -> String {
    match question {
        Question::SelectCards { min, max, .. } => {
            let example: Vec<String> = (0..*max.min(&3)).map(|i| i.to_string()).collect();
            match min {
                0 => format!("sts2 choose {}   (or none: sts2 choose)", example.join(" ")),
                _ => format!("sts2 choose {}", example.join(" ")),
            }
        }
        Question::Rewards { skippable, rewards } => {
            // A card reward is refused without a card index, so the example has
            // to be the dotted pair rather than the reward on its own.
            let first = rewards.first().map(|r| match &r.reward {
                RewardKind::Card { .. } => format!("{}.0", r.index),
                _ => r.index.to_string(),
            });

            match (first, skippable) {
                (Some(pick), true) => format!("sts2 choose {pick}   (or: sts2 answer skip)"),
                (Some(pick), false) => format!("sts2 choose {pick}"),
                (None, true) => "sts2 answer skip".into(),
                (None, false) => "nothing to take, and it cannot be skipped".into(),
            }
        }
    }
}

/// What to do when nothing is pending, in the order the game allows it.
fn observing(state: &Value) -> Option<String> {
    if let Some(room) = state.get("room")
        && let Some(index) = choosable(room)
    {
        return Some(format!("sts2 choose {index}"));
    }

    if let Some(combat) = state.get("combat")
        && state.is_true("inCombat")
    {
        return Some(playing(combat));
    }

    travelling(state)
}

/// A room option that has not been taken and is not locked.
fn choosable(room: &Value) -> Option<i64> {
    room.list_at("options").iter().find_map(|o| {
        let taken = o.is_true("chosen") || o.is_true("locked");
        match taken {
            true => None,
            false => o.int_at("index"),
        }
    })
}

/// The first playable card, with a legal target where one is needed.
fn playing(combat: &Value) -> String {
    let legal = combat.list_at("legal");

    for (index, card) in legal.iter().enumerate() {
        if !card.is_true("playable") {
            continue;
        }

        let target = card
            .list_at("targets")
            .iter()
            .filter_map(Value::as_i64)
            .next()
            .map(|t| format!(" --target {t}"))
            .unwrap_or_default();

        return format!("sts2 play-card {index}{target}");
    }

    "sts2 end-turn".into()
}

/// Where the map can go from here.
///
/// A coordinate arrives as the string `"col,row"` rather than a pair of
/// numbers, which is how the map section already reads it.
fn travelling(state: &Value) -> Option<String> {
    let to = state
        .get("map")?
        .list_at("travelable")
        .first()?
        .str_at("at")?;

    let (col, row) = to.split_once(',')?;

    Some(format!("sts2 travel {col} {row}"))
}
