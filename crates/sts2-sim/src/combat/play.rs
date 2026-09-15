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

use sts2_core::{CardTable, CombatId, Cost, Counters, FlagSet, HandIndex, Reason, Rules, RunRng};

use crate::rng::MegaRng;

use super::board::Board;
use super::live::{CardInstance, Live};
use super::{Error, legal, relic, stream};

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
    let card = live
        .hand
        .get(at)
        .map(|held| held.ident)
        .ok_or(Reason::OutOfRange {
            index: i64::from(index.get()),
            count: live.hand.len(),
        })?;

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
    let refusals = legal::reasons::<V>(card, &live.creatures, live.energy, live.stars);
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

    let star_cost = card.card.star_cost();
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
        energy: legal::spend(card.card.cost(), live.energy).expect(PAYS),
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
    raw_seed: u32,
    counters: &Counters<V>,
    relics: &[crate::run::HeldRelic<V>],
) -> Result<super::Fought<V>, Error> {
    let mut next = live.clone();
    let mut relics = relics.to_vec();
    let mut counters = counters.clone();
    // Nothing here draws. The stream is what a board is built with, and
    // recording it back leaves the counter where it was.
    let mut shuffle = stream::seek(raw_seed, &counters, V::run_rng(RunRng::Shuffle), "shuffle")?;
    let mut niche = stream::seek(raw_seed, &counters, V::run_rng(RunRng::Niche), "niche")?;

    let mut board = Board::for_card(
        &mut next,
        &mut shuffle,
        &mut niche,
        live.ascension,
        None,
        None,
    );
    sts2_core::Fight::apply_power(&mut board, at, power, amount);
    let won = board.won;
    let skipped = std::mem::take(&mut board.skipped);

    next.skip_tick = merged::<V>(&next.skip_tick, skipped);
    stream::record(
        &mut counters,
        V::run_rng(RunRng::Shuffle),
        "shuffle",
        &shuffle,
    )?;
    stream::record(&mut counters, V::run_rng(RunRng::Niche), "niche", &niche)?;
    if won {
        relic::combat_ended::<V>(&mut relics);
    }
    Ok(super::Fought {
        live: next,
        counters,
        relics,
        won,
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
    raw_seed: u32,
    counters: &Counters<V>,
    relics: &[crate::run::HeldRelic<V>],
) -> Result<super::Fought<V>, Error> {
    let mut next = live.clone();
    let relics = relics.to_vec();
    let counters = counters.clone();
    let mut shuffle = stream::seek(raw_seed, &counters, V::run_rng(RunRng::Shuffle), "shuffle")?;
    let mut niche = stream::seek(raw_seed, &counters, V::run_rng(RunRng::Niche), "niche")?;

    // `SpendResources`, before the card leaves the hand.
    next.energy = next.energy - play.energy;
    next.stars = next.stars - play.stars;

    // `AddDuringManualCardPlay`: the card is in the play pile, and so out of
    // the hand, while its own effect runs.
    let card = next.hand.remove(play.index);
    let mut board = Board::for_card(
        &mut next,
        &mut shuffle,
        &mut niche,
        live.ascension,
        play.target,
        Some(&card),
    );
    let played = V::on_play(card.ident.card, &mut board);
    let won = board.won;
    let skipped = std::mem::take(&mut board.skipped);
    next.skip_tick = merged::<V>(&next.skip_tick, skipped);

    carry_on::<V>(
        next,
        shuffle,
        niche,
        counters,
        relics,
        live.ascension,
        card,
        play.target,
        played,
        won,
    )
}

/// Resume the card play the fight parked on a selection.
pub(crate) fn answer<V: Rules>(
    live: &Live<V>,
    chosen: &[usize],
    raw_seed: u32,
    counters: &Counters<V>,
    relics: &[crate::run::HeldRelic<V>],
) -> Result<super::Fought<V>, Error> {
    let mut next = live.clone();
    let asking = next
        .asking
        .take()
        .unwrap_or_else(|| unimplemented!("an answer to a fight that asked nothing"));
    let held = pile(&next, asking.pile);
    let cards: Option<Vec<_>> = chosen
        .iter()
        .map(|&index| {
            held.get(index).map(|card| {
                let index = num_traits::NumCast::from(index)
                    .unwrap_or_else(|| unimplemented!("a pile deeper than a fight can number"));
                sts2_core::CardAt::new(card.id, asking.pile, index)
            })
        })
        .collect();
    let cards = cards.unwrap_or_else(|| unimplemented!("a selection naming a card not offered"));
    let distinct = cards
        .iter()
        .map(|card| card.id())
        .collect::<std::collections::HashSet<_>>();
    if chosen.len() != distinct.len() {
        unimplemented!("a selection naming the same card twice");
    }
    let cards = asking
        .selection
        .validate_answer(cards.into_boxed_slice())
        .unwrap_or_else(|| unimplemented!("a selection with the wrong number of cards"));

    let mut shuffle = stream::seek(raw_seed, counters, V::run_rng(RunRng::Shuffle), "shuffle")?;
    let mut niche = stream::seek(raw_seed, counters, V::run_rng(RunRng::Niche), "niche")?;
    let mut board = Board::for_card(
        &mut next,
        &mut shuffle,
        &mut niche,
        live.ascension,
        asking.target,
        Some(&asking.card),
    );
    let played = V::resume_play(&mut board, asking.at, cards);
    let won = asking.won || board.won;
    let skipped = std::mem::take(&mut board.skipped);
    next.skip_tick = merged::<V>(&next.skip_tick, skipped);
    carry_on(
        next,
        shuffle,
        niche,
        counters.clone(),
        relics.to_vec(),
        live.ascension,
        asking.card,
        asking.target,
        played,
        won,
    )
}

/// Carry a play on from wherever it got to.
///
/// `OnPlay` is a coroutine in the game. A card that has to ask says so and
/// comes back here with the answer. The first call and each resumption use
/// this loop.
#[allow(clippy::too_many_arguments)]
fn carry_on<V: Rules>(
    mut next: Live<V>,
    mut shuffle: MegaRng,
    mut niche: MegaRng,
    mut counters: Counters<V>,
    mut relics: Vec<crate::run::HeldRelic<V>>,
    ascension: V::AscensionLevel,
    card: CardInstance<V>,
    target: Option<CombatId>,
    mut played: sts2_core::Played<V>,
    mut won: bool,
) -> Result<super::Fought<V>, Error> {
    while let sts2_core::Played::Asking {
        pile,
        selection,
        at,
    } = played
    {
        // `CardSelectCmd.FromHand` and its kin. The pile is taken whole when
        // it holds no more than the selection, and an empty one selects
        // nothing. Neither is a question, so neither stops the fight.
        let Some(chosen) = taken::<V>(&next, pile, selection) else {
            stream::record(
                &mut counters,
                V::run_rng(RunRng::Shuffle),
                "shuffle",
                &shuffle,
            )?;
            stream::record(&mut counters, V::run_rng(RunRng::Niche), "niche", &niche)?;
            next.asking = Some(super::live::Asking {
                card,
                target,
                at,
                pile,
                selection,
                won,
            });
            return Ok(super::Fought {
                live: next,
                counters,
                relics,
                won,
            });
        };

        let mut board = Board::for_card(
            &mut next,
            &mut shuffle,
            &mut niche,
            ascension,
            target,
            Some(&card),
        );
        played = V::resume_play(&mut board, at, chosen);
        won |= board.won;
        let skipped = std::mem::take(&mut board.skipped);
        next.skip_tick = merged::<V>(&next.skip_tick, skipped);
    }

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
    let asked = relic::card_played::<V>(&mut relics, card_played);
    if !asked.is_empty() {
        let mut board = Board::for_card(&mut next, &mut shuffle, &mut niche, ascension, None, None);
        relic::apply::<V, _>(&mut board, asked);
        let skipped = std::mem::take(&mut board.skipped);
        next.skip_tick = merged::<V>(&next.skip_tick, skipped);
    }

    stream::record(
        &mut counters,
        V::run_rng(RunRng::Shuffle),
        "shuffle",
        &shuffle,
    )?;
    stream::record(&mut counters, V::run_rng(RunRng::Niche), "niche", &niche)?;
    if won {
        relic::combat_ended::<V>(&mut relics);
    }
    Ok(super::Fought {
        live: next,
        counters,
        relics,
        won,
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
