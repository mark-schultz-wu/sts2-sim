//! Every command the game accepts right now.
//!
//! [`super::super::render::next`] answers "what do I do next". This answers
//! "what is available", which is the question a driver that is not a person
//! asks. The list is the run's own. The game reports it in each observation,
//! and the simulator reports its `legal`, which the scenarios hold to the
//! game's. Nothing here decides what is legal.

use sts2_core::{Answer, Decision, Question, RewardKind, Snapshot, TakeReward};

/// Every legal move, in the order the run reported them, or an empty list
/// when the run is waiting on nobody.
pub fn legal<V: sts2_content_wire::WireContentVersion>(snapshot: &Snapshot<V>) -> Vec<Decision<V>> {
    snapshot.observation.moves.clone()
}

/// The ordinary numbered choices shared by terminal and automation frontends,
/// out of every legal move.
pub fn navigation<V: sts2_content_wire::WireContentVersion>(
    legal: &[Decision<V>],
) -> Vec<Decision<V>> {
    (legal.iter())
        .filter(|decision| {
            matches!(
                decision,
                Decision::ChooseOption { .. }
                    | Decision::Buy { .. }
                    | Decision::Travel { .. }
                    | Decision::Proceed
            )
        })
        .cloned()
        .collect()
}

/// Every individual reward the pending set will still hand over.
pub fn takeable<V: sts2_content_wire::WireContentVersion>(
    question: &Question<V>,
) -> Vec<TakeReward> {
    let Question::Rewards { rewards, .. } = question else {
        return Vec::new();
    };

    rewards
        .iter()
        .filter(|detail| !detail.taken)
        .flat_map(|detail| match &detail.reward {
            RewardKind::Card {
                cards,
                alternatives,
            } => (0..cards.len())
                .map(|card| TakeReward::card(detail.index, card))
                .chain(
                    (alternatives.iter())
                        .map(|&alternative| TakeReward::alternative(detail.index, alternative)),
                )
                .collect(),
            _ => vec![TakeReward::non_card(detail.index)],
        })
        .collect()
}

/// What a person-facing log says a decision did.
pub fn describe<V: sts2_content_wire::WireContentVersion>(decision: &Decision<V>) -> String {
    match decision {
        Decision::PlayCard { index, target } => match target {
            Some(t) => format!("played card {index} at {t}"),
            None => format!("played card {index}"),
        },
        Decision::EndTurn => "ended the turn".into(),
        Decision::UsePotion { index, target } => match target {
            Some(t) => format!("drank potion {index} at {t}"),
            None => format!("drank potion {index}"),
        },
        Decision::Travel { to } => format!("travelled to {},{}", to.col, to.row),
        Decision::Proceed => "left the act".into(),
        Decision::ChooseOption { index } => format!("chose option {index}"),
        Decision::Buy { index } => format!("bought entry {index}"),
        Decision::Answer(Answer::Skip) => "skipped".into(),
        Decision::Answer(Answer::Cards(cards)) if cards.is_empty() => "chose no cards".into(),
        Decision::Answer(Answer::Cards(cards)) => format!("chose cards {cards:?}"),
        Decision::Answer(Answer::Pick(at)) => format!("picked card {at}"),
        Decision::Answer(Answer::Undo) => "took back the last pick".into(),
        Decision::Answer(Answer::Done) => "handed over the picks".into(),
        Decision::Answer(Answer::Take(TakeReward::Card { index, card })) => {
            format!("took {index}.{card}")
        }
        Decision::Answer(Answer::Take(TakeReward::NonCard { index })) => format!("took {index}"),
        Decision::Answer(Answer::Take(TakeReward::Alternative { index, alternative })) => {
            format!("took {index} as {}", alternative.option_id())
        }
        Decision::Answer(Answer::Divine { x, y, .. }) => format!("divined {x},{y}"),
        Decision::Answer(Answer::Confirm(true)) => "confirmed".into(),
        Decision::Answer(Answer::Confirm(false)) => "cancelled".into(),
        Decision::EnterRoom { room } => format!("entered {}", crate::render::spelling(room)),
    }
}

#[cfg(test)]
mod tests {
    use super::{legal, navigation};
    use crate::fixture;
    use serde_json::json;
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_core::{Answer, Coord, Decision};

    type V = V0_107_1;

    /// The list is the one the run reported, in its order, and nothing is
    /// added to it or taken away.
    #[test]
    fn the_moves_are_the_ones_the_run_reported() {
        let snapshot = fixture::snapshot(json!({
            "moves": [
                {"cmd": "use_potion", "index": 0, "target": null},
                {"cmd": "choose_option", "index": 2},
                {"cmd": "travel", "col": 0, "row": 3}
            ]
        }));

        assert_eq!(
            legal::<V>(&snapshot),
            [
                Decision::UsePotion {
                    index: sts2_core::fixture::belt(0),
                    target: None,
                },
                Decision::ChooseOption { index: 2 },
                Decision::Travel {
                    to: Coord::new(0, 3)
                },
            ]
        );
    }

    /// A pending question's answers come in the same list.
    #[test]
    fn a_question_answers_from_the_reported_list() {
        let applied = fixture::answered(json!({
            "type": "pending", "seq": 3, "cmd": "play_card", "digest": "AA",
            "state": {"moves": [
                {"cmd": "answer", "pick": 1},
                {"cmd": "answer", "done": true}
            ]},
            "kind": "select_cards", "min": 0, "max": 1, "asker": "DISCARD",
            "options": [{"index": 0, "card": "ANGER"}, {"index": 1, "card": "ARMAMENTS"}],
            "picked": []
        }));

        assert_eq!(
            legal::<V>(&applied.snapshot()),
            [
                Decision::Answer(Answer::Pick(1)),
                Decision::Answer(Answer::Done),
            ]
        );
    }

    /// The TUI consumes this exact subset rather than deriving a parallel list.
    #[test]
    fn navigation_is_the_numbered_subset_of_legal_play() {
        let applied = fixture::applied(json!({
            "moves": [
                {"cmd": "use_potion", "index": 0, "target": null},
                {"cmd": "choose_option", "index": 6}
            ]
        }));

        assert_eq!(
            navigation(&applied.observation.moves),
            vec![Decision::ChooseOption { index: 6 }]
        );
    }
}
