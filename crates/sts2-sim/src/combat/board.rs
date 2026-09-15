//! The fight, as behaviour acts on it.
//!
//! One board for both kinds of behaviour. The player plays a card and a
//! monster makes a move. Past that the two issue the game's same commands
//! against the same fight.
//!
//! Everything here is one of those commands with nothing listening. This
//! build's fights hold no power, relic or card that hooks any of them yet. A
//! command that needs one stops the run where it stands.

use crate::rng::MegaRng;
use num_traits::Zero;
use sts2_core::rng::GameRng;
use sts2_core::{
    Amount, CardIdent, CardNumberModifier, CardTable, CardVars, CombatId, Creature, Effect,
    EffectResolution, Fight, Holding, Modify, PowerAmount, PowerTable, Rules, SideKinds, Stack,
};

use super::live::{CardInstance, Live};

/// The fight an effect is resolved against.
pub(super) struct Board<'a, V: Rules> {
    live: &'a mut Live<V>,
    /// The shuffle stream, because drawing a card can turn the discard pile
    /// back into a draw pile.
    shuffle: &'a mut MegaRng,
    /// The stream monsters use when they roll their hit points.
    niche: &'a mut MegaRng,
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
    /// How many times it advertises hitting, which is `WithHitCount`.
    intent_repeats: V::Count,
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

/// An effect after a simulator fight's listeners have modified it.
///
/// One effect, because the game resolves one. A move that hits every opponent,
/// and a card that hits the same one twice, are both a loop of resolve and
/// apply. Each hit reaches the listeners the hit before it left behind, so a
/// resolution standing for several of them freezes hooks the first landing
/// changes.
pub struct ResolvedEffect<V: Rules> {
    effect: Resolved<V>,
}

enum Resolved<V: Rules> {
    Attack {
        target: CombatId,
        damage: V::Number,
    },
    GainBlock {
        recipient: CombatId,
        amount: V::Number,
    },
}

impl<V: Rules> EffectResolution<V> for ResolvedEffect<V> {
    fn amount(&self) -> V::Number {
        match self.effect {
            Resolved::Attack { damage, .. } => damage,
            Resolved::GainBlock { amount, .. } => amount,
        }
    }
}

impl<'a, V: Rules> Board<'a, V> {
    /// A board for one card, played from the player.
    pub(super) fn for_card(
        live: &'a mut Live<V>,
        shuffle: &'a mut MegaRng,
        niche: &'a mut MegaRng,
        ascension: V::AscensionLevel,
        target: Option<CombatId>,
        playing: Option<&'a CardInstance<V>>,
    ) -> Self {
        let actor = live.player().id;
        Board {
            live,
            shuffle,
            niche,
            actor,
            target,
            playing,
            intent_damage: V::Number::zero(),
            // Nothing a card does reads an intent, and a card is not one.
            intent_repeats: num_traits::zero(),
            ascension,
            skipped: Vec::new(),
            won: false,
        }
    }

    /// A board for one move, made by a monster.
    pub(super) fn for_move(
        live: &'a mut Live<V>,
        shuffle: &'a mut MegaRng,
        niche: &'a mut MegaRng,
        ascension: V::AscensionLevel,
        actor: CombatId,
        intent_damage: V::Number,
        intent_repeats: V::Count,
    ) -> Self {
        Board {
            live,
            shuffle,
            niche,
            actor,
            target: None,
            playing: None,
            intent_damage,
            intent_repeats,
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

    /// Resolve a death, then end the fight if no primary enemy remains.
    fn kill(&mut self, at: usize) {
        if self.die(at) {
            return;
        }
        let primary_enemy_remains = self.live.creatures.iter().any(|creature| {
            creature.side == <V::CombatSide as SideKinds>::ENEMY
                && !creature.powers.iter().any(|held| {
                    <V as sts2_core::Hooks>::decide_hook(
                        held.listener(creature.side),
                        sts2_core::DecisionHook::ShouldCountForCombatEnd,
                    )
                    .is_decision_and(|counts| !counts)
                })
        });
        if primary_enemy_remains {
            return;
        }

        // `EndCombat`: everything left on the enemy side is secondary. It
        // dies through the same hooks, but cannot keep the fight open.
        for at in (0..self.live.creatures.len()).rev() {
            if self.live.creatures[at].side == <V::CombatSide as SideKinds>::ENEMY
                && self.live.creatures[at].alive
            {
                self.live.creatures[at].hp = V::Health::zero();
                self.die(at);
            }
        }
        self.won = true;
    }

    /// `KillWithoutCheckingWinCondition`, including a power's revival hook.
    ///
    /// Returns whether a hook keeps the dead creature in the fight.
    fn die(&mut self, at: usize) -> bool {
        self.live.creatures[at].alive = false;
        if self.live.creatures[at].side != <V::CombatSide as SideKinds>::ENEMY {
            unimplemented!("a player dying in a fight");
        }
        let owner = self.live.creatures[at].side;
        let powers = self.live.creatures[at].powers.clone();
        let kept = powers.iter().any(|held| {
            <V as sts2_core::Hooks>::decide_hook(
                held.listener(owner),
                sts2_core::DecisionHook::ShouldRemoveCreatureAfterDeath,
            )
            .is_decision_and(|remove| !remove)
        });
        if kept {
            let revival = powers
                .iter()
                .flat_map(|held| {
                    <V as sts2_core::Hooks>::event_hook(
                        held.listener(owner),
                        sts2_core::EventHook::AfterDeath,
                    )
                })
                .find_map(|effect| match effect {
                    sts2_core::HookEffect::InstallMove(next) => Some(next),
                    _ => None,
                })
                .expect("a retained dead monster installs a move");
            let id = self.live.creatures[at].id;
            let resume = self.live.creatures[at].next_move.replace(revival);
            let held = self
                .live
                .machines
                .iter_mut()
                .find(|held| held.creature == id)
                .expect("a reviving monster has a move machine");
            held.resume = resume;
            return true;
        }
        // A dead monster is taken out of the fight, so nothing that reads it
        // afterwards can see it. That is why the observation stops reporting
        // it rather than reporting it dead.
        self.live.creatures.remove(at);
        false
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
pub(super) fn hooked<V: Rules>(
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
/// `dealer` is the creature swinging and `target` the one it hits. A creature
/// that has left the fight has no listeners left to ask, so it modifies
/// nothing and the hit still works out. Nothing caps a hit.
pub(crate) fn modified<V: Rules>(
    creatures: &[Creature<V>],
    dealer: CombatId,
    target: CombatId,
    damage: V::Number,
) -> V::Number {
    let at = |id: CombatId| creatures.iter().position(|creature| creature.id == id);
    let sides = [(at(target), Holding::Target), (at(dealer), Holding::Dealer)];
    hooked::<V>(
        damage,
        sides.into_iter().flat_map(|(holder, holding)| {
            holder
                .into_iter()
                .flat_map(move |holder| {
                    let side = creatures[holder].side;
                    creatures[holder]
                        .powers
                        .iter()
                        .map(move |held| (held, side))
                })
                .map(move |(held, side)| {
                    <V as sts2_core::Hooks>::modify_hook(
                        held.listener(side),
                        sts2_core::ModifyHook::Damage { holding },
                    )
                })
        }),
    )
}

/// One block gain after every power on the creature gaining it has had its
/// say, which is `Hook.ModifyBlock`.
///
/// The same hook whether the block is previewed or gained, as damage's is. A
/// recipient that has left the fight modifies nothing, as a hit's creatures
/// do.
pub(super) fn blocked<V: Rules>(
    creatures: &[Creature<V>],
    recipient: CombatId,
    amount: V::Number,
) -> V::Number {
    let held = creatures
        .iter()
        .find(|creature| creature.id == recipient)
        .into_iter()
        .flat_map(|creature| {
            creature.powers.iter().map(|held| {
                <V as sts2_core::Hooks>::modify_hook(
                    held.listener(creature.side),
                    sts2_core::ModifyHook::Block,
                )
            })
        });
    hooked::<V>(amount, held)
}

/// One effect after the fight's listeners have had their say.
///
/// The one path a number takes. It is worked out from the card that prints
/// it, then run through the hooks the effect names. No step in between is one
/// a caller can reach. A preview and the play it previews call this with the
/// same effect, so the number a card shows is the number it deals.
pub(crate) fn resolve<V: Rules>(
    creatures: &[Creature<V>],
    playing: Option<&CardInstance<V>>,
    effect: Effect<V>,
) -> ResolvedEffect<V> {
    let effect = match effect {
        Effect::Attack {
            dealer,
            target,
            damage,
        } => Resolved::Attack {
            target,
            damage: modified::<V>(
                creatures,
                dealer,
                target,
                carried::<V>(playing, creatures, damage),
            ),
        },
        Effect::GainBlock { recipient, amount } => Resolved::GainBlock {
            recipient,
            amount: blocked::<V>(
                creatures,
                recipient,
                carried::<V>(playing, creatures, amount),
            ),
        },
    };
    ResolvedEffect { effect }
}

/// The number an effect carries, before any hook of its own runs.
fn carried<V: Rules>(
    playing: Option<&CardInstance<V>>,
    creatures: &[Creature<V>],
    amount: Amount<V>,
) -> V::Number {
    match amount {
        Amount::Number(number) => number,
        Amount::Var(key) => var::<V>(
            playing
                .unwrap_or_else(|| unimplemented!("a card's variable with no card being played")),
            key,
            creatures,
        ),
    }
}

/// What one of this copy's variables is worth right now.
///
/// A `CalculatedVar` counts something in the live fight. It is worth what it
/// counted, rather than the zero it was built with.
pub(crate) fn var<V: Rules>(
    held: &CardInstance<V>,
    key: V::CardVar,
    creatures: &[Creature<V>],
) -> V::Number {
    let number = number::<V>(held, key);
    match number.calculated {
        false => number.base,
        true => V::calculated_card_var(held.ident.card, key, &held.vars, powers::<V>(creatures)),
    }
}

/// The entry this copy of the card holds for `key`.
fn number<V: Rules>(
    held: &CardInstance<V>,
    key: V::CardVar,
) -> &sts2_core::CardNumber<V::CardVar, V::Number> {
    // A key the card does not print is a mistake in the port, the same way a
    // card asking for a number no card prints is.
    held.vars
        .iter()
        .find(|var| var.id == key)
        .unwrap_or_else(|| unimplemented!("the {} this card does not print", key.name()))
}

/// The player's own powers, which is what a card reads about the fight.
pub(super) fn powers<V: Rules>(creatures: &[Creature<V>]) -> &[Stack<V>] {
    creatures
        .iter()
        .find(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER)
        .map(|creature| creature.powers.as_slice())
        .unwrap_or_default()
}

/// The effect one of a card's variables becomes, for a card played at
/// `target`.
///
/// `DynamicVar`'s subclass decides it: what the number runs through is the
/// class the card filed it under, not the key it is filed by. A number that
/// runs through nothing has no effect to build, and is worth what it reads.
pub(crate) fn effect_of<V: Rules>(
    modifier: CardNumberModifier,
    key: V::CardVar,
    dealer: CombatId,
    target: Option<CombatId>,
) -> Option<Effect<V>> {
    match modifier {
        CardNumberModifier::Damage => target.map(|target| Effect::Attack {
            dealer,
            target,
            damage: Amount::Var(key),
        }),
        CardNumberModifier::Block => Some(Effect::GainBlock {
            recipient: dealer,
            amount: Amount::Var(key),
        }),
        CardNumberModifier::OstyDamage => unimplemented!("a card previewing the Osty's damage"),
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
    type ResolvedEffect = ResolvedEffect<V>;

    fn actor(&self) -> CombatId {
        self.actor
    }

    fn target(&self) -> CombatId {
        // `resolve` refuses a play whose card names a creature and was pointed
        // at none, so a card that reads this was pointed at one.
        self.target
            .expect("a card that names a target is played at one")
    }

    fn opponents(&self) -> Vec<CombatId> {
        self.opponents()
    }

    fn resolve(&self, effect: Effect<V>) -> Self::ResolvedEffect {
        resolve::<V>(&self.live.creatures, self.playing, effect)
    }

    fn apply(&mut self, effect: Self::ResolvedEffect) {
        match effect.effect {
            Resolved::Attack { target, damage } => {
                // `CreatureCmd.Damage`: block first, then hit points, then the
                // death the hit caused. Resolution already ran hooks. A target
                // that has left the fight takes nothing, which is what the
                // command does against a creature with no combat state.
                let Some(at) = self.at(target) else { return };
                if !self.live.creatures[at].alive {
                    return;
                }
                let zero = V::Number::zero();
                let blocked = self.live.creatures[at].block.min(damage).max(zero);
                self.live.creatures[at].block = self.live.creatures[at].block - blocked;
                let unblocked = (damage - blocked).max(zero);

                let hp = self.live.creatures[at].hp;
                let health_zero = V::Health::zero();
                let killed = hp > health_zero && unblocked >= V::Number::from(hp);
                self.live.creatures[at].hp = (hp - V::Health::from(unblocked)).max(health_zero);
                if killed {
                    self.kill(at);
                }
            }
            Resolved::GainBlock { recipient, amount } => {
                let Some(at) = self.at(recipient) else { return };
                if amount > V::Number::zero() {
                    self.live.creatures[at].block = self.live.creatures[at].block + amount;
                }
            }
        }
    }

    fn gain_stars(&mut self, amount: V::Number) {
        // The pool is a count, so a card printing a negative one is spending
        // rather than gaining. No card on this build does.
        let amount = num_traits::NumCast::from(amount)
            .unwrap_or_else(|| unimplemented!("a card spending stars as it plays"));
        self.live.stars = self.live.stars + amount;
    }

    fn apply_power(&mut self, at: CombatId, power: V::Power, amount: PowerAmount<V::Number>) {
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

    fn intent_repeats(&self) -> V::Count {
        self.intent_repeats
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
        let held = self
            .playing
            .unwrap_or_else(|| unimplemented!("a number printed by no card being played"));
        if number::<V>(held, key).modifier.is_some() {
            // The hooks it runs through name creatures to run against, and a
            // bare read has none. `Fight::resolve` is where both arrive.
            unimplemented!(
                "the {} read without the creatures it runs against",
                key.name()
            );
        }
        var::<V>(held, key, &self.live.creatures)
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

    fn summon(&mut self, monster: V::Monster, position: usize) -> CombatId {
        super::monster::summon(self.live, monster, position, self.niche)
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
