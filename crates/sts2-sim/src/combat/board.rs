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
use sts2_core::{
    CardIdent, CardTable, Combat, CombatId, Creature, Fight, Holding, PowerAmount, Rules,
    SideKinds, Stack,
};

/// The fight an effect is resolved against.
pub(super) struct Board<'a, V: Rules> {
    combat: &'a mut Combat<V>,
    /// The shuffle stream, because drawing a card can turn the discard pile
    /// back into a draw pile.
    shuffle: &'a mut MegaRng,
    /// Whose effect this is. The player for a card, the monster for a move.
    actor: CombatId,
    /// The creature a card was pointed at.
    target: Option<CombatId>,
    /// What the acting monster's move advertises it will hit for.
    intent_damage: V::Number,
    /// The debuffs this effect put on the player, whose next tick is skipped.
    ///
    /// `PowerCmd.Apply` sets `SkipNextDurationTick` on every debuff that lands
    /// on the player. So a Weak applied during the enemy's turn lasts the
    /// turns it says rather than one fewer. Per instance, and no observation
    /// reports it, so the run keeps it beside the fight.
    pub(super) skipped: Vec<(CombatId, V::Power)>,

    /// The last enemy left the fight while this effect was resolving.
    ///
    /// `CheckWinCondition` runs after the command that killed it, so this is
    /// read once the effect is over rather than acted on where it is set.
    pub(super) won: bool,
}

impl<'a, V: Rules> Board<'a, V> {
    /// A board for one card, played from the player.
    pub(super) fn for_card(
        combat: &'a mut Combat<V>,
        shuffle: &'a mut MegaRng,
        target: Option<CombatId>,
    ) -> Self {
        let actor = player::<V>(&combat.creatures);
        Board {
            combat,
            shuffle,
            actor,
            target,
            intent_damage: V::Number::zero(),
            skipped: Vec::new(),
            won: false,
        }
    }

    /// A board for one move, made by a monster.
    pub(super) fn for_move(
        combat: &'a mut Combat<V>,
        shuffle: &'a mut MegaRng,
        actor: CombatId,
        intent_damage: V::Number,
    ) -> Self {
        Board {
            combat,
            shuffle,
            actor,
            target: None,
            intent_damage,
            skipped: Vec::new(),
            won: false,
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

    /// `PowerCmd.Apply<P>(at, amount, Owner.Creature, this)`.
    fn land(&mut self, at: CombatId, power: V::Power, amount: PowerAmount<V::Number>) {
        // The command returns on a zero amount, and on a creature that cannot
        // hold powers. A monster the fight has taken out is one, because its
        // combat state is gone with it.
        if amount == PowerAmount::Counted(V::Number::zero()) {
            return;
        }
        let Some(at) = self.at(at) else { return };

        // `PowerCmd.Apply` sets `SkipNextDurationTick` on a debuff that lands
        // on the player, so the next tick passes it over. An endless power has
        // no tick to skip, and nothing else keeps a flag it never reads.
        let counts_down = amount != PowerAmount::Endless
            && [
                <V::CombatSide as SideKinds>::PLAYER,
                <V::CombatSide as SideKinds>::ENEMY,
            ]
            .into_iter()
            .any(|side| V::ticks_down(power, side));
        if self.combat.creatures[at].side == <V::CombatSide as SideKinds>::PLAYER && counts_down {
            let id = self.combat.creatures[at].id;
            if !self.skipped.contains(&(id, power)) {
                self.skipped.push((id, power));
            }
        }

        // `FindExistingInstanceForStacking`: a counter stacks into the one
        // already there. An endless instance is one instance however many
        // times it lands.
        let held = self.combat.creatures[at]
            .powers
            .iter_mut()
            .find(|held| held.power == power);
        match (held, amount) {
            (Some(held), PowerAmount::Counted(more)) => match held.amount {
                PowerAmount::Counted(had) => held.amount = PowerAmount::Counted(had + more),
                PowerAmount::Endless => unimplemented!("a count landing on an endless power"),
            },
            (Some(_), PowerAmount::Endless) => {}
            (None, amount) => self.combat.creatures[at]
                .powers
                .push(Stack { power, amount }),
        }
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
        self.won = !self
            .combat
            .creatures
            .iter()
            .any(|creature| creature.side == <V::CombatSide as SideKinds>::ENEMY);
    }
}

/// One hit after every power on either side has multiplied it, which is
/// `Hook.ModifyDamage`.
///
/// `dealer` is the creature swinging and `at` is where the one it hits sits.
/// Nothing adds to a hit yet, and nothing caps one.
///
/// The multipliers this build fields are halves and quarters. A binary float
/// holds those exactly, so the truncation at the end lands where the game's
/// own decimal arithmetic lands.
pub(crate) fn modified<V: Rules>(
    creatures: &[Creature<V>],
    dealer: CombatId,
    at: usize,
    damage: V::Number,
) -> V::Number {
    let dealer = creatures.iter().position(|creature| creature.id == dealer);
    let mut amount: f64 = num_traits::cast(damage).expect("a hit is a number a float holds");
    for (holder, holding) in [(Some(at), Holding::Target), (dealer, Holding::Dealer)] {
        let Some(holder) = holder else { continue };
        for held in &creatures[holder].powers {
            amount *= V::damage_multiplier(held.power, holding);
        }
    }
    num_traits::cast(amount.max(0.0).trunc()).expect("a modified hit is a game number")
}

/// The fight as it is reported, with every intent run through the modifiers.
///
/// `Attack::damage` is the number that will land rather than the one the move
/// prints. The game works it out on every read, because a power applied after
/// the intent was rolled changes it. So a fight keeps the move's own number
/// and this is where the fight has its say.
pub(crate) fn reported<V: Rules>(combat: &Combat<V>) -> Combat<V> {
    let mut out = combat.clone();
    let Some(target) = out
        .creatures
        .iter()
        .position(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER)
    else {
        return out;
    };

    for at in 0..out.creatures.len() {
        let dealer = out.creatures[at].id;
        let Some(next) = out.creatures[at].next_move.clone() else {
            continue;
        };
        let intents = next
            .intents
            .into_iter()
            .map(|mut intent| {
                if let Some(attack) = &mut intent.attack {
                    attack.damage = modified::<V>(&combat.creatures, dealer, target, attack.damage);
                }
                intent
            })
            .collect();
        out.creatures[at].next_move = Some(sts2_core::NextMove {
            id: next.id,
            intents,
        });
    }
    out
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
        let damage = modified::<V>(&self.combat.creatures, self.actor, at, damage);
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

    fn apply(&mut self, at: CombatId, power: V::Power, amount: PowerAmount<V::Number>) {
        self.land(at, power, amount);
    }

    fn apply_to_opponents(&mut self, power: V::Power, amount: PowerAmount<V::Number>) {
        // The list is taken before the first lands, as an attack's is.
        for id in self.opponents() {
            self.land(id, power, amount);
        }
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

    fn draw(&mut self, count: V::Number) {
        // A negative count draws nothing: the game's own loop runs no times.
        let count = num_traits::NumCast::from(count).unwrap_or(0usize);
        draw(self.combat, self.shuffle, count);
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
