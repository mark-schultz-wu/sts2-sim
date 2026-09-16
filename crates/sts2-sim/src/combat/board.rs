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
use num_traits::{Saturating as _, Zero};
use sts2_core::rng::GameRng;
use sts2_core::{
    Amount, CardAt, CardIdent, CardNumberModifier, CardTable, CardVars, CombatId, Creature, Effect,
    EffectResolution, Fight, Holding, Modify, Pile, PowerAmount, PowerTable, Rules, RunStreamKinds,
    Scaling, SideKinds, Stack,
};

use super::live::{CardInstance, Counter, Live, counted};
use crate::run::HeldRelic;
use crate::stream::CombatRngs;

/// The fight an effect is resolved against.
pub(super) struct Board<'a, V: Rules> {
    live: &'a mut Live<V>,
    /// The run streams an effect can draw from while it resolves.
    rngs: CombatRngs<'a>,
    /// The player's relic belt, for hooks raised while an effect resolves.
    relics: &'a mut [HeldRelic<V>],
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
        rngs: CombatRngs<'a>,
        relics: &'a mut [HeldRelic<V>],
        ascension: V::AscensionLevel,
        target: Option<CombatId>,
        playing: Option<&'a CardInstance<V>>,
    ) -> Self {
        let actor = live.player().id;
        Board {
            live,
            rngs,
            relics,
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
        rngs: CombatRngs<'a>,
        relics: &'a mut [HeldRelic<V>],
        ascension: V::AscensionLevel,
        acting: (CombatId, V::Number, V::Count),
    ) -> Self {
        let (actor, intent_damage, intent_repeats) = acting;
        Board {
            live,
            rngs,
            relics,
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
        // `BeforeApplied`, which runs before the power attaches. What it asks
        // for lands first, so a creature carries those powers ahead of this
        // one.
        // Not a `Stack`, because nothing carries it yet. The side is the
        // creature it is landing on, which is the owner it is about to have.
        let listener = sts2_core::Listener::Power {
            power,
            amount,
            owner: self.live.creatures[at].side,
            // Nothing is counted on a power that is not on a creature yet.
            counted: V::Number::zero(),
        };
        let asked = <V as sts2_core::Hooks>::event_hook(
            listener,
            sts2_core::EventHook::BeforeApplied { landing: amount },
        );
        for effect in asked {
            match effect {
                sts2_core::HookEffect::ApplyToOwner(power, amount) => {
                    self.land(self.live.creatures[at].id, power, amount);
                }
                other => unimplemented!("a power asking for {other:?} as it lands"),
            }
        }
        let Some(at) = self.at(self.live.creatures[at].id) else {
            return;
        };

        // The game reads the power's type behind `&&`, so it asks only about
        // one landing on the player. A power that works its own type out is
        // never asked on the way onto a monster.
        let on_player = self.live.creatures[at].side == <V::CombatSide as SideKinds>::PLAYER;
        let debuff = || {
            power
                .debuff()
                .unwrap_or_else(|| unimplemented!("whether {power:?} is a debuff"))
        };
        if on_player && debuff() {
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
        // A counted power whose stack reaches zero is gone. Temporary
        // Strength returns exactly what it took, so leaving a zero instance
        // here would make a creature observably carry Strength.
        self.live.creatures[at]
            .powers
            .retain(|held| held.amount != PowerAmount::Counted(V::Number::zero()));
    }

    /// Resolve a death, then end the fight if no primary enemy remains.
    fn kill(&mut self, at: usize) {
        if self.die(at) {
            return;
        }
        let counters = &self.live.counted;
        let primary_enemy_remains = self.live.creatures.iter().any(|creature| {
            creature.side == <V::CombatSide as SideKinds>::ENEMY
                && !creature.powers.iter().any(|held| {
                    <V as sts2_core::Hooks>::decide_hook(
                        held.listener(
                            creature.side,
                            counted::<V>(counters, creature.id, held.power),
                        ),
                        sts2_core::DecisionHook::ShouldCountForCombatEnd,
                    )
                    .is_decision_and(|counts| !counts)
                })
        });
        if primary_enemy_remains {
            self.after_enemy_death();
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
        let id = self.live.creatures[at].id;
        let powers = self.live.creatures[at].powers.clone();
        let counters = self.live.counted.clone();
        let after_death: Vec<_> = powers
            .iter()
            .flat_map(|held| {
                <V as sts2_core::Hooks>::event_hook(
                    held.listener(owner, counted::<V>(&counters, id, held.power)),
                    sts2_core::EventHook::AfterDeath,
                )
            })
            .collect();
        let kept = powers.iter().any(|held| {
            <V as sts2_core::Hooks>::decide_hook(
                held.listener(owner, counted::<V>(&counters, id, held.power)),
                sts2_core::DecisionHook::ShouldRemoveCreatureAfterDeath,
            )
            .is_decision_and(|remove| !remove)
        });
        if kept {
            let revival = after_death
                .into_iter()
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
        let id = self.live.creatures[at].id;
        let position = self.live.creatures[..at]
            .iter()
            .filter(|creature| creature.side == owner)
            .count();
        // Its move graph is finished, but the creature remains addressable
        // until its death commands finish. Monsters those commands add must
        // receive ids after its own rather than reuse the one being removed.
        self.live.machines.retain(|held| held.creature != id);
        for effect in after_death {
            match effect {
                sts2_core::HookEffect::SummonAfterOwnerDeath {
                    monster,
                    count,
                    opening,
                } => {
                    let count = num_traits::cast(count)
                        .unwrap_or_else(|| unimplemented!("a death summoning {count:?} monsters"));
                    for offset in 0..count {
                        let id = super::monster::summon(
                            self.live,
                            monster,
                            position + offset,
                            self.rngs.niche,
                        );
                        let summoned = self
                            .live
                            .creatures
                            .iter_mut()
                            .find(|creature| creature.id == id)
                            .expect("a summoned monster is in the fight");
                        summoned.next_move = Some(opening.clone());
                    }
                }
                other => unimplemented!("a dead power asking for {other:?}"),
            }
        }
        // Nothing that reads the fight after the death commands can see the
        // dead monster. The observation therefore omits it rather than
        // reporting it dead, and the numbers its powers counted go with it.
        self.live.creatures.retain(|creature| creature.id != id);
        self.live.counted.retain(|held| held.creature != id);
        false
    }

    /// Notify the player's belt after a death that did not end combat.
    fn after_enemy_death(&mut self) {
        let asked = super::relic::notify::<V>(self.relics, sts2_core::EventHook::AfterDeath);
        super::hook::apply::<V, _>(self, asked);
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
    let mut ceiling: Option<V::Number> = None;
    for hook in hooks {
        match hook {
            Modify::Nothing => {}
            Modify::Added(more) => added = added + more,
            Modify::Scaled(by) => share *= by,
            // Last, because a ceiling is a listener handing back a number of
            // its own. Nothing this build fields both caps and scales, so the
            // order between them is not one the game has had to settle.
            Modify::AtMost(most) => {
                ceiling = Some(ceiling.map_or(most, |held: V::Number| held.min(most)));
            }
        }
    }

    // The multipliers this build fields are halves and quarters. A binary
    // float holds those exactly, so the truncation lands where the game's own
    // decimal arithmetic lands.
    let amount: f64 = num_traits::cast(added).expect("a number a fight works out fits a float");
    let amount: V::Number = num_traits::cast((amount * share).max(0.0).trunc())
        .expect("a modified number is a game number");
    match ceiling {
        Some(most) => amount.min(most),
        None => amount,
    }
}

/// What a hit takes off the hit points, which is `Hook.ModifyHpLost`.
///
/// The block has already taken what it can. The game asks every listener in
/// the fight, and each reads whether the creature hit is its own. This asks
/// the powers on that creature, which is the same set answering.
pub(super) fn hp_lost<V: Rules>(
    creatures: &[Creature<V>],
    counters: &[Counter<V>],
    target: CombatId,
    unblocked: V::Number,
) -> V::Number {
    let held = creatures
        .iter()
        .find(|creature| creature.id == target)
        .into_iter()
        .flat_map(|creature| {
            creature.powers.iter().map(|held| {
                <V as sts2_core::Hooks>::modify_hook(
                    held.listener(
                        creature.side,
                        counted::<V>(counters, creature.id, held.power),
                    ),
                    sts2_core::ModifyHook::HpLost,
                )
            })
        });
    hooked::<V>(unblocked, held)
}

/// One hit after every power on either side has had its say, which is
/// `Hook.ModifyDamage`.
///
/// `dealer` is the creature swinging and `target` the one it hits. A creature
/// that has left the fight has no listeners to ask. Nothing caps a hit.
///
/// The target is absent for a hit previewed against nobody.
/// `Hook.ModifyDamage` takes a null target and runs anyway. Only the powers
/// reading the target sit that one out.
pub(crate) fn modified<V: Rules>(
    creatures: &[Creature<V>],
    counters: &[Counter<V>],
    dealer: CombatId,
    target: Option<CombatId>,
    damage: V::Number,
) -> V::Number {
    let at = |id: CombatId| creatures.iter().position(|creature| creature.id == id);
    let sides = [
        (target.and_then(at), Holding::Target),
        (at(dealer), Holding::Dealer),
    ];
    hooked::<V>(
        damage,
        sides.into_iter().flat_map(|(holder, holding)| {
            holder
                .into_iter()
                .flat_map(move |holder| {
                    let creature = &creatures[holder];
                    creature
                        .powers
                        .iter()
                        .map(move |held| (held, creature.side, creature.id))
                })
                .map(move |(held, side, id)| {
                    <V as sts2_core::Hooks>::modify_hook(
                        held.listener(side, counted::<V>(counters, id, held.power)),
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
    counters: &[Counter<V>],
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
                    held.listener(
                        creature.side,
                        counted::<V>(counters, creature.id, held.power),
                    ),
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
    counters: &[Counter<V>],
    playing: Option<&CardInstance<V>>,
    effect: Effect<V>,
) -> ResolvedEffect<V> {
    let effect = match effect {
        Effect::Attack {
            dealer,
            target,
            damage,
            scaling,
        } => Resolved::Attack {
            target,
            damage: {
                let damage = carried::<V>(playing, creatures, damage);
                match scaling {
                    Scaling::Dynamic => {
                        modified::<V>(creatures, counters, dealer, Some(target), damage)
                    }
                    Scaling::Static => damage,
                }
            },
        },
        Effect::GainBlock {
            recipient,
            amount,
            scaling,
        } => {
            let amount = carried::<V>(playing, creatures, amount);
            Resolved::GainBlock {
                recipient,
                // `ModifyBlockAdditive` pays a card's block or a monster
                // move's, and nothing else. A relic's block is asked of no
                // listener this build fields.
                amount: match scaling {
                    Scaling::Dynamic => blocked::<V>(creatures, counters, recipient, amount),
                    Scaling::Static => amount,
                },
            }
        }
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
            scaling: Scaling::Dynamic,
        }),
        CardNumberModifier::Block => Some(Effect::GainBlock {
            recipient: dealer,
            amount: Amount::Var(key),
            // Every block a card prints on this build is a `BlockVar` with
            // `ValueProp.Move`.
            scaling: Scaling::Dynamic,
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
        resolve::<V>(
            &self.live.creatures,
            &self.live.counted,
            self.playing,
            effect,
        )
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
                // `ModifyHpLost` over what the block left, which is where a
                // power that caps a hit rather than shrinks it acts.
                let unblocked = hp_lost::<V>(
                    &self.live.creatures,
                    &self.live.counted,
                    target,
                    (damage - blocked).max(zero),
                );

                let hp = self.live.creatures[at].hp;
                let health_zero = V::Health::zero();
                let killed = hp > health_zero && unblocked >= V::Number::from(hp);
                self.live.creatures[at].hp = (hp - V::Health::from(unblocked)).max(health_zero);
                // `AfterDamageReceived` carries the `DamageResult`, so this is
                // what the hit took off rather than what it was swung for.
                let asked = super::power::notify_creature::<V>(
                    self.live,
                    target,
                    sts2_core::EventHook::DamageReceived { unblocked },
                );
                if let Some(effect) = asked.first() {
                    unimplemented!("a power asking for {effect:?} after a hit")
                }
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

    fn modify_energy(&mut self, amount: V::Number) {
        let size = |amount: V::Number| {
            num_traits::NumCast::from(amount)
                .unwrap_or_else(|| unimplemented!("an energy change of {amount:?}"))
        };
        // `PlayerCombatState` holds the pool as a count and clamps it at zero.
        match amount < V::Number::zero() {
            true => self.live.energy = self.live.energy.saturating_sub(size(-amount)),
            false => self.live.energy = self.live.energy + size(amount),
        }
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
        draw(
            self.live,
            self.rngs.reborrow(),
            self.relics,
            self.ascension,
            count,
        );
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

    fn upgraded(&self) -> bool {
        self.playing
            .unwrap_or_else(|| unimplemented!("the upgrade of no card being played"))
            .ident
            .upgrade
            != 0
    }

    fn transform(&mut self, card: CardAt<V>, into: CardIdent<V>) {
        // A new model for the replacement, so a new id. The original's own is
        // gone with it.
        let id = self.live.card_ids.mint();
        let pile = match card.pile() {
            Pile::Hand => &mut self.live.hand,
            Pile::Draw => &mut self.live.draw,
            Pile::Discard => &mut self.live.discard,
            Pile::Exhaust => &mut self.live.exhaust,
        };
        // `Transform` puts the replacement where the original stood, in the
        // pile it stood in. The place came with the name, so this looks once.
        let at: usize = num_traits::NumCast::from(card.index())
            .unwrap_or_else(|| unimplemented!("a pile deeper than this machine can index"));
        let held = pile.get_mut(at);
        let Some(held) = held.filter(|held| held.id == card.id()) else {
            unimplemented!("a card transformed after the fight moved it");
        };
        *held = CardInstance::new(into, id);
    }

    fn forge(&mut self, card: V::Card, key: V::CardVar, amount: V::Number) {
        // `ForgeCmd.Forge` creates one when the fight holds none it can reach.
        // The exhaust pile is not one it can reach, and is forged all the same.
        let held = [&self.live.hand, &self.live.draw, &self.live.discard]
            .into_iter()
            .flatten()
            .any(|held| held.ident.card == card);
        if !held {
            let id = self.live.card_ids.mint();
            self.live
                .hand
                .push(CardInstance::new(CardIdent::new(card), id));
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
            let id = self.live.card_ids.mint();
            self.live
                .discard
                .push(CardInstance::new(CardIdent::new(card), id));
        }
    }

    fn cards_in(&self, pile: Pile) -> Vec<CardAt<V>> {
        let held = match pile {
            Pile::Hand => &self.live.hand,
            Pile::Draw => &self.live.draw,
            Pile::Discard => &self.live.discard,
            Pile::Exhaust => &self.live.exhaust,
        };
        held.iter()
            .enumerate()
            .map(|(at, held)| {
                let at = num_traits::NumCast::from(at)
                    .unwrap_or_else(|| unimplemented!("a pile of more cards than a fight counts"));
                CardAt::new(held.id, pile, at)
            })
            .collect()
    }

    fn card_ident(&self, card: CardAt<V>) -> CardIdent<V> {
        let pile = match card.pile() {
            Pile::Hand => &self.live.hand,
            Pile::Draw => &self.live.draw,
            Pile::Discard => &self.live.discard,
            Pile::Exhaust => &self.live.exhaust,
        };
        let at: usize = num_traits::NumCast::from(card.index())
            .unwrap_or_else(|| unimplemented!("a pile deeper than this machine can index"));
        pile.get(at)
            .filter(|held| held.id == card.id())
            .unwrap_or_else(|| unimplemented!("a card read after the fight moved it"))
            .ident
    }

    fn random_card(&mut self, cards: &[CardAt<V>], stream: V::RunRngType) -> Option<CardAt<V>> {
        if cards.is_empty() {
            return None;
        }
        if stream != V::RunRngType::COMBAT_CARD_GENERATION {
            unimplemented!("a combat card selected from {stream:?}")
        }
        let bound = sts2_core::rng::Bound::try_new(
            num_traits::NumCast::from(cards.len())
                .expect("a combat pile's length fits the game's RNG integer"),
        )
        .expect("a nonempty candidate list has a nonzero bound");
        let at: usize = num_traits::NumCast::from(self.rngs.generation.next_int(bound))
            .expect("a bounded draw is a candidate index");
        Some(cards[at])
    }

    fn remove_card(&mut self, card: CardAt<V>) -> CardIdent<V> {
        let pile = match card.pile() {
            Pile::Hand => &mut self.live.hand,
            Pile::Draw => &mut self.live.draw,
            Pile::Discard => &mut self.live.discard,
            Pile::Exhaust => &mut self.live.exhaust,
        };
        let at: usize = num_traits::NumCast::from(card.index())
            .unwrap_or_else(|| unimplemented!("a pile deeper than this machine can index"));
        let held = pile
            .get(at)
            .filter(|held| held.id == card.id())
            .unwrap_or_else(|| unimplemented!("a card removed after the fight moved it"));
        let ident = held.ident;
        pile.remove(at);
        self.live.removed_from_deck.push(ident);
        ident
    }

    fn modify_card(
        &mut self,
        card: CardAt<V>,
        change: sts2_core::Change<V>,
        lasting: sts2_core::Lasting,
    ) {
        let pile = match card.pile() {
            Pile::Hand => &mut self.live.hand,
            Pile::Draw => &mut self.live.draw,
            Pile::Discard => &mut self.live.discard,
            Pile::Exhaust => &mut self.live.exhaust,
        };
        let at: usize =
            num_traits::NumCast::from(card.index()).expect("a card's pile index fits this machine");
        let held = pile
            .get_mut(at)
            .filter(|held| held.id == card.id())
            .expect("the card is still at its named place");
        held.modify(change, lasting);
    }

    fn discard_this(&mut self) {
        let playing = self.playing.expect("a card behavior has a card");
        let Some(at) = self.live.hand.iter().position(|held| held.id == playing.id) else {
            return;
        };
        self.live.discard.push(self.live.hand.remove(at));
    }

    fn summon(&mut self, monster: V::Monster, position: usize) -> CombatId {
        super::monster::summon(self.live, monster, position, self.rngs.niche)
    }
}

/// `CardPileCmd.Draw`, which shuffles the discard pile back in when the draw
/// pile runs dry.
///
/// A fight with every card in hand draws short rather than manufacture one.
pub(super) fn draw<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut [HeldRelic<V>],
    ascension: V::AscensionLevel,
    count: usize,
) {
    for _ in 0..count {
        if live.draw.is_empty() {
            if live.discard.is_empty() {
                return;
            }
            reshuffle(live, rngs.shuffle);
        }
        let drawn = live.draw.remove(0);
        live.hand.push(drawn.clone());
        // `Hook.AfterCardDrawn`, raised for each card inside the loop rather
        // than once for the whole draw. So a card that spends the pool as it
        // arrives has spent it before the next card is taken.
        //
        // The game raises it on every listener in the fight. This asks the
        // card that arrived, which is the only listener ported to answer it.
        let asked = V::event_hook(
            sts2_core::Listener::Card {
                card: drawn.ident.card,
                printed: &drawn.vars,
            },
            sts2_core::EventHook::CardDrawn {
                card: drawn.ident.card,
            },
        );
        let mut board =
            Board::for_card(live, rngs.reborrow(), relics, ascension, None, Some(&drawn));
        super::hook::apply::<V, _>(&mut board, asked);
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

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_content::v0_107_1::{
        V0_107_1,
        cards::{CardVar, Cards, StatusCard},
        characters::Character,
        powers::Power,
        runstate::CombatSide,
    };
    use sts2_core::{CardId, EffectResolution, ModelId, fixture::at};

    type V = V0_107_1;

    fn infection_damage_with(power: Power) -> i32 {
        let player = at(0);
        let creatures = [Creature {
            id: player,
            model: Some(ModelId::Character(Character::Regent)),
            side: CombatSide::Player,
            hp: 10,
            max_hp: 10,
            block: 0,
            alive: true,
            powers: vec![Stack {
                power,
                amount: PowerAmount::Counted(2),
            }],
            next_move: None,
        }];
        let card = CardInstance::new(
            CardIdent::new(Cards::Status(StatusCard::Infection)),
            CardId::new(0),
        );

        resolve::<V>(
            &creatures,
            &[],
            Some(&card),
            Effect::Attack {
                dealer: player,
                target: player,
                damage: Amount::Var(CardVar::Damage),
                scaling: Scaling::Static,
            },
        )
        .amount()
    }

    #[test]
    fn infection_damage_does_not_benefit_from_strength() {
        assert_eq!(infection_damage_with(Power::StrengthPower), 3);
    }

    #[test]
    fn infection_damage_does_not_benefit_from_vulnerable() {
        assert_eq!(infection_damage_with(Power::VulnerablePower), 3);
    }
}
