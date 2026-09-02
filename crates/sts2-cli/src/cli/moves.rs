//! Every command the game accepts right now.
//!
//! [`super::super::render::next`] answers "what do I do next". This answers
//! "what is available", which is the question a driver that is not a person
//! asks. The two agree because the legality rules are [`choose`]'s in both
//! cases. A move this offers is a move that verb runs.
//!
//! Only the moves that advance a run are listed. `observe` changes nothing and
//! `quit` ends the session, so neither belongs in a set something picks from at
//! random. `grant` and `fight` construct a scenario rather than play one.

use super::choose;
use nonmax::NonMaxU32;
use sts2_core::{
    Answer, Applied, Combat, ContentVersion, Decision, Map, Observation, Question, RewardKind,
    TakeReward, Tool,
};

/// Every legal move, or an empty list when the game is waiting on nobody.
///
/// One category at a time, in the protocol's order of precedence. A pending
/// question blocks everything but an answer, and a room with options must be
/// answered before the map opens again. Mixing categories produces moves that
/// are legal in isolation and refused in place.
pub fn legal<V: ContentVersion>(applied: &Applied<V>) -> Vec<Decision<V>> {
    match &applied.asking {
        Some(question) => answers(question),
        None => plays(&applied.observation, applied.observation.fight()),
    }
}

/// Every answer a parked game accepts.
fn answers<V: ContentVersion>(question: &Question<V>) -> Vec<Decision<V>> {
    match question {
        // Every legal *size* of selection rather than every combination, which
        // is exponential and mostly the same question asked again. The indices
        // are the first `n`; which cards those are is the shuffle's business,
        // and a driver picking differently is picking between equals.
        Question::SelectCards {
            min, max, options, ..
        } => {
            let most = (*max as usize).min(options.len());
            (*min as usize..=most)
                .map(|n| Decision::Answer(Answer::Cards((0..n).collect())))
                .collect()
        }

        Question::Rewards { skippable, rewards } => {
            let mut moves: Vec<Decision<V>> = Vec::new();

            for reward in rewards.iter().filter(|r| !r.taken) {
                match &reward.reward {
                    // A card reward is refused without a card index rather than
                    // defaulting to one, so each card is its own move.
                    RewardKind::Card { cards } => moves.extend((0..cards.len()).map(|card| {
                        Decision::Answer(Answer::Take(vec![TakeReward::Card {
                            index: reward.index,
                            card,
                        }]))
                    })),
                    _ => moves.push(Decision::Answer(Answer::Take(vec![TakeReward::NonCard {
                        index: reward.index,
                    }]))),
                }
            }

            if *skippable {
                moves.push(Decision::Answer(Answer::Skip));
            }
            moves
        }

        // Every still-hidden cell, with both tools. That is up to 242 moves at
        // the start, and unlike a card selection they are not equals: what is
        // under one cell is not what is under another, and `Big` trades a wider
        // uncovering against the chance of turning up a curse.
        Question::Divine {
            width,
            height,
            cleared,
            ..
        } => {
            let mut moves: Vec<Decision<V>> = Vec::new();
            for x in 0..*width {
                for y in 0..*height {
                    if cleared.iter().any(|c| c.x == x && c.y == y) {
                        continue;
                    }
                    for tool in [Tool::Small, Tool::Big] {
                        moves.push(Decision::Answer(Answer::Divine { x, y, tool }));
                    }
                }
            }
            moves
        }
    }
}

/// Every move an idle game accepts, in the order the protocol allows them.
fn plays<V: ContentVersion>(
    observed: &Observation<V>,
    combat: Option<&Combat<V>>,
) -> Vec<Decision<V>> {
    // Drinking cuts across the rest: the belt says when a potion may be drunk,
    // and an `AnyTime` potion may be drunk in a shop as readily as in a fight.
    // So these are added to whatever else is on offer rather than being one
    // more branch that shadows the others.
    let mut moves = potion_moves(observed);

    if let Some(room) = &observed.room {
        let taken = room_moves(room);
        if !taken.is_empty() {
            moves.extend(taken);
            return moves;
        }
    }

    if let Some(combat) = combat {
        moves.extend(combat_moves(combat));
        return moves;
    }

    moves.extend(travel_moves(&observed.map));
    moves
}

/// Every drinkable potion, against every creature it can be drunk at.
///
/// The belt's own `reason` and `targets`, believed rather than re-derived. No
/// rule about when a potion is drinkable lives in this crate.
fn potion_moves<V: ContentVersion>(observed: &Observation<V>) -> Vec<Decision<V>> {
    let mut moves: Vec<Decision<V>> = Vec::new();

    for (index, potion) in observed.potions.iter().enumerate() {
        if potion.reason.is_some() {
            continue;
        }
        let Ok(index) = u8::try_from(index) else {
            continue;
        };

        let targets: Vec<Option<NonMaxU32>> =
            potion.targets.iter().copied().map(NonMaxU32::new).collect();

        match targets.is_empty() {
            true => moves.push(Decision::UsePotion {
                index,
                target: None,
            }),
            false => moves.extend(
                targets
                    .into_iter()
                    .map(|target| Decision::UsePotion { index, target }),
            ),
        }
    }

    moves
}

/// Taking a room's options, and buying a shop's stock.
///
/// Which of the two a room offers is the room's kind. So both lists come from
/// [`choose`] rather than from reading the same option two ways.
fn room_moves<V: ContentVersion>(room: &sts2_core::Room<V>) -> Vec<Decision<V>> {
    let options = choose::choosable(room)
        .into_iter()
        .map(|index| Decision::ChooseOption { index });

    let entries = choose::buyable(room)
        .into_iter()
        .map(|index| Decision::Buy { index });

    options.chain(entries).collect()
}

/// Every playable card against every target it takes, and the turn.
///
/// Ending the turn is always legal in the play phase, and it has to be in the
/// list. Otherwise a fuzzer holding an unplayable hand has nothing to do.
fn combat_moves<V: ContentVersion>(combat: &Combat<V>) -> Vec<Decision<V>> {
    let mut moves: Vec<Decision<V>> = Vec::new();

    for (index, card) in combat.legal.iter().enumerate() {
        // No reason is the whole of playable: `CardModel.CanPlay` returns
        // `reason == None`.
        if card.reason.is_some() {
            continue;
        }
        let Ok(index) = u8::try_from(index) else {
            continue;
        };

        let targets: Vec<Option<NonMaxU32>> =
            card.targets.iter().copied().map(NonMaxU32::new).collect();

        match targets.is_empty() {
            true => moves.push(Decision::PlayCard {
                index,
                target: None,
            }),
            false => moves.extend(
                targets
                    .into_iter()
                    .map(|target| Decision::PlayCard { index, target }),
            ),
        }
    }

    moves.push(Decision::EndTurn);
    moves
}

/// Every map point reachable from here, or the way out of the act.
///
/// The two are exclusive by construction. A beaten boss has no children, so the
/// travelable set is empty exactly when proceeding is the move.
fn travel_moves<V: ContentVersion>(map: &Map<V>) -> Vec<Decision<V>> {
    if map.proceed {
        return vec![Decision::Proceed];
    }

    map.travelable
        .iter()
        .map(|p| Decision::Travel { to: p.at })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::legal;
    use crate::fixture::{self, playable, unplayable};
    use serde_json::json;
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_core::{Answer, Coord, Decision, TakeReward};

    type V = V0_107_1;

    /// The moves an observation with this much bent into it offers.
    fn moves(overlay: serde_json::Value) -> Vec<Decision<V>> {
        legal::<V>(&fixture::applied(overlay))
    }

    /// The moves a whole reply offers, for a pending, which carries no room.
    fn answering(reply: serde_json::Value) -> Vec<Decision<V>> {
        legal::<V>(&fixture::answered(reply))
    }

    /// The belt is believed rather than second-guessed. What is drinkable is
    /// what the observation said is drinkable, at the targets it listed.
    #[test]
    fn a_drinkable_potion_is_a_move_at_every_target_it_takes() {
        let out = moves(json!({
            "potions": [
                {"id": "FIRE_POTION", "usable": true, "reason": null,
                 "targets": [1, 2]},
                {"id": "BLOOD_POTION", "usable": false, "reason": "OutOfCombat",
                 "targets": []},
                {"id": "ASHWATER", "usable": true, "reason": null, "targets": []},
            ]
        }));

        assert!(out.contains(&Decision::UsePotion {
            index: 0,
            target: nonmax::NonMaxU32::new(1),
        }));
        assert!(out.contains(&Decision::UsePotion {
            index: 0,
            target: nonmax::NonMaxU32::new(2),
        }));
        // No target at all, which is what an untargeted potion sends.
        assert!(out.contains(&Decision::UsePotion {
            index: 2,
            target: None,
        }));
        assert!(
            !out.iter()
                .any(|m| matches!(m, Decision::UsePotion { index: 1, .. })),
            "a potion the game said cannot be drunk was offered: {out:?}"
        );
    }

    /// Drinking is not one more branch that shadows the others. A potion the
    /// belt calls drinkable is drinkable wherever the run is standing.
    #[test]
    fn a_potion_is_offered_beside_whatever_else_there_is() {
        let belt = json!([{"id": "ASHWATER", "usable": true, "reason": null, "targets": []}]);
        let drink = Decision::UsePotion {
            index: 0,
            target: None,
        };

        let in_a_room = moves(json!({
            "potions": belt,
            "room": {"type": "Event", "id": "WELLSPRING",
                     "options": [{"index": 0, "key": "DRINK"}]}
        }));
        assert!(in_a_room.contains(&drink), "{in_a_room:?}");
        assert!(in_a_room.contains(&Decision::ChooseOption { index: 0 }));

        let on_the_map = moves(json!({
            "potions": belt,
            "map": {
                "at": "1,2", "visited": ["1,2"],
                "travelable": [{"at": "0,3", "type": "Monster"}],
                "points": [{"at": "1,2", "type": "Monster", "to": ["0,3"]},
                           {"at": "0,3", "type": "Monster", "to": []}],
                "proceed": false
            }
        }));
        assert!(on_the_map.contains(&drink), "{on_the_map:?}");
        assert!(on_the_map.contains(&Decision::Travel {
            to: Coord::new(0, 3)
        }));
    }

    /// A card that names a target is a different move per target.
    ///
    /// Collapsing them to one leaves a driver unable to express "hit the one
    /// that is nearly dead". That is most of what a target choice is for.
    #[test]
    fn a_targeted_card_is_one_move_per_target() {
        let out = moves(json!({
            "inCombat": true,
            "combat": fixture::combat(json!({
                "hand": ["STRIKE_IRONCLAD", "DEFEND_IRONCLAD", "CLASH"],
                "legal": [
                    playable("STRIKE_IRONCLAD", &[1, 2]),
                    playable("DEFEND_IRONCLAD", &[]),
                    unplayable("CLASH", "BlockedByCardLogic")
                ]
            }))
        }));

        assert_eq!(out.len(), 4, "{out:?}");
        assert!(out.contains(&Decision::PlayCard {
            index: 0,
            target: nonmax::NonMaxU32::new(2)
        }));
        // No target to name, so exactly one move rather than none.
        assert!(out.contains(&Decision::PlayCard {
            index: 1,
            target: None
        }));
        // Unplayable is not a move, and the turn always is.
        assert!(out.contains(&Decision::EndTurn));
    }

    /// A room's options come before the map, because the protocol says so.
    ///
    /// Offering a `travel` while a room is still asking produces a move that is
    /// legal-looking and refused. That is the failure this ordering exists to
    /// prevent.
    #[test]
    fn an_unfinished_room_hides_the_map() {
        let option = |index: u8, chosen: bool, locked: bool| {
            json!({"index": index, "key": "K", "title": null, "text": null,
                   "locked": locked, "proceed": false, "chosen": chosen, "relic": null})
        };
        let state = json!({
            "room": {"type": "Event", "id": "WELLSPRING", "finished": false,
                     "title": null, "text": null,
                     "options": [option(0, false, false),
                                 option(1, true, false),
                                 option(2, false, true)]},
            "map": {"at": "3,0", "visited": [],
                    "travelable": [{"at": "2,1", "type": "Monster"}]}
        });

        let out = moves(state.clone());
        assert_eq!(out, vec![Decision::ChooseOption { index: 0 }], "{out:?}");

        // With the room done, the same observation offers the map.
        let mut done = state;
        done["room"]["options"] = json!([]);
        let out = moves(done);
        assert_eq!(
            out,
            vec![Decision::Travel {
                to: Coord::new(2, 1)
            }],
            "{out:?}"
        );
    }

    /// A beaten act boss has nowhere to travel, and one way out.
    ///
    /// Without this a run reads as finished on the floor before act two. The
    /// map is right that there is nowhere to go, and the move that leaves is
    /// not a map move at all.
    #[test]
    fn a_finished_act_offers_the_way_out_of_it() {
        let out = moves(json!({
            "map": {"at": "3,16", "visited": ["3,16"], "travelable": [], "proceed": true}
        }));

        assert_eq!(out, vec![Decision::Proceed], "{out:?}");
    }

    /// A card reward is refused without a card index, so each card is a move.
    #[test]
    fn every_card_in_a_reward_is_its_own_move() {
        let out = answering(json!({
            "type": "pending", "seq": 7, "cmd": "end_turn", "digest": "AA",
            "state": {},
            "kind": "rewards", "skippable": true,
            "rewards": [
                {"index": 0, "type": "GoldReward", "populated": true, "taken": false,
                 "amount": 25},
                {"index": 1, "type": "CardReward", "populated": true, "taken": false,
                 "cards": [{"card": "ANGER"}, {"card": "ARMAMENTS"}]},
                {"index": 2, "type": "PotionReward", "populated": true, "taken": true,
                 "potion": "FLEX_POTION"}
            ]
        }));

        // Gold, two cards, and a skip. The taken potion is not a move.
        assert_eq!(out.len(), 4, "{out:?}");
        assert!(
            out.contains(&Decision::Answer(Answer::Take(vec![TakeReward::Card {
                index: 1,
                card: 1
            }])))
        );
        assert!(out.contains(&Decision::Answer(Answer::Skip)));
    }

    /// Only what the shop will actually sell.
    #[test]
    fn a_shop_offers_what_is_stocked_and_affordable() {
        let entry = |index: u8, kind: &str, stocked: bool, affordable: bool| {
            json!({"index": index, "kind": kind, "id": null, "cost": 50,
                   "stocked": stocked, "affordable": affordable, "blocked": null})
        };
        let out = moves(json!({
            "room": {"type": "Shop", "entries": [
                entry(0, "MerchantCardEntry", true, true),
                entry(1, "MerchantCardEntry", true, false),
                entry(2, "MerchantRelicEntry", false, true)
            ]}
        }));

        assert_eq!(out, vec![Decision::Buy { index: 0 }], "{out:?}");
    }

    /// Every legal size of selection, so a driver can decline one that allows it.
    #[test]
    fn a_card_selection_offers_every_size_it_allows() {
        let out = answering(json!({
            "type": "pending", "seq": 3, "cmd": "play_card", "digest": "AA",
            "state": {},
            "kind": "select_cards", "min": 0, "max": 3, "asker": "DISCARD",
            "options": [{"index": 0, "card": "ANGER"}, {"index": 1, "card": "ARMAMENTS"}]
        }));

        // Nought, one or two — `max` is 3 but only two cards are there.
        assert_eq!(out.len(), 3, "{out:?}");
        assert!(out.contains(&Decision::Answer(Answer::Cards(vec![]))));
        assert!(out.contains(&Decision::Answer(Answer::Cards(vec![0, 1]))));
    }
}
