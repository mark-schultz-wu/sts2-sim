//! The fight, as behaviour acts on it.
//!
//! One board for both kinds of behaviour. The player plays a card and a
//! monster makes a move. Past that the two issue the game's same commands
//! against the same fight.
//!
//! Everything here is one of those commands with nothing listening. This
//! build's fights hold no power, relic or card that hooks any of them yet. A
//! command that needs one stops the run where it stands.

use num_traits::Zero;
use sts2_core::rng::{GameRng, MegaRng};
use sts2_core::{CardIdent, CardTable, Combat, CombatId, Creature, Fight, Rules, SideKinds};

/// The fight an effect is resolved against.
pub(super) struct Board<'a, V: Rules> {
    combat: &'a mut Combat<V>,
    /// Whose effect this is. The player for a card, the monster for a move.
    actor: CombatId,
    /// The creature a card was pointed at.
    target: Option<CombatId>,
    /// What the acting monster's move advertises it will hit for.
    intent_damage: V::Number,
}

impl<'a, V: Rules> Board<'a, V> {
    /// A board for one card, played from the player.
    pub(super) fn for_card(combat: &'a mut Combat<V>, target: Option<CombatId>) -> Self {
        let actor = player::<V>(&combat.creatures);
        Board {
            combat,
            actor,
            target,
            intent_damage: V::Number::zero(),
        }
    }

    /// A board for one move, made by a monster.
    pub(super) fn for_move(
        combat: &'a mut Combat<V>,
        actor: CombatId,
        intent_damage: V::Number,
    ) -> Self {
        Board {
            combat,
            actor,
            target: None,
            intent_damage,
        }
    }

    /// Where a creature sits, or nothing for one no longer in the fight.
    fn at(&self, at: CombatId) -> Option<usize> {
        self.combat
            .creatures
            .iter()
            .position(|creature| creature.id == at)
    }

    /// Which side the acting creature fights on.
    fn side(&self) -> V::CombatSide {
        self.at(self.actor)
            .map(|at| self.combat.creatures[at].side)
            .unwrap_or(<V::CombatSide as SideKinds>::PLAYER)
    }

    /// Everything on the other side, which is `GetOpponentsOf`.
    fn opponents(&self) -> Vec<CombatId> {
        let side = self.side();
        self.combat
            .creatures
            .iter()
            .filter(|creature| creature.side != side && creature.alive)
            .map(|creature| creature.id)
            .collect()
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

/// The player, which every fight has one of.
pub(super) fn player<V: Rules>(creatures: &[Creature<V>]) -> CombatId {
    creatures
        .iter()
        .find(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER)
        .expect("a fight has the player in it")
        .id
}

impl<V: Rules> Fight<V> for Board<'_, V> {
    fn target(&self) -> CombatId {
        // `resolve` refuses a play whose card names a creature and was pointed
        // at none, so a card that reads this was pointed at one.
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
            let actor = self.actor;
            let Some(at) = self.at(actor) else { return };
            self.combat.creatures[at].block = self.combat.creatures[at].block + amount;
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

    fn intent_damage(&self) -> V::Number {
        self.intent_damage
    }

    fn attack_opponents(&mut self, damage: V::Number) {
        // The list is taken before the first hit lands, which is what the
        // game's own `validTargets` is.
        for id in self.opponents() {
            self.attack(id, damage);
        }
    }

    fn give_cards(&mut self, card: V::Card, count: usize) {
        // Every opponent takes its own copies, into its own discard pile. A
        // fight here has one set of piles, so a second player would need the
        // observation to grow one.
        if self.opponents().len() > 1 {
            unimplemented!("cards given to more than one player");
        }
        for _ in 0..count {
            self.combat.discard.push(CardIdent::new(card));
        }
    }
}

/// `CardPileCmd.Draw`, which shuffles the discard pile back in when the draw
/// pile runs dry.
///
/// A fight with every card in hand draws short rather than manufacture one.
pub(super) fn draw<V: Rules>(combat: &mut Combat<V>, shuffle: &mut MegaRng, count: usize) {
    for _ in 0..count {
        if combat.draw.is_empty() {
            if combat.discard.is_empty() {
                return;
            }
            reshuffle(combat, shuffle);
        }
        combat.hand.push(combat.draw.remove(0));
    }
}

/// `CardPileCmd.Shuffle`: the discard pile and whatever is left of the draw
/// pile, shuffled together into a new draw pile.
///
/// A stable shuffle, unlike the one that deals a fight its opening hand. The
/// two are different sequences over the same stream, so which one this is
/// decides every card drawn after it.
fn reshuffle<V: Rules>(combat: &mut Combat<V>, shuffle: &mut MegaRng) {
    let mut shuffled = std::mem::take(&mut combat.discard);
    shuffled.append(&mut combat.draw);
    shuffle.stable_shuffle_by_key(&mut shuffled, |held| (held.card.order(), held.upgrade));
    combat.draw = shuffled;
}
