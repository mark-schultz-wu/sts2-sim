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
    CardIdent, CardTable, CardVars, CombatId, Creature, Fight, Holding, Modify, PowerAmount,
    PowerTable, Rules, SideKinds, Stack,
};

use super::live::{CardInstance, Live};

/// The fight an effect is resolved against.
pub(super) struct Board<'a, V: Rules> {
    live: &'a mut Live<V>,
    /// The shuffle stream, because drawing a card can turn the discard pile
    /// back into a draw pile.
    shuffle: &'a mut MegaRng,
    /// Whose effect this is. The player for a card, the monster for a move.
    actor: CombatId,
    /// The creature a card was pointed at.
    target: Option<CombatId>,
    /// The card being played, which is in no pile while its effect runs.
    ///
    /// `AddDuringManualCardPlay` holds it in the play pile. Nothing here for a
    /// monster's move, which prints no numbers of its own.
    playing: Option<&'a CardInstance<V>>,
    /// What the acting monster's move advertises it will hit for.
    intent_damage: V::Number,
    /// What the run is being played at, which a move's own constants scale by.
    ascension: V::AscensionLevel,
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
        live: &'a mut Live<V>,
        shuffle: &'a mut MegaRng,
        ascension: V::AscensionLevel,
        target: Option<CombatId>,
        playing: Option<&'a CardInstance<V>>,
    ) -> Self {
        let actor = live.player().id;
        Board {
            live,
            shuffle,
            actor,
            target,
            playing,
            intent_damage: V::Number::zero(),
            ascension,
            skipped: Vec::new(),
            won: false,
        }
    }

    /// A board for one move, made by a monster.
    pub(super) fn for_move(
        live: &'a mut Live<V>,
        shuffle: &'a mut MegaRng,
        ascension: V::AscensionLevel,
        actor: CombatId,
        intent_damage: V::Number,
    ) -> Self {
        Board {
            live,
            shuffle,
            actor,
            target: None,
            playing: None,
            intent_damage,
            ascension,
            skipped: Vec::new(),
            won: false,
        }
    }

    /// Where a creature sits, or nothing for one no longer in the fight.
    fn at(&self, at: CombatId) -> Option<usize> {
        self.live
            .creatures
            .iter()
            .position(|creature| creature.id == at)
    }

    /// Which side the acting creature fights on.
    fn side(&self) -> V::CombatSide {
        self.at(self.actor)
            .map(|at| self.live.creatures[at].side)
            .unwrap_or(<V::CombatSide as SideKinds>::PLAYER)
    }

    /// Everything on the other side, which is `GetOpponentsOf`.
    fn opponents(&self) -> Vec<CombatId> {
        let side = self.side();
        self.live
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

        // `PowerCmd.Apply` sets `SkipNextDurationTick` on every debuff that
        // lands on a player-side creature, so the next tick passes it over.
        // The power's own kind and the side it landed on, and nothing about
        // what the power does when a turn ends.
        let debuff = power
            .debuff()
            .unwrap_or_else(|| unimplemented!("whether {power:?} is a debuff"));
        if self.live.creatures[at].side == <V::CombatSide as SideKinds>::PLAYER && debuff {
            let id = self.live.creatures[at].id;
            if !self.skipped.contains(&(id, power)) {
                self.skipped.push((id, power));
            }
        }

        // `FindExistingInstanceForStacking`: a counter stacks into the one
        // already there. An endless instance is one instance however many
        // times it lands.
        let held = self.live.creatures[at]
            .powers
            .iter_mut()
            .find(|held| held.power == power);
        match (held, amount) {
            (Some(held), PowerAmount::Counted(more)) => match held.amount {
                PowerAmount::Counted(had) => held.amount = PowerAmount::Counted(had + more),
                PowerAmount::Endless => unimplemented!("a count landing on an endless power"),
            },
            (Some(_), PowerAmount::Endless) => {}
            (None, amount) => self.live.creatures[at].powers.push(Stack { power, amount }),
        }
    }

    /// `KillWithoutCheckingWinCondition`, for a creature the damage killed.
    fn kill(&mut self, at: usize) {
        self.live.creatures[at].alive = false;
        if self.live.creatures[at].side != <V::CombatSide as SideKinds>::ENEMY {
            unimplemented!("a player dying in a fight");
        }
        // A dead monster is taken out of the fight, so nothing that reads it
        // afterwards can see it. That is why the observation stops reporting
        // it rather than reporting it dead.
        self.live.creatures.remove(at);
        self.won = !self
            .live
            .creatures
            .iter()
            .any(|creature| creature.side == <V::CombatSide as SideKinds>::ENEMY);
    }
}

/// `Hook.ModifyDamage` and `Hook.ModifyBlock` in the shape they share.
///
/// Every addition lands, then every share, then the result is floored at zero.
///
/// The game asks every listener in the fight and lets each read who is
/// involved. The callers below ask only the creatures the hook is about, which
/// is narrower. Nothing ported so far modifies a number it is not standing on
/// one side of.
fn hooked<V: Rules>(
    amount: V::Number,
    hooks: impl Iterator<Item = Modify<V::Number>>,
) -> V::Number {
    let (mut added, mut share) = (amount, 1.0f64);
    for hook in hooks {
        match hook {
            Modify::Nothing => {}
            Modify::Added(more) => added = added + more,
            Modify::Scaled(by) => share *= by,
        }
    }

    // The multipliers this build fields are halves and quarters. A binary
    // float holds those exactly, so the truncation lands where the game's own
    // decimal arithmetic lands.
    let amount: f64 = num_traits::cast(added).expect("a number a fight works out fits a float");
    num_traits::cast((amount * share).max(0.0).trunc()).expect("a modified number is a game number")
}

/// One hit after every power on either side has had its say, which is
/// `Hook.ModifyDamage`.
///
/// `dealer` is the creature swinging and `at` is where the one it hits sits.
/// Nothing caps a hit.
pub(crate) fn modified<V: Rules>(
    creatures: &[Creature<V>],
    dealer: CombatId,
    at: usize,
    damage: V::Number,
) -> V::Number {
    let dealer = creatures.iter().position(|creature| creature.id == dealer);
    let sides = [(Some(at), Holding::Target), (dealer, Holding::Dealer)];
    hooked::<V>(
        damage,
        sides.into_iter().flat_map(|(holder, holding)| {
            holder
                .into_iter()
                .flat_map(move |holder| &creatures[holder].powers)
                .map(move |held| V::damage_hook(held.power, holding, held.amount))
        }),
    )
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
        if !self.live.creatures[at].alive {
            return;
        }
        let damage = modified::<V>(&self.live.creatures, self.actor, at, damage);
        let zero = V::Number::zero();
        let blocked = self.live.creatures[at].block.min(damage).max(zero);
        self.live.creatures[at].block = self.live.creatures[at].block - blocked;
        let unblocked = (damage - blocked).max(zero);

        let hp = self.live.creatures[at].hp;
        let killed = hp > zero && unblocked >= hp;
        self.live.creatures[at].hp = (hp - unblocked).max(zero);
        if killed {
            self.kill(at);
        }
    }

    fn gain_block(&mut self, amount: V::Number) {
        // `CreatureCmd.GainBlock` runs the amount through `Hook.ModifyBlock`,
        // clamps at zero, and adds nothing at all when nothing is left.
        let actor = self.actor;
        let Some(at) = self.at(actor) else { return };
        let amount = hooked::<V>(
            amount,
            self.live.creatures[at]
                .powers
                .iter()
                .map(|held| V::block_hook(held.power, held.amount)),
        );
        if amount > V::Number::zero() {
            self.live.creatures[at].block = self.live.creatures[at].block + amount;
        }
    }

    fn gain_stars(&mut self, amount: V::Number) {
        // The pool is a count, so a card printing a negative one is spending
        // rather than gaining. No card on this build does.
        let amount = num_traits::NumCast::from(amount)
            .unwrap_or_else(|| unimplemented!("a card spending stars as it plays"));
        self.live.stars = self.live.stars + amount;
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
        draw(self.live, self.shuffle, count);
    }

    fn ascension(&self) -> V::AscensionLevel {
        self.ascension
    }

    fn var(&self, key: V::CardVar) -> V::Number {
        // A key the card does not print is a mistake in the port, the same way
        // a card asking for a number no card prints is.
        self.playing
            .unwrap_or_else(|| unimplemented!("a number printed by no card being played"))
            .vars
            .iter()
            .find(|var| var.id == key)
            .unwrap_or_else(|| unimplemented!("the {} this card does not print", key.name()))
            .base
    }

    fn forge(&mut self, card: V::Card, key: V::CardVar, amount: V::Number) {
        // `ForgeCmd.Forge` creates one when the fight holds none it can reach.
        // The exhaust pile is not one it can reach, and is forged all the same.
        let held = [&self.live.hand, &self.live.draw, &self.live.discard]
            .into_iter()
            .flatten()
            .any(|held| held.ident.card == card);
        if !held {
            self.live.hand.push(CardInstance::new(CardIdent::new(card)));
        }
        for pile in [
            &mut self.live.hand,
            &mut self.live.draw,
            &mut self.live.discard,
            &mut self.live.exhaust,
        ] {
            for held in pile.iter_mut().filter(|held| held.ident.card == card) {
                let var = held
                    .vars
                    .iter_mut()
                    .find(|var| var.id == key)
                    .unwrap_or_else(|| unimplemented!("the {} a forge raises", key.name()));
                var.base = var.base + amount;
            }
        }
    }

    fn apply_to_actor(&mut self, power: V::Power, amount: PowerAmount<V::Number>) {
        self.land(self.actor, power, amount);
    }

    fn give_cards(&mut self, card: V::Card, count: usize) {
        // Every opponent takes its own copies, into its own discard pile. A
        // fight here has one set of piles, so a second player would need the
        // observation to grow one.
        if self.opponents().len() > 1 {
            unimplemented!("cards given to more than one player");
        }
        for _ in 0..count {
            self.live
                .discard
                .push(CardInstance::new(CardIdent::new(card)));
        }
    }
}

/// `CardPileCmd.Draw`, which shuffles the discard pile back in when the draw
/// pile runs dry.
///
/// A fight with every card in hand draws short rather than manufacture one.
pub(super) fn draw<V: Rules>(live: &mut Live<V>, shuffle: &mut MegaRng, count: usize) {
    for _ in 0..count {
        if live.draw.is_empty() {
            if live.discard.is_empty() {
                return;
            }
            reshuffle(live, shuffle);
        }
        live.hand.push(live.draw.remove(0));
    }
}

/// `CardPileCmd.Shuffle`: the discard pile and whatever is left of the draw
/// pile, shuffled together into a new draw pile.
///
/// A stable shuffle, unlike the one that deals a fight its opening hand. The
/// two are different sequences over the same stream, so which one this is
/// decides every card drawn after it.
fn reshuffle<V: Rules>(live: &mut Live<V>, shuffle: &mut MegaRng) {
    let mut shuffled = std::mem::take(&mut live.discard);
    shuffled.append(&mut live.draw);
    shuffle.stable_shuffle_by_key(&mut shuffled, |held| {
        (held.ident.card.order(), held.ident.upgrade)
    });
    live.draw = shuffled;
}
