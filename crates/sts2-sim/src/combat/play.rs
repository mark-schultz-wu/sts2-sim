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

use sts2_core::{CardTable, CombatId, Cost, FlagSet, HandIndex, Reason, Rules};

use super::board::Board;
use super::live::{Asked, Asker, CardInstance, Live, Plays, Resuming};
use super::{Error, hook, legal, power, relic};
use crate::stream::Streams;

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
    let refusals = legal::reasons::<V>(
        card,
        &live.creatures,
        cost,
        star_cost,
        live.energy,
        live.stars,
    );
    if !refusals.is_empty() {
        return Err(Reason::Unplayable {
            card,
            why: <V::UnplayableReasons as FlagSet>::from_members(&refusals),
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
    live: &Live<V>,
    at: CombatId,
    power: V::Power,
    amount: sts2_core::PowerAmount<V::Number>,
    streams: &Streams<V>,
    relics: &[crate::run::HeldRelic<V>],
) -> Result<super::Fought<V>, Error> {
    let mut next = live.clone();
    let mut relics = relics.to_vec();
    // Nothing here draws. A board is built with the set, and building one
    // leaves every counter where it was.
    let mut streams = streams.clone();

    let mut board = Board::for_card(
        &mut next,
        streams.combat(),
        &mut relics,
        live.ascension,
        None,
        None,
    );
    sts2_core::Fight::apply_power(&mut board, at, power, amount);
    let ended = board.ended;
    let skipped = std::mem::take(&mut board.skipped);

    next.skip_tick = merged::<V>(&next.skip_tick, skipped);
    if ended == Some(super::Ended::Won) {
        relic::combat_ended::<V>(&mut relics);
    }
    Ok(super::Fought {
        live: next,
        streams,
        relics,
        ended,
    })
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

/// Make a resolved play, and give back the fight it leaves behind.
///
/// Nothing is committed until this returns. It works on a copy, so a card
/// asking for a rule the simulator lacks costs the caller nothing.
pub(crate) fn play<V: Rules>(
    live: &Live<V>,
    play: Play<V>,
    streams: &Streams<V>,
    relics: &[crate::run::HeldRelic<V>],
) -> Result<super::Fought<V>, Error> {
    let mut next = live.clone();
    let mut relics = relics.to_vec();
    let mut streams = streams.clone();

    // `SpendResources`, before the card leaves the hand.
    next.energy = next.energy - play.energy;
    next.stars = next.stars - play.stars;

    // `AddDuringManualCardPlay`: the card is in the play pile, and so out of
    // the hand, while its own effect runs.
    let card = next.hand.remove(play.index);
    let mut board = Board::for_card(
        &mut next,
        streams.combat(),
        &mut relics,
        live.ascension,
        play.target,
        Some(&card),
    );
    let played = V::on_play(card.ident.card, &mut board);
    let ended = board.ended;
    let skipped = std::mem::take(&mut board.skipped);
    next.skip_tick = merged::<V>(&next.skip_tick, skipped);

    carry_on::<V>(
        next,
        streams,
        relics,
        live.ascension,
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
    live: &Live<V>,
    chosen: &[usize],
    streams: &Streams<V>,
    relics: &[crate::run::HeldRelic<V>],
) -> Result<super::Fought<V>, Error> {
    let mut next = live.clone();
    let asking = next
        .asking
        .take()
        .unwrap_or_else(|| unimplemented!("an answer to a fight that asked nothing"));
    let taken = match &asking.question {
        Asked::Pile {
            pile: from,
            selection,
        } => {
            let held = pile(&next, *from);
            let cards: Option<Vec<_>> = chosen
                .iter()
                .map(|&index| {
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
            let distinct = cards
                .iter()
                .map(|card| card.id())
                .collect::<std::collections::HashSet<_>>();
            if chosen.len() != distinct.len() {
                unimplemented!("a selection naming the same card twice");
            }
            sts2_core::Taken::Selected(
                selection
                    .validate_answer(cards.into_boxed_slice())
                    .unwrap_or_else(|| {
                        unimplemented!("a selection with the wrong number of cards")
                    }),
            )
        }
        Asked::Offered { offered, skippable } => {
            let chosen = match chosen {
                [] if *skippable => None,
                [] => unimplemented!("skipping a choice that cannot be skipped"),
                [only] => Some(*offered.get(*only).unwrap_or_else(|| {
                    unimplemented!("a choice naming a card that was not offered")
                })),
                _ => unimplemented!("a choice taking more than one card"),
            };
            sts2_core::Taken::Chosen(chosen)
        }
    };

    let mut streams = streams.clone();
    let mut relics = relics.to_vec();
    let mut effect = asking.effect;
    let playing = match &effect.asker {
        Asker::Card(card) => Some(card.clone()),
        Asker::Potion(_) => None,
    };
    let mut board = Board::for_card(
        &mut next,
        streams.combat(),
        &mut relics,
        live.ascension,
        effect.target,
        playing.as_ref(),
    );
    let played = V::resume_play(&mut board, asking.at, taken);
    effect.ended = effect.ended.or(board.ended);
    let skipped = std::mem::take(&mut board.skipped);
    next.skip_tick = merged::<V>(&next.skip_tick, skipped);
    carry_on(
        next,
        streams.clone(),
        relics,
        live.ascension,
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
    mut next: Live<V>,
    mut streams: Streams<V>,
    mut relics: Vec<crate::run::HeldRelic<V>>,
    ascension: V::AscensionLevel,
    mut effect: Resuming<V>,
    mut played: sts2_core::Played<V>,
) -> Result<super::Fought<V>, Error> {
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
                let card = card.clone();
                let mut board = Board::for_card(
                    &mut next,
                    streams.combat(),
                    &mut relics,
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
                taken::<V>(&next, pile, selection).map(sts2_core::Taken::Selected),
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
            return Ok(super::Fought {
                live: next,
                streams,
                relics,
                ended,
            });
        };

        let playing = match &effect.asker {
            Asker::Card(card) => Some(card.clone()),
            Asker::Potion(_) => None,
        };
        let mut board = Board::for_card(
            &mut next,
            streams.combat(),
            &mut relics,
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
            relic::combat_ended::<V>(&mut relics);
        }
        return Ok(super::Fought {
            live: next,
            streams,
            relics,
            ended,
        });
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
    let mut asked = relic::card_played::<V>(&mut relics, card_played);
    asked.extend(power::notify::<V>(
        &mut next,
        sts2_core::EventHook::CardPlayed { card: card_played },
    ));
    if !asked.is_empty() {
        let mut board = Board::for_card(
            &mut next,
            streams.combat(),
            &mut relics,
            ascension,
            None,
            None,
        );
        hook::apply::<V, _>(&mut board, asked);
        let skipped = std::mem::take(&mut board.skipped);
        next.skip_tick = merged::<V>(&next.skip_tick, skipped);
    }

    if ended == Some(super::Ended::Won) {
        relic::combat_ended::<V>(&mut relics);
    }
    Ok(super::Fought {
        live: next,
        streams,
        relics,
        ended,
    })
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
