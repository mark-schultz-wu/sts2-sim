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

use sts2_core::{CardTable, Combat, CombatId, Cost, HandIndex, Reason, Rules};

use super::board::Board;
use super::{Error, legal};

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
    combat: &Combat<V>,
    index: HandIndex,
    target: Option<CombatId>,
) -> Result<Play<V>, Reason<V>> {
    let at = usize::from(index.get());
    let playable = combat.legal.get(at).ok_or(Reason::OutOfRange {
        index: i64::from(index.get()),
        count: combat.hand.len(),
    })?;

    // The creature the play named has to still be in this fight. An id is
    // chosen against an observation, and a fight moves on from one.
    if let Some(id) = target
        && !combat.creatures.iter().any(|creature| creature.id == id)
    {
        return Err(Reason::NoSuchCreature { id });
    }

    if let Some(why) = playable.reason {
        return Err(Reason::Unplayable {
            card: playable.card,
            why,
            preventer: playable.preventer,
        });
    }

    // `CanPlayTargeting`: the card is pointed at one of the creatures its own
    // target type accepts, and a card that names one is pointed at something.
    if !target.is_some_and(|id| playable.targets.contains(&id)) && !playable.targets.is_empty() {
        return Err(Reason::WrongTarget {
            card: playable.card,
            target,
            takes: playable.target,
        });
    }

    let star_cost = playable.card.card.star_cost();
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
        energy: legal::spend(playable.card.card.cost(), combat.energy).expect(PAYS),
        stars: legal::spend(star_cost, combat.stars).expect(PAYS),
    })
}

/// Make a resolved play, and give back the fight it leaves behind.
///
/// Nothing is committed until this returns. It works on a copy, so a card
/// asking for a rule the simulator lacks costs the caller nothing.
pub(crate) fn play<V: Rules>(combat: &Combat<V>, play: Play<V>) -> Result<Combat<V>, Error> {
    let mut next = combat.clone();

    // `SpendResources`, before the card leaves the hand.
    next.energy = next.energy - play.energy;
    next.stars = next.stars - play.stars;

    // `AddDuringManualCardPlay`: the card is in the play pile, and so out of
    // the hand, while its own effect runs.
    let card = next.hand.remove(play.index);
    let mut board = Board::for_card(&mut next, play.target);
    V::on_play(card.card, &mut board);

    // `GetResultPileTypeForCardPlay`, once the effect is over.
    match card
        .card
        .power()
        .unwrap_or_else(|| unimplemented!("a card that works its own type out from the fight"))
    {
        // A power card leaves the fight rather than going to a pile.
        true => {}
        false if card.card.exhausts() => next.exhaust.push(card),
        false => next.discard.push(card),
    }

    next.legal = legal::hand::<V>(&next.hand, &next.creatures, next.energy, next.stars)?;
    Ok(next)
}
