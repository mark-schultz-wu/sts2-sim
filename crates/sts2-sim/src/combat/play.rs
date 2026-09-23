//! Playing one card.
//!
//! A port of `PlayCardAction.ExecuteAction` and the wrapper it calls. The
//! order is the game's. The play is checked, the pools are spent, and the card
//! leaves the hand. Its own `OnPlay` runs. Only then does the card land in the
//! pile it ends in.
//!
//! What each card does is not here. That is per build and lives in
//! `sts2-content`, reached through [`Fight`]. This file is the fight that
//! trait acts on.

use sts2_core::{CardTable, CombatId, Cost, HandIndex, Reason, Rules};

use super::board::Board;
use super::live::{Asked, Asker, CardInstance, Live, Plays, Resuming};
use super::{Error, Fighting, hook, legal, power, relic};

/// A play the fight allows, resolved but not yet made.
///
/// Split from making it the way travel is, so that a card the simulator
/// cannot resolve leaves the fight where it stood.
pub(crate) struct Play<V: Rules> {
    /// Where in the hand it is, which is the card's identity here. A hand
    /// holds two of one card, and only the index tells them apart.
    index: usize,
    /// The creature it was pointed at, or nothing for a card that names none.
    /// Held as the fight names one, because this is where it was found.
    target: Option<CombatId>,
    /// What each pool gives up for it, worked out where the play was approved
    /// rather than again where it is made.
    energy: V::Count,
    stars: V::Count,
}

/// Check one play against the fight, without changing anything.
///
/// The order the game refuses in, which is `Protocol.PlayCard`: the hand
/// index, then the creature id, then the card's own two rules.
pub(crate) fn resolve<V: Rules>(
    live: &Live<V>,
    index: HandIndex,
    target: Option<CombatId>,
) -> Result<Play<V>, Reason<V>> {
    let at = usize::from(index.get());
    let held = live.hand.get(at).ok_or(Reason::OutOfRange {
        index: i64::from(index.get()),
        count: live.hand.len(),
    })?;
    let card = held.ident;
    let (cost, star_cost) = held.costs();

    // The creature the play named has to still be in this fight. An id is
    // chosen against an observation, and a fight moves on from one.
    if let Some(id) = target
        && !live.creatures.iter().any(|creature| creature.id == id)
    {
        return Err(Reason::NoSuchCreature { id });
    }

    // The same rules the reported hand carries, asked of the one card being
    // played. Nothing here builds the rest of the hand, or its previews.
    let takes = legal::target_type::<V>(card, &live.creatures);
    let refused = legal::reasons::<V>(
        card,
        &live.creatures,
        cost,
        star_cost,
        live.energy,
        live.stars,
    );
    if let Some(why) = refused {
        return Err(Reason::Unplayable {
            card,
            why,
            // `BlockedByHook` is the only reason that names one, and nothing
            // in a fight raises it yet.
            preventer: None,
        });
    }
    let targets = legal::targets::<V>(takes, &live.creatures, false);

    // `CanPlayTargeting`: the card is pointed at one of the creatures its own
    // target type accepts, and a card that names one is pointed at something.
    if !target.is_some_and(|id| targets.contains(&id)) && !targets.is_empty() {
        return Err(Reason::WrongTarget {
            card,
            target,
            takes,
        });
    }

    if star_cost == Cost::X {
        // `GetStarCostWithModifiers` is a number, not a pool to drain. No card
        // on this build prices its stars at X.
        unimplemented!("a card priced at X stars");
    }

    // `SpendResources`, worked out where the play was held to the pools rather
    // than again where it is made. `legal::playable` marks a card whose price
    // its pool cannot meet unplayable, and the reason check above has passed.
    const PAYS: &str = "a play the legality pass approved is one the pool pays";
    Ok(Play {
        index: at,
        target,
        energy: legal::spend(cost, live.energy).expect(PAYS),
        stars: legal::spend(star_cost, live.stars).expect(PAYS),
    })
}

/// Put a power on a creature, with no card or move behind it.
///
/// Scenario construction. The command a card applies a power with, reached
/// without a card, so the state it leaves is the state a card leaves.
pub(crate) fn apply_power<V: Rules>(
    fighting: Fighting<'_, V>,
    at: CombatId,
    power: V::Power,
    amount: sts2_core::PowerAmount<V::Number>,
) -> Option<super::Ended> {
    let Fighting {
        live,
        streams,
        relics,
    } = fighting;
    // Nothing here draws. A board is built with the set, and building one
    // leaves every counter where it was.
    let ascension = live.ascension;
    let mut board = Board::for_card(live, streams.combat(), relics, ascension, None, None);
    sts2_core::Fight::apply_power(&mut board, at, power, amount);
    let ended = board.ended;
    let skipped = std::mem::take(&mut board.skipped);

    live.skip_tick = merged::<V>(&live.skip_tick, skipped);
    if ended == Some(super::Ended::Won) {
        relic::combat_ended::<V>(relics);
    }
    ended
}

/// The skipped ticks a fight already held, and the ones an effect just added.
pub(super) fn merged<V: Rules>(
    held: &[(CombatId, V::Power)],
    added: Vec<(CombatId, V::Power)>,
) -> Vec<(CombatId, V::Power)> {
    let mut out = held.to_vec();
    for one in added {
        if !out.contains(&one) {
            out.push(one);
        }
    }
    out
}

/// Make a resolved play, on the fight where it stands.
///
/// Whatever the play ended, if it ended the fight.
pub(crate) fn play<V: Rules>(
    fighting: Fighting<'_, V>,
    play: Play<V>,
) -> Result<Option<super::Ended>, Error> {
    let Fighting {
        live,
        streams,
        relics,
    } = fighting;

    // `SpendResources`, before the card leaves the hand.
    live.energy = live.energy - play.energy;
    live.stars = live.stars - play.stars;

    // `AddDuringManualCardPlay`: the card is in the play pile, and so out of
    // the hand, while its own effect runs.
    let card = live.hand.remove(play.index);
    let ascension = live.ascension;
    let mut board = Board::for_card(
        live,
        streams.combat(),
        relics,
        ascension,
        play.target,
        Some(&card),
    );
    let played = V::on_play(card.ident.card, &mut board);
    let ended = board.ended;
    let skipped = std::mem::take(&mut board.skipped);
    live.skip_tick = merged::<V>(&live.skip_tick, skipped);

    carry_on::<V>(
        Fighting {
            live,
            streams,
            relics,
        },
        ascension,
        Resuming {
            // `GeneratePlayCount`, worked out once and before the first play.
            plays: Plays {
                done: num_traits::zero(),
                count: card.play_count(),
            },
            asker: Asker::Card(card),
            target: play.target,
            ended,
        },
        played,
    )
}

/// Resume the card play the fight parked on a selection.
pub(crate) fn answer<V: Rules>(
    fighting: Fighting<'_, V>,
    chosen: sts2_core::Picks,
) -> Result<Option<super::Ended>, Error> {
    let Fighting {
        live,
        streams,
        relics,
    } = fighting;
    let asking = live
        .asking
        .take()
        .unwrap_or_else(|| unimplemented!("an answer to a fight that asked nothing"));
    let taken = match &asking.question {
        Asked::Pile {
            pile: from,
            selection,
        } => {
            let held = pile(live, *from);
            let cards: Option<Vec<_>> = chosen
                .iter()
                .map(|index| {
                    held.get(index).map(|card| {
                        let index = num_traits::NumCast::from(index).unwrap_or_else(|| {
                            unimplemented!("a pile deeper than a fight can number")
                        });
                        sts2_core::CardAt::new(card.id, *from, index)
                    })
                })
                .collect();
            let cards =
                cards.unwrap_or_else(|| unimplemented!("a selection naming a card not offered"));
            sts2_core::Taken::Selected(
                selection
                    .validate_answer(cards.into_boxed_slice())
                    .unwrap_or_else(|| {
                        unimplemented!("a selection with the wrong number of cards")
                    }),
            )
        }
        Asked::Offered { offered, skippable } => {
            let chosen = if chosen.is_empty() {
                if !*skippable {
                    unimplemented!("skipping a choice that cannot be skipped");
                }
                None
            } else {
                let Some(only) = chosen.only() else {
                    unimplemented!("a choice taking more than one card");
                };
                Some(*offered.get(only).unwrap_or_else(|| {
                    unimplemented!("a choice naming a card that was not offered")
                }))
            };
            sts2_core::Taken::Chosen(chosen)
        }
    };

    let mut effect = asking.effect;
    let playing = match &effect.asker {
        Asker::Card(card) => Some(*card),
        Asker::Potion(_) => None,
    };
    let ascension = live.ascension;
    let mut board = Board::for_card(
        live,
        streams.combat(),
        relics,
        ascension,
        effect.target,
        playing.as_ref(),
    );
    let played = V::resume_play(&mut board, asking.at, taken);
    effect.ended = effect.ended.or(board.ended);
    let skipped = std::mem::take(&mut board.skipped);
    live.skip_tick = merged::<V>(&live.skip_tick, skipped);
    carry_on(
        Fighting {
            live,
            streams,
            relics,
        },
        ascension,
        effect,
        played,
    )
}

/// Carry a play on from wherever it got to.
///
/// `OnPlay` is a coroutine in the game. A card that has to ask says so and
/// comes back here with the answer. The first call and each resumption use
/// this loop.
pub(super) fn carry_on<V: Rules>(
    fighting: Fighting<'_, V>,
    ascension: V::AscensionLevel,
    mut effect: Resuming<V>,
    mut played: sts2_core::Played<V>,
) -> Result<Option<super::Ended>, Error> {
    let Fighting {
        live: next,
        streams,
        relics,
    } = fighting;
    loop {
        let (at, question, answer) = match played {
            // One turn of `GeneratePlayCount`'s loop is over. A card carrying
            // replays runs its own effect again, from the top.
            sts2_core::Played::Done => {
                effect.plays.done = effect.plays.done + num_traits::one();
                let Asker::Card(card) = &effect.asker else {
                    break;
                };
                if effect.plays.done >= effect.plays.count {
                    break;
                }
                let card = *card;
                let mut board = Board::for_card(
                    next,
                    streams.combat(),
                    relics,
                    ascension,
                    effect.target,
                    Some(&card),
                );
                played = V::on_play(card.ident.card, &mut board);
                effect.ended = effect.ended.or(board.ended);
                let skipped = std::mem::take(&mut board.skipped);
                next.skip_tick = merged::<V>(&next.skip_tick, skipped);
                continue;
            }
            sts2_core::Played::Asking {
                pile,
                selection,
                at,
            } => (
                at,
                Asked::Pile { pile, selection },
                taken::<V>(next, pile, selection).map(sts2_core::Taken::Selected),
            ),
            sts2_core::Played::Choosing {
                offered,
                skippable,
                at,
            } => (at, Asked::Offered { offered, skippable }, None),
        };
        let Some(answer) = answer else {
            let ended = effect.ended;
            next.asking = Some(super::live::Asking {
                effect,
                at,
                question,
            });
            return Ok(ended);
        };

        let playing = match &effect.asker {
            Asker::Card(card) => Some(*card),
            Asker::Potion(_) => None,
        };
        let mut board = Board::for_card(
            next,
            streams.combat(),
            relics,
            ascension,
            effect.target,
            playing.as_ref(),
        );
        played = V::resume_play(&mut board, at, answer);
        effect.ended = effect.ended.or(board.ended);
        let skipped = std::mem::take(&mut board.skipped);
        next.skip_tick = merged::<V>(&next.skip_tick, skipped);
    }

    let ended = effect.ended;
    let Asker::Card(mut card) = effect.asker else {
        if ended == Some(super::Ended::Won) {
            relic::combat_ended::<V>(relics);
        }
        return Ok(ended);
    };

    card.was_played();

    let card_played = card.ident.card;
    // `GetResultPileTypeForCardPlay`, once the effect is over.
    match card
        .ident
        .card
        .power()
        .unwrap_or_else(|| unimplemented!("a card that works its own type out from the fight"))
    {
        // A power card leaves the fight rather than going to a pile.
        true => {}
        false if card.ident.card.exhausts() => next.exhaust.push(card),
        false => next.discard.push(card),
    }

    // `AfterCardPlayed`, once the card has left the play pile. A relic that
    // answers it acts through the board a card acts through, and draws from
    // the same stream, so this is before the streams are written back.
    let mut asked = relic::card_played::<V>(relics, card_played);
    asked.extend(power::notify::<V>(
        next,
        sts2_core::EventHook::CardPlayed { card: card_played },
    ));
    if !asked.is_empty() {
        let mut board = Board::for_card(next, streams.combat(), relics, ascension, None, None);
        hook::apply::<V, _>(&mut board, asked);
        let skipped = std::mem::take(&mut board.skipped);
        next.skip_tick = merged::<V>(&next.skip_tick, skipped);
    }

    if ended == Some(super::Ended::Won) {
        relic::combat_ended::<V>(relics);
    }
    Ok(ended)
}

/// What a selection takes without asking, or nothing for one that is a
/// question.
///
/// `FromHand`: an empty pile selects nothing, and a pile no larger than an
/// exact selection is taken whole. Anything else the player answers.
fn taken<V: Rules>(
    live: &Live<V>,
    from: sts2_core::Pile,
    selection: sts2_core::Selection,
) -> Option<sts2_core::SelectedCards<V>> {
    let held = pile(live, from);
    let cards = held
        .iter()
        .enumerate()
        .map(|(index, card)| {
            let index = num_traits::NumCast::from(index)
                .unwrap_or_else(|| unimplemented!("a pile deeper than a fight can number"));
            sts2_core::CardAt::new(card.id, from, index)
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    selection.validate_automatic(cards)
}

pub(super) fn pile<V: Rules>(live: &Live<V>, pile: sts2_core::Pile) -> &[CardInstance<V>] {
    match pile {
        sts2_core::Pile::Hand => &live.hand,
        sts2_core::Pile::Draw => &live.draw,
        sts2_core::Pile::Discard => &live.discard,
        sts2_core::Pile::Exhaust => &live.exhaust,
    }
}
