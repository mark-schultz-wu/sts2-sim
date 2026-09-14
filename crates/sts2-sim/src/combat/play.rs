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

use sts2_core::{CardTable, CombatId, Cost, Counters, FlagSet, HandIndex, Reason, Rules};

use super::board::Board;
use super::live::Live;
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
    let mut shuffle = stream::seek(raw_seed, &counters, V::shuffle_rng(), "shuffle")?;
    let mut niche = stream::seek(raw_seed, &counters, V::niche_rng(), "niche")?;

    let mut board = Board::for_card(
        &mut next,
        &mut shuffle,
        &mut niche,
        live.ascension,
        None,
        None,
    );
    sts2_core::Fight::apply(&mut board, at, power, amount);
    let won = board.won;
    let skipped = std::mem::take(&mut board.skipped);

    next.skip_tick = merged::<V>(&next.skip_tick, skipped);
    stream::record(&mut counters, V::shuffle_rng(), "shuffle", &shuffle)?;
    stream::record(&mut counters, V::niche_rng(), "niche", &niche)?;
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
    let mut relics = relics.to_vec();
    let mut counters = counters.clone();
    let mut shuffle = stream::seek(raw_seed, &counters, V::shuffle_rng(), "shuffle")?;
    let mut niche = stream::seek(raw_seed, &counters, V::niche_rng(), "niche")?;

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
    V::on_play(card.ident.card, &mut board);
    let board_won = board.won;
    let skipped = std::mem::take(&mut board.skipped);

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

    next.skip_tick = merged::<V>(&next.skip_tick, skipped);
    stream::record(&mut counters, V::shuffle_rng(), "shuffle", &shuffle)?;
    stream::record(&mut counters, V::niche_rng(), "niche", &niche)?;

    // `AfterCardPlayed`, once the card has left the play pile.
    relic::card_played::<V>(&mut relics);
    if board_won {
        relic::combat_ended::<V>(&mut relics);
    }
    Ok(super::Fought {
        live: next,
        counters,
        relics,
        won: board_won,
    })
}
