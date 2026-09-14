//! The one line that says what to do.
//!
//! There were three: a card selection printed its answer, a room printed its
//! options' command, a hand printed a play. Each decided independently, so a
//! reward pending and a Neow room both claimed to be actionable in one
//! observation. Only one of them was, because a pending blocks everything but
//! `answer`, `observe` and `quit`.
//!
//! So it is computed once, from the whole response, in the protocol's own order
//! of precedence. Sections still list what they hold, and none of them offers
//! advice.

use crate::cli::choose;
use sts2_core::{Combat, Map, Observation, Question, RewardKind, Room, Snapshot};

/// The command to run next, or nothing when the game is not waiting on anyone.
pub(super) fn next<V: sts2_content_wire::WireContentVersion>(
    snapshot: &Snapshot<V>,
) -> Option<String> {
    match &snapshot.asking {
        Some(question) => Some(answering(question)),
        None => observing(&snapshot.observation, snapshot.observation.fight()),
    }
}

/// What answers the question a parked game is asking.
fn answering<V: sts2_content_wire::WireContentVersion>(question: &Question<V>) -> String {
    match question {
        Question::SelectCards {
            min, max, options, ..
        } => {
            // An example the game would actually accept. Taking it from `max`
            // alone gave `choose 0 1 2` for "discard up to 3" with two cards in
            // hand — index 2 refused — and three indices for a question with a
            // minimum of five, refused for being too few.
            let most = (*max as usize).min(options.len());
            let shown = (*min as usize).max(3.min(most)).min(most);
            let example: Vec<String> = (0..shown).map(|i| i.to_string()).collect();

            match (min, example.is_empty()) {
                (_, true) => "sts2 act choose   (nothing to pick from)".into(),
                (0, false) => format!(
                    "sts2 act choose {}   (or none: sts2 act choose)",
                    example.join(" ")
                ),
                (_, false) => format!("sts2 act choose {}", example.join(" ")),
            }
        }
        Question::Rewards { skippable, rewards } => {
            // A card reward is refused without a card index, so the example has
            // to be the dotted pair rather than the reward on its own.
            //
            // The first one still *on offer*: a screen asked twice keeps the
            // rewards already taken in its list, and pointing at one of those
            // would be advising a command the game refuses.
            let first = rewards.iter().find(|r| !r.taken).map(|r| match &r.reward {
                RewardKind::Card { .. } => format!("{}.0", r.index),
                _ => r.index.to_string(),
            });

            match (first, skippable) {
                (Some(pick), true) => {
                    format!("sts2 act choose {pick}   (or: sts2 act answer skip)")
                }
                (Some(pick), false) => format!("sts2 act choose {pick}"),
                (None, true) => "sts2 act answer skip".into(),
                (None, false) => "nothing to take, and it cannot be skipped".into(),
            }
        }

        // The middle of the grid, which is hidden while anything is. Any cell
        // would do -- the point of the hint is the shape of the command, and
        // there is no better cell to name: what is under one is not knowable.
        Question::Divine {
            width,
            height,
            cleared,
            ..
        } => {
            let free = (0..*width)
                .flat_map(|x| (0..*height).map(move |y| (x, y)))
                .find(|(x, y)| !cleared.iter().any(|c| c.x == *x && c.y == *y));

            match free {
                Some((x, y)) => format!("sts2 act answer divine {x} {y}   (or: --big)"),
                None => "every cell is uncovered".into(),
            }
        }
    }
}

/// What to do when nothing is pending, in the order the game allows it.
fn observing<V: sts2_content_wire::WireContentVersion>(
    observed: &Observation<V>,
    combat: Option<&Combat<V>>,
) -> Option<String> {
    if let Some(room) = &observed.room
        && let Some(index) = takeable(room)
    {
        return Some(format!("sts2 act choose {index}"));
    }

    if let Some(combat) = combat {
        return Some(playing(combat));
    }

    travelling(&observed.map)
}

/// The first thing in the room `sts2 act choose` takes.
///
/// Uses [`choose`] itself so advice cannot drift from action. This avoids
/// suggesting disabled rest-site options. Shops join the search because their
/// stock uses the same numeric input even though it is bought, not chosen.
fn takeable<V: sts2_content_wire::WireContentVersion>(room: &Room<V>) -> Option<u8> {
    choose::choosable(room)
        .into_iter()
        .chain(choose::buyable(room))
        .next()
}

/// The first playable card, with a legal target where one is needed.
fn playing<V: sts2_content_wire::WireContentVersion>(combat: &Combat<V>) -> String {
    for (index, card) in combat.legal.iter().enumerate() {
        if card.reason.is_some() {
            continue;
        }

        let target = card
            .targets
            .first()
            .map(|t| format!(" --target {t}"))
            .unwrap_or_default();

        return format!("sts2 act play-card {index}{target}");
    }

    "sts2 act end-turn".into()
}

/// Where the map can go from here.
fn travelling<V: sts2_content_wire::WireContentVersion>(map: &Map<V>) -> Option<String> {
    // A beaten boss has nowhere to travel to, so without this the run reads as
    // finished when it is one command from the next act.
    if map.proceed {
        return Some("sts2 act proceed".into());
    }

    let to = map.travelable.first()?.at;
    Some(format!("sts2 act travel {} {}", to.col, to.row))
}
