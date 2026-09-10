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

use num_traits::Zero;
use sts2_core::{
    CardTable, Combat, CombatId, Cost, Creature, Fight, NonMaxU32, Reason, Rules, SideKinds,
};

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
    index: u8,
    target: Option<NonMaxU32>,
) -> Result<Play<V>, Reason<V>> {
    let at = usize::from(index);
    let playable = combat.legal.get(at).ok_or(Reason::OutOfRange {
        index: i64::from(index),
        count: combat.hand.len(),
    })?;

    // The id the decision named, which the observation reported and the two
    // refusals below quote back.
    let named = target.map(|target| target.get());

    // The creature it names, which every command the card issues is pointed
    // at. Found once, here, so a rule never carries a bare number.
    let aimed = match named {
        None => None,
        Some(id) => Some(
            combat
                .creatures
                .iter()
                .find(|creature| creature.id == id)
                .ok_or(Reason::NoSuchCreature { id })?
                .combat_id(),
        ),
    };

    if let Some(why) = playable.reason {
        return Err(Reason::Unplayable {
            card: playable.card,
            why,
            preventer: playable.preventer,
        });
    }

    // `CanPlayTargeting`: the card is pointed at one of the creatures its own
    // target type accepts, and a card that names one is pointed at something.
    if !named.is_some_and(|id| playable.targets.contains(&id)) && !playable.targets.is_empty() {
        return Err(Reason::WrongTarget {
            card: playable.card,
            target: named,
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
        target: aimed,
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
    let mut board = Board {
        combat: &mut next,
        target: play.target,
    };
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

/// The fight a card's own behaviour acts on.
///
/// Everything it does is one of the game's commands with nothing listening.
/// This build's fights have no power, relic or card that hooks any of them
/// yet. A command that needs one stops the run where it stands.
struct Board<'a, V: Rules> {
    combat: &'a mut Combat<V>,
    target: Option<CombatId>,
}

impl<V: Rules> Board<'_, V> {
    /// Where a creature sits, or nothing for one no longer in the fight.
    fn at(&self, at: CombatId) -> Option<usize> {
        self.combat
            .creatures
            .iter()
            .position(|creature| creature.combat_id() == at)
    }

    /// The player, which is the only creature a card is played from.
    fn player(&mut self) -> &mut Creature<V> {
        self.combat
            .creatures
            .iter_mut()
            .find(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER)
            .expect("a fight has the player in it")
    }

    /// `KillWithoutCheckingWinCondition`, for a creature the damage killed.
    fn kill(&mut self, at: usize) {
        self.combat.creatures[at].alive = false;
        if self.combat.creatures[at].side != <V::CombatSide as SideKinds>::ENEMY {
            unimplemented!("a player dying in a fight");
        }
        // A dead monster is taken out of the fight, so nothing that reads it
        // afterwards can see it. That is why the observation stops reporting
        // it rather than reporting it dead.
        self.combat.creatures.remove(at);
        if !self
            .combat
            .creatures
            .iter()
            .any(|creature| creature.side == <V::CombatSide as SideKinds>::ENEMY)
        {
            unimplemented!("the end of a fight");
        }
    }
}

impl<V: Rules> Fight<V> for Board<'_, V> {
    fn target(&self) -> CombatId {
        // [`resolve`] refuses a play whose card names a creature and was
        // pointed at none, so a card that reads this was pointed at one.
        self.target
            .expect("a card that names a target is played at one")
    }

    fn attack(&mut self, at: CombatId, damage: V::Number) {
        // `CreatureCmd.Damage`: block first, then hit points, then the death
        // the hit caused. Nothing modifies the amount on the way.
        let Some(at) = self.at(at) else { return };
        if !self.combat.creatures[at].alive {
            return;
        }
        let zero = V::Number::zero();
        let blocked = self.combat.creatures[at].block.min(damage).max(zero);
        self.combat.creatures[at].block = self.combat.creatures[at].block - blocked;
        let unblocked = (damage - blocked).max(zero);

        let hp = self.combat.creatures[at].hp;
        let killed = hp > zero && unblocked >= hp;
        self.combat.creatures[at].hp = (hp - unblocked).max(zero);
        if killed {
            self.kill(at);
        }
    }

    fn gain_block(&mut self, amount: V::Number) {
        // `GainBlock` clamps the modified amount at zero and only then adds.
        let amount = amount.max(V::Number::zero());
        if amount > V::Number::zero() {
            let player = self.player();
            player.block = player.block + amount;
        }
    }

    fn gain_stars(&mut self, amount: V::Number) {
        // The pool is a count, so a card printing a negative one is spending
        // rather than gaining. No card on this build does.
        let amount = num_traits::NumCast::from(amount)
            .unwrap_or_else(|| unimplemented!("a card spending stars as it plays"));
        self.combat.stars = self.combat.stars + amount;
    }

    fn apply_power(&mut self, at: CombatId, _power: V::Power, amount: V::Number) {
        // `PowerCmd.Apply` returns on a zero amount, and on a creature that
        // cannot receive powers. A monster the fight has taken out is one,
        // because its combat state is gone.
        if amount.is_zero() || self.at(at).is_none() {
            return;
        }
        unimplemented!("a power on a creature still in the fight");
    }
}
