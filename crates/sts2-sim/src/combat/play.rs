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
    CardIdent, CardTable, Combat, Cost, Creature, Fight, Printed, Reason, Rules, SideKinds,
};

use super::{Error, legal};

/// A play the fight allows, resolved but not yet made.
///
/// Split from making it the way travel is, so that a card the simulator
/// cannot resolve leaves the fight where it stood.
pub(crate) struct Play<V: Rules> {
    /// Where in the hand it is. The hand can hold two of one card, so the
    /// index is the identity and the card beside it is a convenience.
    index: usize,
    card: CardIdent<V>,
    target: Option<u32>,
}

/// Check one play against the fight, without changing anything.
///
/// The order the game refuses in, which is `Protocol.PlayCard`: the hand
/// index, then the creature id, then the card's own two rules.
pub(crate) fn resolve<V: Rules>(
    combat: &Combat<V>,
    index: u8,
    target: Option<u32>,
) -> Result<Play<V>, Reason<V>> {
    let at = usize::from(index);
    let playable = combat.legal.get(at).ok_or(Reason::OutOfRange {
        index: i64::from(index),
        count: combat.hand.len(),
    })?;

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

    Ok(Play {
        index: at,
        card: playable.card,
        target,
    })
}

/// Make a resolved play, and give back the fight it leaves behind.
///
/// Nothing is committed until this returns. It works on a copy, so a card
/// asking for a rule the simulator lacks costs the caller nothing.
pub(crate) fn play<V: Rules>(combat: &Combat<V>, play: Play<V>) -> Result<Combat<V>, Error> {
    let mut next = combat.clone();

    // `SpendResources`, before the card leaves the hand. A price this build's
    // count cannot hold got the card refused as unplayable well before here.
    let spent = |cost, pool| legal::spend(cost, pool).expect("a legal play is one this pool pays");
    let energy = spent(play.card.card.cost(), next.energy);
    let stars = match play.card.card.star_cost() {
        // `GetStarCostWithModifiers` is a number, not a pool to drain. No card
        // on this build prices its stars at X.
        Cost::X => unimplemented!("a card priced at X stars"),
        other => spent(other, next.stars),
    };
    next.energy = next.energy - energy;
    next.stars = next.stars - stars;

    // `AddDuringManualCardPlay`: the card is in the play pile, and so out of
    // the hand, while its own effect runs.
    let card = next.hand.remove(play.index);
    let vars = card.card.vars();

    let mut board = Board {
        combat: &mut next,
        target: play.target,
        vars,
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
    target: Option<u32>,
    vars: &'static [Printed<V::CardVar, <V::Card as CardTable>::Number>],
}

impl<V: Rules> Board<'_, V> {
    /// Where a creature sits, or nothing for one no longer in the fight.
    fn at(&self, id: u32) -> Option<usize> {
        self.combat
            .creatures
            .iter()
            .position(|creature| creature.id == id)
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
    fn target(&self) -> Option<u32> {
        self.target
    }

    fn var(&mut self, key: V::CardVar) -> V::Number {
        // A key the card does not print is a mistake in the port. A printed
        // number this build's game number cannot hold is the same mistake
        // wearing a different hat: the card cannot act on what it read.
        self.vars
            .iter()
            .find(|var| var.id == key)
            .and_then(|var| num_traits::NumCast::from(var.base))
            .expect("a card reads a number it prints")
    }

    fn attack(&mut self, at: u32, damage: V::Number) {
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

    fn apply_power(&mut self, at: u32, _power: V::Power, amount: V::Number) {
        // `PowerCmd.Apply` returns on a zero amount, and on a creature that
        // cannot receive powers. A monster the fight has taken out is one,
        // because its combat state is gone.
        if amount.is_zero() || self.at(at).is_none() {
            return;
        }
        unimplemented!("a power on a creature still in the fight");
    }
}
