//! The fight, as behaviour acts on it.
//!
//! One board for both kinds of behaviour. The player plays a card and a
//! monster makes a move. Past that the two issue the game's same commands
//! against the same fight.
//!
//! Everything here is one of those commands with nothing listening. This
//! build's fights hold no power, relic or card that hooks any of them yet. A
//! command that needs one stops the run where it stands.

use num_traits::{Saturating as _, Zero};
use smallvec::SmallVec;
use sts2_core::rng::GameRng;
use sts2_core::{
    Amount, CardAt, CardId, CardIdent, CardNumberModifier, CardTable as _, CardVars, CombatId,
    Decimal, Effect, EffectResolution, EnchantHook, Fight, HookEffect, Modify, ModifyHook, Pile,
    PowerAmount, Rules, RunStreamKinds, Scaling, SideKinds, Stack,
};

use super::listen::{Listener, Walk};
use super::live::{CardInstance, Face, Fighter, HeldPower, Live};
use super::powers::Powers as _;
use crate::run::Belts;
use crate::stream::CombatRngs;

/// The fight an effect is resolved against.
pub(super) struct Board<'a, V: Rules> {
    pub(super) live: &'a mut Live<V>,
    /// The run streams an effect can draw from while it resolves.
    rngs: CombatRngs<'a>,
    /// The player's relic belt, for hooks raised while an effect resolves.
    belts: &'a mut Belts<V>,
    /// Whose effect this is. The player for a card, the monster for a move.
    actor: CombatId,
    /// The creature a card was pointed at.
    target: Option<CombatId>,
    /// The card whose effect this is, by id. A card being played stands in
    /// the play pile, and one answering a hook stands where the hook found
    /// it. Nothing here for a monster's move, which prints no numbers of its
    /// own.
    playing: Option<CardId<V>>,
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

    /// The player died, or the last primary enemy left, while this effect
    /// was resolving.
    ///
    /// `CheckWinCondition` runs after the command, so this is read once the
    /// effect is over, through [`Board::ended`]. A loss is `_pendingLoss`,
    /// which stands. A win is `IsEnding` as the kill saw it, which a primary
    /// enemy arriving after it undoes.
    ended: Option<super::Ended>,
    /// The `PlayerChoiceContext` this board's commands run in, which says
    /// what a question raised inside them does.
    choice: Choice,
    /// The hits of the attack under way that wait on a question, in order.
    held: Vec<Effect<V>>,
    /// The hit of the attack under way whose `BeforeDamageReceived` asked,
    /// which lands before the rest once the answer is in.
    half: Option<super::live::HalfHit<V>>,
    /// The command over many targets that hit was part of, which waits
    /// with it.
    held_command: Option<Box<super::live::HeldCommand<V>>>,
    /// The model of a monster that left the fight during its own move. The
    /// game's model object outlives the creature, so the rest of the move
    /// still reads and writes it.
    left: Option<V::MonsterState>,
    /// The results of the attack command whose hits are landing, which
    /// `AttackCommand.Results` gathers.
    landed: Option<sts2_core::AttackResults<V>>,
    /// Whether a hit lands inside a [`Board::damage_command`], to wait for
    /// the command's last before its hooks run.
    commanding: bool,
    /// Whether the [`Board::damage_command`] under way has asked whether its
    /// dealer stands. `CreatureCmd.Damage` asks once, before its first
    /// target.
    dealer_asked: bool,
    /// The card making the attack under way, as it stood when the attack
    /// began. `DamageCmd.Attack` takes its numbers as the command is built,
    /// so a hit later in it reads them as they were.
    attacking_as: Option<CardInstance<V>>,
    /// The player a monster's command reaches while it runs for each player
    /// in turn, which [`Board::seat`] reads before the actor.
    reaching: Option<sts2_core::SeatIndex>,
}

/// The game's `PlayerChoiceContext`, as far as it decides what a question
/// raised inside a command does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Choice {
    /// `ThrowingPlayerChoiceContext`, which throws on a question. A hook's
    /// commands take it.
    Throwing,
    /// A card's or a potion's play, whose context blocks. The effect stands
    /// still on the question, and [`super::live::Deferred`] holds what waits.
    Blocking,
    /// A monster's move. Its attack is `Execute(null)`, which blocks, and
    /// every other command it runs takes a `ThrowingPlayerChoiceContext`.
    Move { attacking: bool },
}

/// What one hit took off one creature's hit points, before its block is
/// counted in.
#[derive(Debug, Clone, PartialEq)]
struct Lost<V: Rules> {
    receiver: CombatId,
    lost: V::Number,
    overkill: V::Number,
    killed: bool,
}

/// One hit of a `CreatureCmd.Damage` that has landed, and waits for its
/// hooks.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Landed<V: Rules> {
    dealer: Option<CombatId>,
    target: CombatId,
    powered: bool,
    unblockable: bool,
    card: Option<V::Card>,
    attack: bool,
    blocked: V::Number,
    block_broken: bool,
    fully_blocked: bool,
    results: SmallVec<[Lost<V>; 2]>,
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
        /// The creature swinging, which is not always the one acting.
        dealer: CombatId,
        target: CombatId,
        damage: V::Number,
        /// `IsPoweredAttack`, which is what a redirect of the damage asks.
        powered: bool,
        /// The card that deals it, if a card does.
        card: Option<V::Card>,
    },
    GainBlock {
        recipient: CombatId,
        amount: V::Number,
        /// `IsCardOrMonsterMove`.
        moved: bool,
        /// The play that gives it.
        play: Option<sts2_core::PlayAt>,
        /// `cardSource`: which copy gives it.
        source: Option<CardId<V>>,
        /// The belts whose block hooks changed it.
        modifying: SmallVec<[V::Relic; 1]>,
        /// Whether the decimal was above nothing before the floor.
        positive: bool,
    },
    /// A hit inside an attack while a question an earlier hit raised waits.
    /// It lands after the answer, resolved then.
    Held(Effect<V>),
}

impl<V: Rules> EffectResolution<V> for ResolvedEffect<V> {
    fn amount(&self) -> V::Number {
        match self.effect {
            Resolved::Attack { damage, .. } => damage,
            Resolved::GainBlock { amount, .. } => amount,
            Resolved::Held(effect) => {
                unimplemented!("reading what {effect:?} comes to while it waits on a question")
            }
        }
    }

    fn positive(&self) -> bool {
        match self.effect {
            Resolved::Attack { damage, .. } => damage > V::Number::zero(),
            Resolved::GainBlock { positive, .. } => positive,
            Resolved::Held(effect) => {
                unimplemented!("reading what {effect:?} comes to while it waits on a question")
            }
        }
    }
}

/// `OstyCmd.Summon` from a hook, outside any card or move.
pub(super) fn summon_osty<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut Belts<V>,
    ascension: V::AscensionLevel,
    amount: V::Number,
) {
    Board::for_card(seat, live, rngs, belts, ascension, None, None).summon_osty(amount);
}

impl<'a, V: Rules> Board<'a, V> {
    /// The seat whose player acts: the player a monster's command reaches,
    /// or else the actor's, or its owner's for a pet. A monster's move in a
    /// solo fight reaches the one player.
    ///
    /// # Panics
    ///
    /// A monster's move in a fight of several players, which names its
    /// player some other way nobody has ported.
    #[inline(always)]
    fn seat(&self) -> sts2_core::SeatIndex {
        if self.live.players.len() == 1 {
            return sts2_core::SeatIndex(0);
        }
        self.seat_of_several()
    }

    /// [`Board::seat`] in a fight of several players.
    #[cold]
    fn seat_of_several(&self) -> sts2_core::SeatIndex {
        self.reaching
            .or_else(|| self.live.seat_of(self.actor))
            .unwrap_or_else(|| {
                let players = self.live.players.len();
                let model = (self.live.creatures.iter())
                    .find(|creature| creature.id == self.actor)
                    .and_then(|creature| creature.model);
                unimplemented!(
                    "{model:?}'s move reaching a player in a fight of {players}, \
                     with no player named"
                )
            })
    }

    /// The actor's seat, and the first player's for a monster's move, which
    /// stands for what no one player owns.
    #[inline(always)]
    fn actor_seat(&self) -> sts2_core::SeatIndex {
        if self.live.players.len() == 1 {
            return sts2_core::SeatIndex(0);
        }
        self.live
            .seat_of(self.actor)
            .unwrap_or(sts2_core::SeatIndex(0))
    }

    /// The seat whose player `listener` acts for, and [`Board::seat`] for
    /// one that belongs to no player.
    #[inline(always)]
    fn listener_seat(&self, listener: Listener<V>) -> sts2_core::SeatIndex {
        if self.live.players.len() == 1 {
            return sts2_core::SeatIndex(0);
        }
        super::listen::owner(self.live, listener).unwrap_or_else(|| self.seat())
    }

    /// A board for one card, played from the player.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn for_card(
        seat: sts2_core::SeatIndex,
        live: &'a mut Live<V>,
        rngs: CombatRngs<'a>,
        belts: &'a mut Belts<V>,
        ascension: V::AscensionLevel,
        target: Option<CombatId>,
        playing: Option<CardId<V>>,
    ) -> Self {
        let actor = seat.creature();
        Board {
            live,
            rngs,
            belts,
            actor,
            target,
            playing,
            intent_damage: V::Number::zero(),
            // Nothing a card does reads an intent, and a card is not one.
            intent_repeats: num_traits::zero(),
            ascension,
            skipped: Vec::new(),
            ended: None,
            choice: Choice::Throwing,
            held: Vec::new(),
            half: None,
            held_command: None,
            left: None,
            landed: None,
            commanding: false,
            dealer_asked: false,
            attacking_as: None,
            reaching: None,
        }
    }

    /// This board for a card's or a potion's own effect, which a question
    /// inside it stands still.
    pub(super) fn blocking(mut self) -> Self {
        self.choice = Choice::Blocking;
        self
    }

    /// This board acting for `seat`'s player, where a potion is aimed at
    /// them, or for its actor's where nothing is.
    pub(super) fn reaching(mut self, seat: Option<sts2_core::SeatIndex>) -> Self {
        self.reaching = seat;
        self
    }

    /// Whether a question raised now stands this board still rather than
    /// throwing.
    #[inline]
    fn pausable(&self) -> bool {
        matches!(
            self.choice,
            Choice::Blocking | Choice::Move { attacking: true }
        )
    }

    /// Whether this board is a monster's move.
    #[inline]
    fn moving(&self) -> bool {
        matches!(self.choice, Choice::Move { .. })
    }

    /// A board for one move, made by a monster.
    pub(super) fn for_move(
        live: &'a mut Live<V>,
        rngs: CombatRngs<'a>,
        belts: &'a mut Belts<V>,
        ascension: V::AscensionLevel,
        acting: (CombatId, V::Number, V::Count),
    ) -> Self {
        let (actor, intent_damage, intent_repeats) = acting;
        Board {
            live,
            rngs,
            belts,
            actor,
            target: None,
            playing: None,
            intent_damage,
            intent_repeats,
            ascension,
            skipped: Vec::new(),
            ended: None,
            choice: Choice::Move { attacking: false },
            held: Vec::new(),
            half: None,
            held_command: None,
            left: None,
            landed: None,
            commanding: false,
            dealer_asked: false,
            attacking_as: None,
            reaching: None,
        }
    }

    /// `Creature.CanReceivePowers` for a creature in the fight: no listener's
    /// `ShouldAllowHitting` refuses it. `IsAlive` is `CurrentHp > 0`, so a
    /// creature a hit took to nothing is dead to it before it dies.
    fn can_receive_powers(&self, creature: CombatId) -> bool {
        let alive = self
            .live
            .creatures
            .iter()
            .chain(self.live.created.iter().map(|created| &created.fighter))
            .any(|held| held.id == creature && held.alive && held.hp > V::Health::zero());
        !super::listen::any_decides::<V>(
            self.walk(),
            sts2_core::DecisionHook::ShouldAllowHitting { creature, alive },
            false,
        )
    }

    /// The card whose effect this is, as the fight holds it now.
    ///
    /// # Panics
    ///
    /// A card the fight no longer holds.
    fn acting(&self) -> Option<CardInstance<V>> {
        if self.attacking_as.is_some() {
            return self.attacking_as;
        }
        self.playing.map(|id| {
            self.live.players[self.seat()]
                .cards
                .named(id)
                .unwrap_or_else(|| unimplemented!("a card acting after it left the fight"))
        })
    }

    /// Where a card the fight named still stands.
    ///
    /// # Panics
    ///
    /// A card the fight has moved since it named it.
    fn standing(&self, card: CardAt<V>) -> usize {
        let seat = self.seat();
        let at: usize = num_traits::NumCast::from(card.index())
            .unwrap_or_else(|| unimplemented!("a pile deeper than this machine can index"));
        match self.live.players[seat].cards.get(card.pile(), at) {
            Some(held) if held.id == card.id() => at,
            _ => unimplemented!("a card named at {card:?} after the fight moved it"),
        }
    }

    /// Where a creature sits, or nothing for one no longer in the fight.
    pub(super) fn at(&self, at: CombatId) -> Option<usize> {
        self.live
            .creatures
            .iter()
            .position(|creature| creature.id == at)
    }

    /// `PowerCmd.Apply<P>(at, amount, applier, cardSource)`.
    ///
    /// `applier` is the creature whose command it is, which the power keeps
    /// as `PowerModel.Applier`. A power the console puts on has none.
    #[tracing::instrument(level = "trace", skip_all)]
    /// `PowerCmd.Apply` over `targets`, which awaits each in turn. A question
    /// one of them raises inside an effect stands the rest still behind it.
    fn land_each(
        &mut self,
        targets: &[CombatId],
        power: V::Power,
        amount: PowerAmount<V::Number>,
        applier: Option<CombatId>,
    ) {
        for (at, &id) in targets.iter().enumerate() {
            if self.pausable() && self.live.asking.is_some() {
                self.live.defer(super::live::Deferred::Land(Lands {
                    targets: targets[at..].iter().copied().collect(),
                    power,
                    amount,
                    applier,
                    playing: self.playing,
                }));
                return;
            }
            self.land(id, power, amount, applier);
        }
    }

    pub(super) fn land(
        &mut self,
        at: CombatId,
        power: V::Power,
        amount: PowerAmount<V::Number>,
        applier: Option<CombatId>,
    ) {
        self.land_copy(at, power, amount, applier, None);
    }

    /// [`Board::land`] of a power whose `Target` its applier set first.
    pub(super) fn land_aimed(
        &mut self,
        at: CombatId,
        power: V::Power,
        amount: PowerAmount<V::Number>,
        applier: Option<CombatId>,
        target: CombatId,
    ) {
        self.land_fresh(at, power, amount, applier, None, Some(target), false);
    }

    /// [`Board::land`], where `copy` is the instance to build, a clone its
    /// caller wrote on. One that stacks onto an instance already there is
    /// dropped, as the game's is.
    fn land_copy(
        &mut self,
        at: CombatId,
        power: V::Power,
        amount: PowerAmount<V::Number>,
        applier: Option<CombatId>,
        copy: Option<V::PowerState>,
    ) {
        self.land_fresh(at, power, amount, applier, copy, None, false);
    }

    /// [`Board::land_copy`], where `fraction` says the zero amount is a
    /// fraction below one the game passes the zero check with.
    #[allow(clippy::too_many_arguments)]
    fn land_fresh(
        &mut self,
        at: CombatId,
        power: V::Power,
        amount: PowerAmount<V::Number>,
        applier: Option<CombatId>,
        copy: Option<V::PowerState>,
        target: Option<CombatId>,
        fraction: bool,
    ) {
        self.ran_on();
        // `PowerCmd.Apply` returns on a zero amount. `Apply<T>` stacks one
        // onto an instance already there first, so a copy, which comes
        // through `Apply`, returns here, and any other at the fresh instance.
        let zero = amount == PowerAmount::Counted(V::Number::zero()) && !fraction;
        if zero && copy.is_some() {
            return;
        }
        // `IsEnding`, and `CanReceivePowers`, which a creature no listener
        // lets be hit fails. A monster the fight has taken out is one,
        // because its combat state is gone with it.
        if self.over() {
            return;
        }
        if !self.can_receive_powers(at) {
            return;
        }
        if let Some(created) = self
            .live
            .created
            .iter()
            .find(|created| created.fighter.id == at)
        {
            if zero {
                if created.powers.iter().any(|held| held.power() == power) {
                    unimplemented!(
                        "a zero {power:?} onto one a creature not yet in the fight holds"
                    );
                }
                return;
            }
            return self.land_on_created(at, power, amount, applier, copy, target);
        }
        let id = at;
        let Some(at) = self.at(at) else { return };

        // `FindExistingInstanceForStacking`: a creature already carrying this
        // power takes the amount through `ModifyAmount`, which is the whole of
        // what happens. Everything below to the stacking itself is the other
        // branch, the one that builds an instance.
        // An instanced power builds another instance beside any there, save
        // one kept apart by its applier, which stacks onto that applier's.
        let held = self
            .live
            .powers
            .of(id)
            .filter(|held| held.power() == power)
            .count();
        let per_applier = sts2_core::PowerTable::per_applier(power);
        let stacks_onto = self
            .live
            .powers
            .of(id)
            .any(|held| held.power() == power && (!per_applier || held.applier == applier));
        let fresh = !stacks_onto || (sts2_core::PowerTable::instanced(power) && !per_applier);
        // `FindExistingInstanceForStacking`: a counter stacks into the one
        // already there through `ModifyAmount`. An endless instance is one
        // instance however many times it lands.
        if !fresh {
            if let PowerAmount::Counted(more) = amount {
                self.modify_amount_from(id, power, more, applier);
            }
            return;
        }
        if zero {
            return;
        }

        let (amount, modifiers) = match amount {
            PowerAmount::Counted(amount) => {
                let (amount, modifiers) = self.received(id, power, amount, applier);
                // `IsPrimaryEnemy || IsSecondaryEnemy`, which is the enemies'
                // side.
                let amount = match self.live.creatures[at].side == V::CombatSide::ENEMY {
                    true => self.live.scaling.power::<V>(power, amount),
                    false => amount,
                };
                (PowerAmount::Counted(amount), modifiers)
            }
            // The game's endless amount is -1, which `IsInfinite` reads, and
            // which the listeners are handed. One that takes it to nothing
            // stops it landing.
            PowerAmount::Endless => {
                if super::listen::heard::<V>(
                    self.live,
                    self.belts,
                    sts2_core::ModelHook::TryModifyPowerAmountReceived,
                ) {
                    let endless = -num_traits::one::<V::Number>();
                    let (amount, modifiers) = self.received(id, power, endless, applier);
                    let amount = match amount {
                        amount if amount == endless => PowerAmount::Endless,
                        amount if amount == V::Number::zero() => PowerAmount::Counted(amount),
                        amount => unimplemented!("an endless {power:?} changed to {amount:?}"),
                    };
                    (amount, modifiers)
                } else {
                    (PowerAmount::Endless, SmallVec::new())
                }
            }
        };
        let carrier = self.live.creatures[at].carrier();
        let state = self.apply_fresh(carrier, power, amount, applier, target, copy);
        // The hook can take the creature out of the fight, or kill one that
        // stays in it, and `CanReceivePowers` is asked again.
        let Some(found) = self.at(id) else {
            return;
        };
        if !self.can_receive_powers(id) {
            return;
        }
        debug_assert_eq!(found, at, "a power landing does not reorder the fight");
        if self
            .live
            .powers
            .of(id)
            .filter(|held| held.power() == power)
            .count()
            != held
        {
            unimplemented!("{power:?} landing during its own `BeforeApplied`")
        }
        // `ApplyInternal`, which puts nothing on for an amount of zero. A
        // fraction below one is not zero there, and floors to it after.
        let landed = fraction || amount != PowerAmount::Counted(V::Number::zero());
        if landed {
            self.live.powers.push(
                id,
                HeldPower::landing(state, amount.into(), applier, target),
            );
            self.live.history.power_received(power, applier);
        }
        self.power_received(modifiers, power);
        if landed {
            self.applied(id, power);
            // An endless power lands no number, so the hook that reads one has
            // nothing to read.
            if let PowerAmount::Counted(delta) = amount {
                self.amount_changed(id, power, delta, applier);
            }
        }
    }

    /// [`Board::land_copy`] onto a creature made and not yet in the fight.
    ///
    /// `PowerCmd.Apply` runs as it does for one in the fight: the hooks
    /// around the amount, `BeforeApplied`, and `AfterPowerAmountChanged`. The
    /// instance waits on the creature, where it hears nothing, until the
    /// creature joins.
    ///
    /// # Panics
    ///
    /// A power that stacks there, an endless one, or one whose class answers
    /// `AfterApplied`, which nothing has needed.
    fn land_on_created(
        &mut self,
        id: CombatId,
        power: V::Power,
        amount: PowerAmount<V::Number>,
        applier: Option<CombatId>,
        copy: Option<V::PowerState>,
        target: Option<CombatId>,
    ) {
        use sts2_core::PowerTable as _;

        let created = |live: &Live<V>| {
            live.created
                .iter()
                .position(|created| created.fighter.id == id)
                .expect("the creature was made")
        };
        let at = created(self.live);
        if self.live.created[at]
            .powers
            .iter()
            .any(|held| held.power() == power)
        {
            unimplemented!("{power:?} stacking on a creature not yet in the fight");
        }
        let PowerAmount::Counted(amount) = amount else {
            unimplemented!("an endless {power:?} on a creature not yet in the fight");
        };
        if power.overrides_virtual(sts2_core::PowerVirtual::AfterApplied) {
            unimplemented!("{power:?}'s AfterApplied on a creature not yet in the fight");
        }
        let (amount, modifiers) = self.received(id, power, amount, applier);
        let amount = match self.live.created[at].fighter.side == V::CombatSide::ENEMY {
            true => self.live.scaling.power::<V>(power, amount),
            false => amount,
        };
        let carrier = self.live.created[at].fighter.carrier();
        let state = self.apply_fresh(
            carrier,
            power,
            PowerAmount::Counted(amount),
            applier,
            target,
            copy,
        );
        if amount.is_zero() {
            return;
        }
        let at = created(self.live);
        self.live.created[at].powers.push(HeldPower::landing(
            state,
            PowerAmount::Counted(amount).into(),
            applier,
            target,
        ));
        self.live.history.power_received(power, applier);
        self.power_received(modifiers, power);
        self.amount_changed(id, power, amount, applier);
    }

    /// `Hook.ModifyPowerAmountReceived` and the hooks `PowerCmd` asks around
    /// it, before `amount` of `power` lands on `target`.
    ///
    /// What lands, and the listeners whose `TryModifyPowerAmountReceived`
    /// changed it, which [`Board::power_received`] tells afterwards.
    fn received(
        &mut self,
        target: CombatId,
        power: V::Power,
        amount: V::Number,
        applier: Option<CombatId>,
    ) -> (V::Number, Received<V>) {
        use sts2_core::{ModelHook, Overrides};

        // One look for the lot, since a fight rarely has any of them.
        const AROUND: Overrides = Overrides::of(ModelHook::BeforePowerAmountChanged)
            .with(ModelHook::ModifyPowerAmountGivenAdditive)
            .with(ModelHook::ModifyPowerAmountGivenMultiplicative)
            .with(ModelHook::AfterModifyingPowerAmountGiven)
            .with(ModelHook::TryModifyPowerAmountReceived);
        if !super::listen::heard_any::<V>(self.live, self.belts, AROUND) {
            return (amount, Received::<V>::new());
        }
        // `Hook.BeforePowerAmountChanged`, which no listener here answers with
        // a command.
        let source = self.playing;
        if super::listen::heard::<V>(self.live, self.belts, ModelHook::BeforePowerAmountChanged) {
            let target_side = self
                .live
                .creatures
                .iter()
                .chain(self.live.created.iter().map(|created| &created.fighter))
                .find(|creature| creature.id == target)
                .expect("a power lands on a creature in the fight or made for it")
                .side;
            // A creature made for the fight holds its powers beside it.
            let artifact =
                |held: &HeldPower<V>| held.power() == <V::Power as sts2_core::PowerKinds>::ARTIFACT;
            let target_artifact = match self
                .live
                .created
                .iter()
                .find(|created| created.fighter.id == target)
            {
                Some(created) => created.powers.iter().any(artifact),
                None => self.live.powers.of(target).any(artifact),
            };
            let hook = sts2_core::EventHook::PowerAmountChanging {
                power,
                amount,
                target,
                target_side,
                target_artifact,
                applier,
                source,
            };
            if let Some((listener, effect)) =
                super::listen::first_answer::<V>(self.actor_seat(), self.live, self.belts, hook)
            {
                unimplemented!("{listener:?} asking for {effect:?} before {power:?} changes");
            }
        }
        // `ModifyPowerAmountGiven`, asked when the applier is in the fight.
        let mut amount = amount;
        if let Some(giver) = applier.filter(|&applier| self.at(applier).is_some()) {
            let modifying;
            (amount, modifying) =
                given_by::<V>(self.walk(), giver, Some(target), power, amount, source);
            // `AfterModifyingPowerAmountGiven`, asked of each relic that
            // changed it. The game asks once the power has landed, and asking
            // here instead is the same while none of them asks for anything.
            // They are the belt of the player who gives it.
            let seat = match modifying.is_empty() {
                true => sts2_core::SeatIndex(0),
                false => self.live.seat_of(giver).unwrap_or_else(|| self.seat()),
            };
            for relic in modifying {
                let hook = sts2_core::EventHook::PowerAmountGivenModified { power };
                if let Some(effect) =
                    super::listen::ask_relic(&*self.live, seat, self.belts, relic, hook).first()
                {
                    unimplemented!("{relic:?} asking for {effect:?} once it changed {power:?}")
                }
            }
        }
        received_by::<V>(self.walk(), target, power, amount)
    }

    /// `Hook.AfterModifyingPowerAmountReceived`, asked of each listener that
    /// changed what `power` landed with.
    fn power_received(&mut self, modifiers: Received<V>, power: V::Power) {
        for modifier in modifiers {
            let (holder, modifier) = match modifier {
                Receiver::Power(holder, modifier) => (holder, modifier),
                Receiver::Relic(seat, relic) => {
                    let hook = sts2_core::EventHook::PowerAmountReceivedModified { power };
                    let asked =
                        super::listen::ask_relic(&*self.live, seat, self.belts, relic, hook);
                    if let Some(effect) = asked.first() {
                        unimplemented!("{relic:?} asking for {effect:?} once it changed {power:?}");
                    }
                    continue;
                }
            };
            let Some(at) = self
                .live
                .powers
                .of(holder)
                .position(|held| held.power() == modifier)
            else {
                continue;
            };
            let mut asked = sts2_core::HookEffects::new();
            self.live.ask_power(
                Some(self.belts),
                holder,
                at,
                sts2_core::EventHook::PowerAmountReceivedModified { power },
                &mut asked,
            );
            for effect in asked {
                match effect {
                    HookEffect::Decrement => {
                        self.modify_amount(holder, modifier, -num_traits::one::<V::Number>())
                    }
                    effect => unimplemented!(
                        "{modifier:?} asking for {effect:?} once it changed what landed"
                    ),
                }
            }
        }
    }

    /// `PowerModel.AfterApplied`, asked of the instance of `power` that `id`
    /// has just taken on, and of nothing else.
    /// `AfterAddedToRoom` for a monster the fight added: the powers it
    /// enters with, each through `PowerCmd.Apply` from itself.
    fn arrived(&mut self, id: CombatId, monster: V::Monster) {
        let side = self.live.side;
        let model = self
            .live
            .machines
            .iter()
            .find(|held| held.creature == id)
            .map_or_else(|| sts2_core::MonsterModel::new(monster), |held| held.state);
        // `CombatState.GetOpponentsOf`, pets included.
        let opponents: SmallVec<[CombatId; 4]> = self
            .live
            .creatures
            .iter()
            .filter(|creature| creature.side != <V::CombatSide as SideKinds>::ENEMY)
            .map(|creature| creature.id)
            .collect();
        for (power, amount) in V::monster_starting_aimed_powers(monster) {
            for &opponent in &opponents {
                self.land_aimed(id, power, amount, Some(id), opponent);
            }
        }
        let entering: SmallVec<[_; 2]> =
            V::monster_starting_powers(monster, self.live.ascension, side, &model).collect();
        for (power, amount) in entering {
            self.land(id, power, amount, Some(id));
        }
        let given: SmallVec<[_; 2]> =
            V::monster_starting_opponent_powers(monster, self.live.ascension).collect();
        for (power, amount) in given {
            for &opponent in &opponents {
                self.land(opponent, power, amount, Some(id));
            }
        }
        if let Some(at) = self.at(id)
            && let Some(hp) = V::monster_hp_after_added(monster, &model, self.live.creatures[at].hp)
        {
            self.live.creatures[at].hp = hp;
        }
    }

    /// `CombatManager.AfterCreatureAdded`: an enemy that joins on the
    /// player's side rolls its move there and then.
    ///
    /// Not for a monster that opens on a state something built for it, whose
    /// roll from that state lands where it stands.
    fn roll_joined(&mut self, id: CombatId) {
        if self.live.side != <V::CombatSide as SideKinds>::PLAYER {
            return;
        }
        let ascension = self.live.ascension;
        super::turn::roll_moves_of::<V>(self.live, self.belts, ascension, self.rngs.ai, |at| {
            at == id
        })
        .unwrap_or_else(|e| unimplemented!("rolling the move of a monster that joined: {e}"));
    }

    fn applied(&mut self, id: CombatId, power: V::Power) {
        // The instance that just landed, which is the last of its kind.
        let Some(at) = self
            .live
            .powers
            .of(id)
            .enumerate()
            .filter(|(_, held)| held.power() == power)
            .map(|(at, _)| at)
            .last()
        else {
            return;
        };
        let mut asked = sts2_core::HookEffects::new();
        self.live.ask_power(
            Some(self.belts),
            id,
            at,
            sts2_core::EventHook::Applied,
            &mut asked,
        );
        if asked.contains(&HookEffect::DowngradeUpgraded) {
            asked.retain(|effect| *effect != HookEffect::DowngradeUpgraded);
            let downgraded = self.downgrade_upgraded();
            self.live.kept_mut(id, power).cards.extend(downgraded);
        }
        let (owned, cards): (SmallVec<[_; 2]>, SmallVec<[_; 2]>) = asked
            .into_iter()
            .partition(|effect| matches!(effect, HookEffect::ApplyToOwner(..)));
        self.write_cards(cards);
        // `PowerCmd.Apply` on the power's owner, from nobody.
        for effect in owned {
            if let HookEffect::ApplyToOwner(power, amount) = effect {
                self.land(id, power, amount, None);
            }
        }
    }

    /// `PowerCmd.ModifyAmount` on the instance of `power` that `id` carries,
    /// by a signed offset.
    ///
    /// A counted power whose stack reaches zero is gone. Temporary Strength
    /// returns exactly what it took, and a zero instance left behind shows a
    /// creature carrying Strength.
    pub(super) fn modify_amount(&mut self, id: CombatId, power: V::Power, offset: V::Number) {
        self.modify_amount_from(id, power, offset, None);
    }

    /// [`Board::modify_amount`] for the creature whose command it is.
    fn modify_amount_from(
        &mut self,
        id: CombatId,
        power: V::Power,
        offset: V::Number,
        applier: Option<CombatId>,
    ) {
        // `FindExistingInstanceForStacking`: a power kept apart by its applier
        // stacks onto that applier's instance alone.
        let per_applier = sts2_core::PowerTable::per_applier(power);
        let stacks = |held: &super::live::HeldPower<V>| {
            held.power() == power && (!per_applier || held.applier == applier)
        };
        if !self.live.powers.of(id).any(stacks) {
            return;
        }
        let (offset, modifiers) = self.received(id, power, offset, applier);
        let Some(held) = self.live.powers.of_mut(id).find(|held| stacks(held)) else {
            return;
        };
        let had = held
            .amount
            .count()
            .unwrap_or_else(|| unimplemented!("a count landing on an endless power"));
        let more: super::live::Amount = num_traits::NumCast::from(offset)
            .unwrap_or_else(|| unimplemented!("a power stacked by {offset:?}"));
        held.amount = super::live::Stacked::counted(had + more);
        let spent = held.amount.spent();
        self.live.history.power_received(power, applier);
        self.power_received(modifiers, power);
        self.amount_changed(id, power, offset, applier);
        // `ShouldRemoveDueToAmount`, and `PowerCmd.Remove` for a power it
        // says is done.
        if spent {
            self.remove_power_carried(id, power);
        }
    }

    /// `PowerCmd.ModifyAmount` on the instance of `power` a death took off
    /// `id`, which keeps its own amount in the detached list. The hooks around
    /// it run as for one still on its creature.
    ///
    /// A change that leaves it spent is `PowerCmd.Remove` again, which takes
    /// nothing off and runs `AfterRemoved` once more.
    ///
    /// # Panics
    ///
    /// No such instance, or one spent whose power overrides `AfterRemoved`.
    pub(super) fn modify_detached(&mut self, id: CombatId, power: V::Power, offset: V::Number) {
        let (offset, modifiers) = self.received(id, power, offset, None);
        let (_, held) = self
            .live
            .detached
            .iter_mut()
            .rev()
            .find(|(holder, held)| *holder == id && held.power() == power)
            .unwrap_or_else(|| unimplemented!("{power:?} changed with no instance to change"));
        let had = held
            .amount
            .count()
            .unwrap_or_else(|| unimplemented!("a count landing on an endless power"));
        let more: super::live::Amount = num_traits::NumCast::from(offset)
            .unwrap_or_else(|| unimplemented!("a power stacked by {offset:?}"));
        held.amount = super::live::Stacked::counted(had + more);
        let spent = held.amount.spent();
        self.live.history.power_received(power, None);
        self.power_received(modifiers, power);
        self.amount_changed(id, power, offset, None);
        if spent
            && sts2_core::PowerTable::overrides_virtual(
                power,
                sts2_core::PowerVirtual::AfterRemoved,
            )
        {
            unimplemented!("{power:?} spent on its instance a death took off");
        }
    }

    /// `Creature.Stun` builds a state and hands it to `SetMoveImmediate`. So a
    /// monster advertising a move it has to perform is not stunned out of it.
    fn stun_monster(
        &mut self,
        owner: CombatId,
        stun: &'static sts2_core::NextMove<V>,
        then: sts2_core::StunResume,
    ) {
        let Some(machine) = self
            .live
            .machines
            .iter_mut()
            .find(|held| held.creature == owner)
        else {
            return;
        };
        if !V::can_leave_current_move(machine.monster, &machine.machine) {
            return;
        }
        match then {
            sts2_core::StunResume::Interrupted => {
                let last = machine.machine.log.recent(1).next();
                match last {
                    Some(last) => machine.machine.interpose_to(last),
                    None => machine.machine.interpose(),
                }
            }
            sts2_core::StunResume::AfterInterrupted => {
                let after = V::follow_up(machine.monster, &machine.machine);
                machine.machine.interpose_to(after);
            }
            sts2_core::StunResume::At(then) => machine.machine.interpose_to(then),
        }
        if let Some(at) = self.at(owner) {
            self.live.creatures[at].next_move = Some(stun.into());
        }
    }

    /// `CardCmd.Downgrade` on every upgraded copy the player holds, in
    /// `AllCards` order, and the level each had.
    ///
    /// `Downgrade` does nothing while the fight is ending, but the level is
    /// still what the copy had.
    fn downgrade_upgraded(&mut self) -> Vec<(CardId<V>, u8)> {
        let seat = self.seat();
        let upgraded: Vec<(CardId<V>, u8)> = [Pile::Hand, Pile::Draw, Pile::Discard, Pile::Exhaust]
            .into_iter()
            .flat_map(|pile| self.live.players[seat].cards.iter(pile))
            .filter(|held| held.face.ident.upgrade > 0)
            .map(|held| (held.id, held.face.ident.upgrade))
            .collect();
        if !(self.over()) {
            for &(card, _) in &upgraded {
                self.live.players[seat].cards.write_named(card, |face| {
                    face.downgrade();
                    true
                });
            }
        }
        upgraded
    }

    /// `PowerCmd.Remove`, with what its `AfterRemoved` asks carried out.
    pub(super) fn remove_power_carried(&mut self, id: CombatId, power: V::Power) {
        // What it does to cards is to its holder's player's.
        let seat = self.live.seat_of(id).unwrap_or_else(|| self.actor_seat());
        for effect in self.live.remove_power_asking(seat, id, power) {
            match effect {
                HookEffect::KillCreature(creature) => self.kill_outright(creature, true),
                // `CardCmd.Upgrade` on a copy that has left the fight writes on
                // a model the fight no longer holds.
                HookEffect::UpgradeCopy { card, .. }
                    if self.live.players[seat].cards.named(card).is_none() => {}
                // `CardCmd.Upgrade` does nothing once the fight is ending, as
                // when the caster that died was the last enemy.
                HookEffect::UpgradeCopy { .. } if self.over() => {}
                HookEffect::UpgradeCopy { card, times } => {
                    self.live.players[seat].cards.write_named(card, |face| {
                        for _ in 0..times {
                            face.upgrade();
                        }
                        true
                    });
                }
                other => unimplemented!("{power:?} asking for {other:?} as it comes off"),
            }
        }
    }

    /// `AfterPowerAmountChanged`, once the amount has moved. It carries the
    /// offset rather than the stack the creature now holds, and an offset of
    /// zero raises nothing.
    ///
    /// A power asking for more of a power on its own holder lands it from
    /// `applier`, whose command moved the amount.
    fn amount_changed(
        &mut self,
        id: CombatId,
        power: V::Power,
        delta: V::Number,
        applier: Option<CombatId>,
    ) {
        if delta.is_zero() {
            return;
        }
        let mut walking = super::listen::Walking::begin(
            self.live,
            self.belts,
            sts2_core::EventHook::PowerAmountChanged {
                creature: id,
                power,
                delta,
                applier,
            },
        );
        let mut cards: SmallVec<[(Listener<V>, HookEffect<V>); 4]> = SmallVec::new();
        while let Some((listener, effect)) =
            walking.as_mut().and_then(|walking| self.next_of(walking))
        {
            match (listener, effect) {
                (Listener::Power { owner, .. }, HookEffect::ApplyToOwner(lent, amount)) => {
                    self.land(owner, lent, amount, applier)
                }
                // `CardPileCmd.Draw` for the power's owner.
                (Listener::Power { .. }, HookEffect::Draw(count)) => {
                    self.as_listener(listener, |board| sts2_core::Fight::draw(board, count));
                }
                // `CreatureCmd.Damage`, unpowered, from the power's owner.
                (Listener::Power { owner, .. }, HookEffect::Damage { target, amount }) => {
                    self.hit_unpowered(Some(owner), target, amount, false)
                }
                // `CreatureCmd.Damage` over `HittableEnemies`, unpowered, from
                // the power's owner.
                (Listener::Power { owner, .. }, HookEffect::DamageOpponents(amount)) => {
                    // One `CreatureCmd.Damage` over them all.
                    let targets = self.hittable_opponents_of(owner);
                    self.damage_command(|board| {
                        for target in targets {
                            board.hit_unpowered(Some(owner), target, amount, false);
                        }
                    });
                }
                // `CreatureCmd.GainBlock`, unpowered, on the power's owner.
                (Listener::Power { owner, .. }, HookEffect::GainBlock(amount)) => {
                    let block = resolve::<V>(
                        self.actor_seat(),
                        self.walk(),
                        None,
                        Effect::GainBlock {
                            recipient: owner,
                            amount: Amount::Number(amount),
                            scaling: Scaling::Static,
                        },
                    );
                    self.apply(block);
                }
                (listener, effect) => cards.push((listener, effect)),
            }
        }
        for (listener, effect) in cards {
            self.as_listener(listener, |board| board.write_cards([effect]));
        }
    }

    /// `CreatureCmd.Damage` on `target` once hooks worked `damage` out:
    /// `BeforeDamageReceived`, block, then hit points, then the deaths the
    /// hit caused. `dealer` is nothing for damage no creature deals, and
    /// `card` for damage no card deals. A target that has left the fight
    /// takes nothing.
    ///
    /// `attack`'s hits add their results to the attack's. Thorns deals its
    /// damage as a command of its own, whose results are not among them.
    #[allow(clippy::too_many_arguments)]
    /// `CreatureCmd.Damage` on one target: the hit lands, its hooks run,
    /// and a creature it killed dies.
    ///
    /// Inside a [`Board::damage_command`], the hit lands now and the rest
    /// waits for the command's other hits.
    pub(super) fn hit(
        &mut self,
        dealer: Option<CombatId>,
        target: CombatId,
        damage: V::Number,
        powered: bool,
        unblockable: bool,
        card: Option<V::Card>,
        attack: bool,
    ) {
        if !self.commanding {
            let mut dying = SmallVec::<[CombatId; 2]>::new();
            if let Some(landed) =
                self.land_hit(dealer, target, damage, powered, unblockable, card, attack)
            {
                let hit = self.next_hit();
                self.settle_hit(landed, hit, &mut dying);
            }
            self.kill_together(&dying);
            return;
        }
        // A hit a listener makes while this one lands is a command of its
        // own, and settles before this one goes on.
        self.commanding = false;
        let landed = self.land_hit(dealer, target, damage, powered, unblockable, card, attack);
        self.commanding = true;
        self.live.landing.extend(landed);
    }

    /// `CreatureCmd.Damage` over many targets: every hit `hits` makes lands
    /// first, then each one's hooks run in order, and then the creatures
    /// they killed die together. Until then a creature the hits took to
    /// nothing still stands, though `IsEnding` counts it dead.
    pub(super) fn damage_command(&mut self, hits: impl FnOnce(&mut Self)) {
        let outer = std::mem::replace(&mut self.commanding, true);
        let asked = std::mem::replace(&mut self.dealer_asked, false);
        let start = self.live.landing.len();
        let (halved, held) = (self.half.is_some(), self.held.len());
        hits(self);
        self.dealer_asked = asked;
        // A hit whose `BeforeDamageReceived` asked stands the command still.
        // What it landed waits for its hooks, and the hits after it wait to
        // land.
        if !halved && self.half.is_some() {
            self.hold_command(start, held);
            self.commanding = outer;
            return;
        }
        // What the hooks run is not this command's. A command they run
        // lands above this one's hits, and takes its own back off.
        self.commanding = false;
        let mut dying = SmallVec::<[CombatId; 2]>::new();
        let hit = self.next_hit();
        while self.live.landing.len() > start {
            let landed = self.live.landing.remove(start);
            self.settle_hit(landed, hit, &mut dying);
        }
        self.kill_together(&dying);
        self.commanding = outer;
    }

    /// The hit a damage command lands as, in the attack whose results are
    /// gathering: one past the last that landed.
    fn next_hit(&self) -> u8 {
        (self.landed.as_ref())
            .and_then(|results| results.last())
            .map_or(0, |last| last.hit + 1)
    }

    /// Hold the command under way behind its hit that asked: what it landed
    /// from `start` on, and the hits held from `held` on.
    #[cold]
    fn hold_command(&mut self, start: usize, held: usize) {
        self.held_command = Some(Box::new(super::live::HeldCommand {
            landed: self.live.landing.drain(start..).collect(),
            hits: self.held.len() - held,
        }));
    }

    /// `CreatureCmd.Kill` over `dying`, in order, then `CheckWinCondition`
    /// once.
    fn kill_together(&mut self, dying: &[CombatId]) {
        let mut died = false;
        for &creature in dying {
            if let Some(at) = self.at(creature) {
                self.die(at, false);
                died = true;
            }
        }
        if died {
            self.end_if_ending();
        }
    }

    /// The first half of `CreatureCmd.Damage` for one target: the hooks
    /// before it lands, the block it meets, and the hit points it takes.
    /// Nothing for a target already dead.
    #[inline(always)]
    #[allow(clippy::too_many_arguments)]
    fn land_hit(
        &mut self,
        dealer: Option<CombatId>,
        target: CombatId,
        damage: V::Number,
        powered: bool,
        unblockable: bool,
        card: Option<V::Card>,
        attack: bool,
    ) -> Option<Landed<V>> {
        let at = self.at(target)?;
        if !self.live.creatures[at].alive {
            return None;
        }
        let walking = super::listen::Walking::begin(
            self.live,
            self.belts,
            sts2_core::EventHook::DamageReceiving {
                target,
                amount: damage,
                dealer,
                powered,
                card,
            },
        );
        let asked_before = self.live.asking.is_some();
        self.after_hit(walking);
        let half = super::live::HalfHit {
            dealer,
            target,
            damage,
            powered,
            unblockable,
            card,
            attack,
        };
        // A listener's question stands the hit still, inside a play whose
        // context it awaits on. It lands once the answer is in.
        if !asked_before && self.live.asking.is_some() && matches!(self.choice, Choice::Blocking) {
            self.hold_half(half);
            return None;
        }
        self.land_rest(half)
    }

    /// Hold `half` to land once the question its `BeforeDamageReceived`
    /// asked is answered.
    #[cold]
    fn hold_half(&mut self, half: super::live::HalfHit<V>) {
        if self.landed.is_none() || self.half.is_some() {
            unimplemented!("a question before a hit lands, outside an attack's first wait")
        }
        self.half = Some(half);
    }

    /// The rest of [`Board::land_hit`] once `BeforeDamageReceived` is over:
    /// the block the hit meets, and the hit points it takes.
    #[inline(always)]
    fn land_rest(&mut self, half: super::live::HalfHit<V>) -> Option<Landed<V>> {
        let super::live::HalfHit {
            dealer,
            target,
            damage,
            powered,
            unblockable,
            card,
            attack,
        } = half;
        // What answered can have moved the creatures.
        let at = self.at(target)?;
        let zero = V::Number::zero();
        // `PetOwner?.Creature ?? originalTarget`: a pet's hit meets its
        // owner's block.
        let shield = self.live.creatures[at].owner.unwrap_or(target);
        let shield_at = self.at(shield)?;
        // `ValueProp.Unblockable` meets no block at all.
        let blocked = match unblockable {
            true => zero,
            false => self.live.creatures[shield_at].block.min(damage).max(zero),
        };
        self.live.creatures[shield_at].block = self.live.creatures[shield_at].block - blocked;
        // `ModifyHpLost` over what the block left, before and after the
        // damage finds who takes it. A power that caps a hit rather
        // than shrinks it acts here.
        let before = hp_lost_by::<V>(
            self.walk(),
            (damage - blocked).max(zero),
            &[
                ModifyHook::HpLostBeforeOsty { target },
                ModifyHook::HpLostBeforeOstyLate { target },
            ],
        );
        let taker = super::listen::unblocked_damage_target::<V>(self.walk(), target, powered);
        let unblocked = self.hp_lost_after_osty(taker, before, LossFrom { dealer, powered });
        // `WasBlockBroken` and `WasFullyBlocked`, as the command works
        // them out, on the creature the hit was aimed at. A hit a hook
        // takes to nothing is fully blocked by whatever block it met.
        let block = self.live.creatures[at].block;
        let block_broken = block <= zero && blocked > zero;
        let fully_blocked = !unblockable && (blocked > zero || block > zero) && unblocked == zero;

        // The taker loses what reached it. Anything past its health
        // goes back to the creature the hit was aimed at.
        let mut results: SmallVec<[Lost<V>; 2]> = SmallVec::new();
        let (lost, overkill, killed) = self.lose_hp(taker, unblocked);
        results.push(Lost {
            receiver: taker,
            lost,
            overkill,
            killed,
        });
        if taker != target {
            let rest = self.hp_lost_after_osty(target, overkill, LossFrom { dealer, powered });
            let (lost, overkill, killed) = match rest > zero {
                true => self.lose_hp(target, rest),
                false => (zero, zero, false),
            };
            results.push(Lost {
                receiver: target,
                lost,
                overkill,
                killed,
            });
        }
        Some(Landed {
            dealer,
            target,
            powered,
            unblockable,
            card,
            attack,
            blocked,
            block_broken,
            fully_blocked,
            results,
        })
    }

    /// The second half of `CreatureCmd.Damage` for one landed hit: each
    /// result's hooks, in the command's order. A creature the hit killed is
    /// not told it was hit. It joins `dying`, to die once the command's hooks
    /// are done.
    #[inline(always)]
    fn settle_hit(&mut self, landed: Landed<V>, hit: u8, dying: &mut SmallVec<[CombatId; 2]>) {
        let Landed {
            dealer,
            target,
            powered,
            unblockable,
            card,
            attack,
            blocked,
            block_broken,
            fully_blocked,
            results,
        } = landed;
        let zero = V::Number::zero();
        let pet_of = dealer
            .and_then(|dealer| self.at(dealer))
            .and_then(|at| self.live.creatures[at].owner);
        for Lost {
            receiver,
            lost,
            overkill,
            killed,
        } in results
        {
            let aimed = receiver == target;
            // `TotalDamage`, whose blocked part is the aimed
            // creature's alone.
            let total = match aimed {
                true => blocked + lost,
                false => lost,
            };
            if let Some(landed) = self.landed.as_mut().filter(|_| attack) {
                landed.push(sts2_core::DamageResult {
                    receiver,
                    hit,
                    total,
                    unblocked: lost,
                    overkill,
                    killed,
                });
            }
            if aimed && block_broken {
                let walking = super::listen::Walking::begin(
                    self.live,
                    self.belts,
                    sts2_core::EventHook::BlockBroken {
                        creature: target,
                        breaker: dealer,
                        pet_of,
                    },
                );
                self.after_hit(walking);
            }
            if lost > zero {
                self.current_hp_changed(receiver, -lost);
            }
            // `History.DamageReceived`, while the fight is in progress. One
            // that is ending plays no card after it, so nothing reads what it
            // records then, and it is not asked.
            if self.live.in_progress && !self.stopped() {
                self.live
                    .history
                    .damage_received(sts2_core::DamageReceived {
                        receiver,
                        dealer,
                        unblocked: lost,
                        powered,
                    });
            }
            let walking = super::listen::Walking::begin(
                self.live,
                self.belts,
                sts2_core::EventHook::DamageGiven {
                    dealer,
                    pet_of,
                    target: receiver,
                    total,
                    unblocked: lost,
                    powered,
                    fully_blocked: aimed && fully_blocked,
                    block_broken: aimed && block_broken,
                },
            );
            self.after_hit(walking);
            // `AfterDamageReceived` carries the `DamageResult`, so this
            // is what the hit took off rather than what it was swung
            // for.
            match killed {
                false => {
                    let walking = super::listen::Walking::begin(
                        self.live,
                        self.belts,
                        sts2_core::EventHook::DamageReceived {
                            target: receiver,
                            unblocked: lost,
                            blocked: match aimed {
                                true => blocked,
                                false => zero,
                            },
                            dealer,
                            powered,
                            card: card.and(self.playing),
                            side: self.live.side,
                            fully_blocked: aimed && fully_blocked,
                            unblockable,
                            turn: self.live.turn,
                            in_progress: self.live.in_progress,
                        },
                    );
                    self.after_hit(walking);
                }
                true => dying.push(receiver),
            }
        }
    }

    /// `CreatureCmd.Damage` on `target` for `amount`, unpowered and from no
    /// card, once the damage hooks worked it out.
    pub(super) fn hit_unpowered(
        &mut self,
        dealer: Option<CombatId>,
        target: CombatId,
        amount: V::Number,
        unblockable: bool,
    ) {
        let seat = self.seat();
        // `CreatureCmd.Damage` lands nothing from a dealer with no hit points
        // left, though its death is still to come.
        if dealer
            .and_then(|dealer| self.at(dealer))
            .is_some_and(|at| self.live.creatures[at].hp <= V::Health::zero())
        {
            return;
        }
        let damage = modified::<V>(seat, self.walk(), dealer, Some(target), None, amount, false);
        self.hit(dealer, target, damage, false, unblockable, None, false);
    }

    /// `HittableEnemies`: the opponents alive, less any a listener refuses a
    /// hit on.
    pub(super) fn hittable_opponents(&self) -> SmallVec<[CombatId; 8]> {
        self.hittable_among(sts2_core::Fight::opponents(self))
    }

    /// `HittableEnemies` for the side `of` fights on, whoever is acting.
    pub(super) fn hittable_opponents_of(&self, of: CombatId) -> SmallVec<[CombatId; 8]> {
        let Some(at) = self.at(of) else {
            return SmallVec::new();
        };
        let side = self.live.creatures[at].side;
        let opponents = self
            .live
            .creatures
            .iter()
            .filter(|creature| creature.side != side && creature.alive && creature.owner.is_none())
            .map(|creature| creature.id)
            .collect();
        self.hittable_among(opponents)
    }

    /// `creatures`, less any a listener refuses a hit on.
    fn hittable_among(&self, creatures: sts2_core::Opponents) -> SmallVec<[CombatId; 8]> {
        creatures
            .into_iter()
            .filter(|&creature| {
                !super::listen::any_decides::<V>(
                    self.walk(),
                    // Every opponent listed is alive.
                    sts2_core::DecisionHook::ShouldAllowHitting {
                        creature,
                        alive: true,
                    },
                    false,
                )
            })
            .collect()
    }

    /// What `PowerCmd.Apply` does that `ModifyAmount` does not.
    ///
    /// The instance half of the command: the hook a power runs before it
    /// attaches, and the tick the creature it lands on will pass over. A
    /// second stack of a power reaches neither, because the command hands one
    /// to `ModifyAmount` before it gets here.
    ///
    /// The instance as `BeforeApplied` left it, which is what attaches.
    fn apply_fresh(
        &mut self,
        carrier: super::live::Carrier<V>,
        power: V::Power,
        amount: PowerAmount<V::Number>,
        applier: Option<CombatId>,
        target: Option<CombatId>,
        copy: Option<V::PowerState>,
    ) -> V::PowerState {
        // `BeforeApplied`, which runs before the power attaches. What it asks
        // for lands first, so a creature carries those powers ahead of this
        // one.
        //
        // Not a `Stack`, because nothing carries it yet. The side and the
        // holder are the creature it is landing on, which is the owner it is
        // about to have.
        let mut state =
            copy.unwrap_or_else(|| <V::PowerState as sts2_core::PowerModel<V>>::new(power));
        let fight = super::live::Onlookers {
            creatures: &self.live.creatures,
            powers: &self.live.powers,
            kept: &self.live.kept,
            belts: Some(&*self.belts),
            attack: &self.live.attack_results,
        };
        let listener = sts2_core::EventPower {
            power,
            amount,
            owner: carrier.side,
            holder: carrier.id,
            applier,
            target,
            monster: carrier.monster,
            on_turn_start: sts2_core::PowerAmount::Counted(num_traits::zero()),
            state: &mut state,
            history: self.live.history(carrier.player),
            fight: &fight,
        };
        let mut asked = sts2_core::HookEffects::new();
        <V as sts2_core::Hooks>::power_event_hook(
            listener,
            sts2_core::EventHook::BeforeApplied { landing: amount },
            &mut asked,
        );
        for effect in asked {
            match effect {
                sts2_core::HookEffect::ApplyToOwner(power, amount) => {
                    self.land(carrier.id, power, amount, applier);
                }
                other => unimplemented!("a power asking for {other:?} as it lands"),
            }
        }
        // By id, because what landed can kill the creature, and a death takes
        // it out of the list.
        let Some(at) = self.at(carrier.id) else {
            return state;
        };

        // `SkipNextDurationTick`, which the next tick then passes over. The
        // game reads the power's type behind `&&`, so it asks only about one
        // landing on the player. A power that works its own type out is never
        // asked on the way onto a monster.
        let on_player = self.live.creatures[at].side == <V::CombatSide as SideKinds>::PLAYER;
        if on_player && <V::PowerState as sts2_core::PowerModel<V>>::debuff(&state) {
            let id = self.live.creatures[at].id;
            if !self.skipped.contains(&(id, power)) {
                self.skipped.push((id, power));
            }
        }
        state
    }

    /// `AfterCardEnteredCombat`, for a card that reached a combat pile from no
    /// pile at all.
    ///
    /// A card moving between piles is `AfterCardChangedPiles` and does not
    /// come here.
    /// `AddGeneratedCardToCombat` of one new copy into the draw pile at
    /// `CardPilePosition.Random`. Nothing once the fight is over.
    pub(super) fn add_to_draw_at_random(&mut self, card: V::Card, creator: Option<CombatId>) {
        self.add_at_random_here(card, Pile::Draw, creator);
    }

    /// `AddGeneratedCardToCombat` of one new copy into `pile` at
    /// `CardPilePosition.Random`: the place `Rng.Shuffle.NextInt(count + 1)`
    /// names, from the top, made by `creator`. Nothing once the fight is
    /// over.
    fn add_at_random_here(
        &mut self,
        card: V::Card,
        pile: Pile,
        creator: Option<CombatId>,
    ) -> Option<CardAt<V>> {
        let seat = self.seat();
        use sts2_core::rng::GameRng as _;

        if self.stopped() {
            return None;
        }
        let places: i32 = num_traits::cast(self.live.players[seat].cards.len(pile) + 1)
            .expect("a pile's places fit the RNG integer");
        let bound = sts2_core::rng::Bound::try_new(places).expect("a pile has a place");
        let at: usize = num_traits::cast(self.rngs.shuffle.next_int(bound))
            .expect("a draw below the bound is a place");
        let id = self.live.card_ids.mint();
        self.live.players[seat]
            .cards
            .insert(pile, at, CardInstance::new(CardIdent::new(card), id));
        let index = num_traits::NumCast::from(at)
            .unwrap_or_else(|| unimplemented!("a pile deeper than a fight can number"));
        let at = CardAt::new(id, pile, index);
        self.card_entered_combat(at, card, false);
        self.card_generated(at, card, creator);
        Some(at)
    }

    pub(super) fn card_entered_combat(&mut self, at: CardAt<V>, card: V::Card, clone: bool) {
        let seat = self.seat();
        let index = num_traits::ToPrimitive::to_usize(&at.index())
            .unwrap_or_else(|| unimplemented!("a pile deeper than a fight can number"));
        let afflicted = (self.live.players[seat].cards.idents(at.pile()).nth(index))
            .is_some_and(|ident| ident.affliction.is_some());
        let mut walking = super::listen::Walking::begin(
            self.live,
            self.belts,
            sts2_core::EventHook::CardEnteredCombat {
                at,
                card,
                player: seat.creature(),
                clone,
                afflicted,
            },
        );
        let mut rest: SmallVec<[(Listener<V>, HookEffect<V>); 4]> = SmallVec::new();
        while let Some((listener, effect)) =
            walking.as_mut().and_then(|walking| self.next_of(walking))
        {
            match (listener, effect) {
                // Written on the card that asked, wherever it stands.
                (Listener::Card { id }, HookEffect::ModifySelf { change, lasting }) => {
                    self.live.modify_self(seat, id, change, lasting)
                }
                (listener, effect) => rest.push((listener, effect)),
            }
        }
        for (listener, effect) in rest {
            self.as_listener(listener, |board| board.write_cards([effect]));
        }
    }

    /// `Hook.AfterCardGeneratedForCombat`, once a card made in the fight has
    /// landed at `at`, and what it asked for. `CardPileCmd.Add` has raised
    /// `AfterCardEnteredCombat` for it first.
    fn card_generated(&mut self, at: CardAt<V>, card: V::Card, creator: Option<CombatId>) {
        let seat = self.seat();
        self.live.history.generated(creator);
        // `IterateCombatHookListeners` hands a fight that is ending no
        // listeners, as for a card made after an orb it channelled won.
        if self.over() {
            return;
        }
        let mut walking = super::listen::Walking::begin(
            self.live,
            self.belts,
            sts2_core::EventHook::CardGeneratedForCombat { at, card, creator },
        );
        let mut rest: SmallVec<[(Listener<V>, HookEffect<V>); 4]> = SmallVec::new();
        while let Some((listener, effect)) =
            walking.as_mut().and_then(|walking| self.next_of(walking))
        {
            match (listener, effect) {
                // Written on the card that asked, wherever it stands.
                (Listener::Card { id }, HookEffect::ModifySelf { change, lasting }) => {
                    self.live.modify_self(seat, id, change, lasting)
                }
                // Written on the cards it names.
                (listener, effect @ HookEffect::ModifyCards { .. }) => {
                    self.as_listener(listener, |board| board.write_cards([effect]))
                }
                (listener, effect) => rest.push((listener, effect)),
            }
        }
        // A listener's block names no `CardPlay`, so the card being played
        // lends it no enchantment.
        let playing = self.playing.take();
        for (listener, effect) in rest {
            self.listener_asked(listener, effect);
        }
        self.playing = playing;
    }

    /// Who a card the acting creature makes is made by: the player, where
    /// the player acts. A monster's cards are made by nobody.
    fn creator(&self) -> Option<CombatId> {
        let seat = self.seat();
        let player = self.live.player(seat).id;
        (self.actor == player).then_some(player)
    }

    /// `CardPileCmd.AddGeneratedCardsToCombat` of `count` new copies into
    /// `pile`, made by `creator`.
    pub(super) fn generate(
        &mut self,
        card: CardIdent<V>,
        pile: Pile,
        count: usize,
        creator: Option<CombatId>,
    ) -> Vec<CardAt<V>> {
        // `IsInProgress`, which a fight already over fails.
        if self.stopped() {
            return Vec::new();
        }
        let mut added = Vec::with_capacity(count);
        for _ in 0..count {
            let at = self.put(card, pile);
            added.push(at);
            self.card_generated(at, card.card, creator);
        }
        added
    }

    /// [`Board::generate`] of copies that `writes` lands on before they
    /// enter the fight, as `SetToFreeThisTurn` does before
    /// `AddGeneratedCardsToCombat`. A card's own `AfterCardEnteredCombat`
    /// then writes after them.
    pub(super) fn generate_written(
        &mut self,
        card: CardIdent<V>,
        pile: Pile,
        count: usize,
        writes: &[(sts2_core::Change<V>, sts2_core::Lasting)],
    ) -> Vec<CardAt<V>> {
        self.ran_on();
        let creator = self.creator();
        if self.stopped() {
            return Vec::new();
        }
        let mut added = Vec::with_capacity(count);
        for _ in 0..count {
            let at = self.put_written(card, pile, writes);
            added.push(at);
            self.card_generated(at, card.card, creator);
        }
        added
    }

    /// `CardPileCmd.Add` of a new copy of `card` into `pile`, and the
    /// `AfterCardEnteredCombat` it raises as it lands.
    fn put(&mut self, card: CardIdent<V>, pile: Pile) -> CardAt<V> {
        self.put_written(card, pile, &[])
    }

    /// [`Board::put`], with `writes` on the copy before it lands.
    fn put_written(
        &mut self,
        card: CardIdent<V>,
        pile: Pile,
        writes: &[(sts2_core::Change<V>, sts2_core::Lasting)],
    ) -> CardAt<V> {
        let seat = self.seat();
        let id = self.live.card_ids.mint();
        // A full hand sends the card to the discard pile instead.
        let full = pile == Pile::Hand
            && self.live.players[seat].cards.len(Pile::Hand) >= super::pile::MAX_CARDS_IN_HAND;
        let pile = if full { Pile::Discard } else { pile };
        let index = num_traits::NumCast::from(self.live.players[seat].cards.len(pile))
            .unwrap_or_else(|| unimplemented!("a pile deeper than a fight can number"));
        self.live.players[seat]
            .cards
            .push(pile, CardInstance::new(card, id));
        let at = CardAt::new(id, pile, index);
        for &(change, lasting) in writes {
            sts2_core::Fight::modify_card(self, at, change, lasting);
        }
        self.card_entered_combat(at, card.card, false);
        at
    }

    /// Carry out what the card-writing hooks asked for.
    ///
    /// Replays and keywords, which are the effects a fight carries out
    /// against its own cards rather than against a creature.
    pub(super) fn write_cards(
        &mut self,
        asked: impl IntoIterator<Item = sts2_core::HookEffect<V>>,
    ) {
        for effect in asked {
            match effect {
                sts2_core::HookEffect::AddReplays { to, amount } => {
                    self.write_on(to, |held| held.replay(amount))
                }
                sts2_core::HookEffect::AddKeyword { to, keyword } => self.write_on(to, |held| {
                    held.modify(
                        sts2_core::Change::AddKeyword(keyword),
                        sts2_core::Lasting::ThisFight,
                    )
                }),
                // `CardCmd.Afflict`, which does nothing to a card in a fight
                // that is ending. `Hook.ShouldAfflict` has no model answering
                // it in this build.
                sts2_core::HookEffect::Afflict { to, affliction } => {
                    if self.over() {
                        continue;
                    }
                    let afflicted = std::cell::Cell::new(0);
                    self.write_on(to, |held| {
                        if afflict::<V>(held, affliction) {
                            afflicted.set(afflicted.get() + 1);
                        }
                    });
                    for _ in 0..afflicted.get() {
                        self.live
                            .history
                            .afflicted(affliction, self.seat().creature());
                    }
                }
                sts2_core::HookEffect::ClearAffliction(to) => {
                    self.write_on(to, |held| held.ident.affliction = None)
                }
                sts2_core::HookEffect::ModifyCards { to, change } => self.write_on(to, |held| {
                    held.modify(change, sts2_core::Lasting::ThisFight)
                }),
                effect => unimplemented!("a power asking for {effect:?} as it writes on cards"),
            }
        }
    }

    /// Write on each card `to` names.
    fn write_on(&mut self, to: sts2_core::Writing<V>, write: impl Fn(&mut Face<V>)) {
        let seat = self.seat();
        match to {
            sts2_core::Writing::One(at) => {
                let index = self.standing(at);
                self.live.players[seat]
                    .cards
                    .write(at.pile(), index, |held| {
                        write(held);
                        true
                    });
            }
            // `PlayerCombatState.AllCards`, which the exhaust and play piles
            // are among. A blade taken out of the fight is written on and
            // never played again.
            sts2_core::Writing::EveryCopyOf(card) => self.live.players[seat]
                .cards
                .write_all_cards(|held| held.ident.card == card, write),
            sts2_core::Writing::EveryTagged(tag) => self.live.players[seat]
                .cards
                .write_all_cards(|held| held.ident.card.tagged(tag), write),
            sts2_core::Writing::Every => self.live.players[seat]
                .cards
                .write_all_cards(|_| true, write),
            sts2_core::Writing::EveryOfKind(kind) => self.live.players[seat]
                .cards
                .write_all_cards(|held| V::card_kind(held.ident.card) == Some(kind), write),
            sts2_core::Writing::EveryAfflictedWith(affliction) => self.live.players[seat]
                .cards
                .write_all_cards(|held| held.ident.affliction == Some(affliction), write),
            sts2_core::Writing::Named(id) => {
                self.live.players[seat].cards.write_named(id, |held| {
                    write(held);
                    true
                })
            }
        }
    }

    /// `LoseHpInternal`: what `creature` lost, what went past its health,
    /// and whether this took it from standing to none.
    fn lose_hp(&mut self, creature: CombatId, amount: V::Number) -> (V::Number, V::Number, bool) {
        let zero = V::Number::zero();
        let Some(at) = self.at(creature) else {
            return (zero, zero, false);
        };
        let hp = self.live.creatures[at].hp;
        let health_zero = V::Health::zero();
        let killed = hp > health_zero && amount >= V::Number::from(hp);
        // A loss below nothing heals, past the creature's maximum too, where
        // the build does not floor it.
        let amount = match V::hp_loss_floors_at_nothing() {
            true => amount.max(zero),
            false => amount,
        };
        self.live.creatures[at].hp = (hp - V::Health::from(amount)).max(health_zero);
        let lost = V::Number::from(hp - self.live.creatures[at].hp);
        let overkill = match killed {
            true => (amount - V::Number::from(hp)).max(zero),
            false => zero,
        };
        (lost, overkill, killed)
    }

    /// Resolve a death, then end the fight if it is ending.
    ///
    /// A creature a power keeps in the fight is still counted, and still
    /// dead, so keeping one does not hold the fight open.
    #[tracing::instrument(level = "trace", skip_all)]
    fn kill(&mut self, at: usize) {
        self.die(at, false);
        self.end_if_ending();
    }

    /// `CheckWinCondition` once a creature leaves: a fight with no primary
    /// enemy left ends, won.
    pub(super) fn end_if_ending(&mut self) {
        if !self.ending() {
            return;
        }

        // `EndCombat`: everything left on the enemy side is secondary. It
        // dies through the same hooks, but cannot keep the fight open.
        for at in (0..self.live.creatures.len()).rev() {
            if self.live.creatures[at].side == <V::CombatSide as SideKinds>::ENEMY
                && self.live.creatures[at].alive
            {
                self.live.creatures[at].hp = V::Health::zero();
                self.die(at, false);
            }
        }
        // `CheckWinCondition` processes a pending loss first, so a fight the
        // player already lost stays lost.
        if self.ended == Some(super::Ended::Lost) || self.live.lost {
            return;
        }
        self.ended = Some(super::Ended::Won);
        self.live.won = true;
    }

    /// How this effect ended the fight, as `CheckWinCondition` finds it once
    /// the effect is over.
    pub(super) fn ended(&self) -> Option<super::Ended> {
        match self.ended {
            Some(super::Ended::Won) if !self.ending() => None,
            ended => ended,
        }
    }

    /// `IsOverOrEnding`: a loss waits, or no primary enemy stands.
    fn over(&self) -> bool {
        self.ended == Some(super::Ended::Lost) || self.live.lost || self.ending()
    }

    /// A command stops on: a loss waits, the win this effect saw still
    /// holds, or an earlier effect won the fight. Cheaper than
    /// [`Board::over`] where nothing has ended.
    fn stopped(&self) -> bool {
        match self.ended {
            // A fight an earlier effect won or lost stays over.
            None => self.live.won || self.live.lost,
            Some(super::Ended::Lost) => true,
            Some(super::Ended::Won) => self.ending(),
        }
    }

    /// `CombatManager.IsEnding` over this board.
    fn ending(&self) -> bool {
        ending::<V>(self.walk())
    }

    /// The fight and the belt, as a walk reads them.
    pub(super) fn walk(&self) -> Walk<'_, V> {
        Walk {
            live: self.live,
            belts: self.belts,
        }
    }

    /// Where `id`, whose death is being resolved, stands now.
    ///
    /// # Panics
    ///
    /// A creature its own death's hooks took out of the fight.
    fn dying_at(&self, id: CombatId) -> usize {
        self.at(id)
            .unwrap_or_else(|| unimplemented!("{id:?} leaving the fight inside its own death"))
    }

    /// `KillWithoutCheckingWinCondition`, including a power's revival hook.
    ///
    /// A player dying takes the run with it. `LoseCombat` files the loss, and
    /// the next win check turns that into the end of the fight.
    ///
    /// `HandlePlayerDeath` empties the piles and the pools. It is the other
    /// branch of the same question, for a player whose teammates still stand,
    /// and a run with one player never reaches it.
    #[tracing::instrument(level = "trace", skip_all)]
    fn die(&mut self, at: usize, force: bool) {
        let owner = self.live.creatures[at].side;
        let id = self.live.creatures[at].id;
        // A player's death is its own seat's, and anything else's is the
        // actor's.
        let seat = self.live.seat_of(id).unwrap_or_else(|| self.actor_seat());
        // `Hook.BeforeDeath`, once its hit points are gone and before
        // anything decides whether it dies.
        let mut walking = super::listen::Walking::begin(
            self.live,
            self.belts,
            sts2_core::EventHook::BeforeDeath { creature: id },
        );
        while let Some((listener, effect)) =
            walking.as_mut().and_then(|walking| self.next_of(walking))
        {
            match effect {
                HookEffect::ReturnCard(card) => self
                    .live
                    .extra_rewards
                    .push(super::live::ExtraReward::Returned(card)),
                // `CombatRoom.AddExtraReward` of a `GoldReward`.
                HookEffect::AddGoldReward(amount) => self
                    .live
                    .extra_rewards
                    .push(super::live::ExtraReward::Gold(amount)),
                effect => unimplemented!("{listener:?} asking for {effect:?} before a death"),
            }
        }
        // `Hook.ShouldDie`, then `Hook.ShouldDieLate`, and the model that
        // kept the creature alive. A forced death asks neither.
        let preventer = (!force)
            .then(|| {
                super::listen::first_deciding::<V>(
                    self.walk(),
                    sts2_core::DecisionHook::ShouldDie { creature: id },
                    false,
                )
                .or_else(|| {
                    super::listen::first_deciding::<V>(
                        self.walk(),
                        sts2_core::DecisionHook::ShouldDieLate { creature: id },
                        false,
                    )
                })
            })
            .flatten();
        if let Some(preventer) = preventer {
            self.death_prevented(at, preventer);
            return;
        }
        self.live.creatures[at].alive = false;
        // A pet dies the way a monster does, and whether it stays is what its
        // powers say. Only a player's death can lose the fight.
        let pet = self.live.creatures[at].owner.is_some();
        if owner != <V::CombatSide as SideKinds>::ENEMY && !pet {
            let standing = self.live.creatures.iter().any(|creature| {
                creature.side == <V::CombatSide as SideKinds>::PLAYER
                    && creature.owner.is_none()
                    && creature.alive
            });
            if let Some(effect) = self.after_death(id, owner, false).first() {
                unimplemented!("a power asking for {effect:?} over a player's death")
            }
            let at = self.dying_at(id);
            self.strip_powers(at);
            // `OrbQueue.Clear`, which leaves no orb and no capacity.
            self.live.players[seat].orbs = None;
            // `Kill(player.Osty, force)` while Osty stands.
            let osty = osty(&self.live.creatures, seat)
                .filter(|osty| osty.alive)
                .map(|osty| osty.id);
            if let Some(osty) = osty {
                self.kill_outright(osty, force);
            }
            // `DeactivateHooks`: the player's powers, belts, potions and
            // cards hear nothing after.
            self.live.players[seat].deaf = true;
            self.belts[seat].deafen();
            self.live.players[seat].potions.deafen();
            self.live.players[seat].cards.deafen();
            if standing {
                self.died_beside_others(seat);
                return;
            }
            self.ended = Some(super::Ended::Lost);
            self.live.lost = true;
            // `CreatureCmd.Kill` readies the turn a player dies on to end.
            // A turn being set up ends once the setup is done, and goes to
            // its end phase. A card or potion the player is running holds
            // the end until it finishes, and the run is over by then, so a
            // death in the play phase stays there.
            type Phase<V> = <V as sts2_core::ContentVersion>::PlayerTurnPhase;
            if self.live.side == <V::CombatSide as SideKinds>::PLAYER
                && self.live.players[seat].phase != <Phase<V> as sts2_core::TurnPhaseKinds>::PLAY
            {
                self.live.players[seat].phase = <Phase<V> as sts2_core::TurnPhaseKinds>::END;
            }
            return;
        }

        // `ShouldCreatureBeRemovedFromCombatAfterDeath`, which the game asks
        // before `AfterDeath`.
        let kept = super::listen::any_decides::<V>(
            self.walk(),
            sts2_core::DecisionHook::ShouldRemoveCreatureAfterDeath { creature: id },
            false,
        );
        let after_death = self.after_death(id, owner, false);
        // `AfterDeath`'s listeners can take other creatures out, which moves
        // where this one stands.
        let at = self.dying_at(id);
        if kept {
            let ascension = self.live.ascension;
            for effect in after_death {
                if let sts2_core::HookEffect::SetMaxAndCurrentHp(hp) = effect {
                    let hp = num_traits::cast(hp)
                        .unwrap_or_else(|| unimplemented!("{hp:?} hit points, which is no health"));
                    sts2_core::Fight::set_max_and_current_hp(self, id, hp);
                    continue;
                }
                let held = self
                    .live
                    .machines
                    .iter_mut()
                    .find(|held| held.creature == id)
                    .expect("a monster kept in the fight has a move graph");
                match effect {
                    // A state the power built, whose follow-up is the move
                    // the graph is standing on. That move is what comes back.
                    sts2_core::HookEffect::InstallMove(next) => {
                        if V::can_leave_current_move(held.monster, &held.machine) {
                            held.machine.interpose();
                            self.live.creatures[at].next_move = Some(next.into());
                        }
                    }
                    // A state the graph already holds, which walks on by
                    // itself and so has nothing to come back to.
                    sts2_core::HookEffect::EnterMove { to, force } => {
                        let monster = held.monster;
                        // A move the graph cannot leave yet is left standing,
                        // unless the command is forced. That is its own guard,
                        // and not a failure this has to answer for.
                        if let Ok(next) =
                            V::set_move_immediate(monster, ascension, to, force, &mut held.machine)
                        {
                            self.live.creatures[at].next_move = Some((&next).into());
                        }
                    }
                    other => unimplemented!("a power asking for {other:?} over a death"),
                }
            }
            self.strip_powers(at);
            return;
        }

        let position = self.live.creatures[..at]
            .iter()
            .filter(|creature| creature.side == owner)
            .count();
        // `SlotName`, which a monster added in its place takes.
        let slot = self
            .live
            .machines
            .iter()
            .find(|held| held.creature == id)
            .and_then(|held| held.slot);
        // Its move graph is finished, but the creature remains addressable
        // until its death commands finish. Monsters those commands add must
        // receive ids after its own rather than reuse the one being removed.
        self.leave_machine(id);
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
                        // `GenerateMoveStateMachine` opens on the move the
                        // spawn names, which a Wriggler's `StartStunned`
                        // makes its Spawned move.
                        let id = super::monster::summon_built::<V>(
                            self.live,
                            sts2_core::Spawned::opening_on(monster, opening.id),
                            sts2_core::MonsterModel::new(monster),
                            position + offset,
                            self.rngs.niche,
                        );
                        self.arrived(id, monster);
                        let summoned = self
                            .live
                            .creatures
                            .iter_mut()
                            .find(|creature| creature.id == id)
                            .expect("a summoned monster is in the fight");
                        summoned.next_move = Some(opening.into());
                        self.enemy_added(id);
                    }
                }
                // `CreatureCmd.Add` in the owner's slot, of a model the power
                // wrote on.
                sts2_core::HookEffect::SummonWritten {
                    monster,
                    written,
                    opens_on,
                } => {
                    let id = super::monster::summon_built::<V>(
                        self.live,
                        sts2_core::Spawned {
                            monster,
                            opens_on,
                            slot,
                            written: Some(written),
                        },
                        sts2_core::MonsterModel::written(monster, written),
                        position,
                        self.rngs.niche,
                    );
                    self.arrived(id, monster);
                    self.roll_joined(id);
                    self.enemy_added(id);
                }
                // `PowerCmd.Apply`, which the Possess powers hand back with.
                sts2_core::HookEffect::ApplyPower {
                    target,
                    power,
                    amount,
                    applier,
                } => self.land(target, power, amount, applier),
                sts2_core::HookEffect::PerformDeath(power) => {
                    V::perform_power_death(power, id, self)
                }
                other => unimplemented!("a dead power asking for {other:?}"),
            }
        }
        // Nothing that reads the fight after the death commands can see the
        // dead monster. The observation therefore omits it rather than
        // reporting it dead, and the numbers its powers counted go with it.
        self.live.creatures.retain(|creature| creature.id != id);
        self.live.powers.clear(id);
    }

    /// `HandlePlayerDeath` for a player who died while another stands, and
    /// the end of turn `CreatureCmd.Kill` asks for them. Their cards leave
    /// the fight, and their energy and stars go to nothing. On their side's
    /// turn they are ready to end it.
    ///
    /// # Panics
    ///
    /// A death inside the player's own card or potion, whose cards the game
    /// takes once the effect is over. A death that leaves every standing
    /// player ready, which ends the turn.
    fn died_beside_others(&mut self, seat: sts2_core::SeatIndex) {
        let player = &mut self.live.players[seat];
        if player.effect_depth > 0 {
            unimplemented!("a player dying inside their own card or potion beside a living one");
        }
        // `RemoveDeadPlayerCardsFromCombat`, which raises no hook.
        player.cards.clear();
        player.energy = num_traits::zero();
        player.stars = num_traits::zero();
        if self.live.side == <V::CombatSide as SideKinds>::PLAYER {
            player.ready = true;
            let live = &*self.live;
            if super::turn::seats(live).all(|other| live.players[other].ready) {
                unimplemented!("a player's death leaving every player ready to end the turn");
            }
        }
    }

    /// `RemoveAllPowersAfterDeath` on a creature that stays in the fight.
    ///
    /// Two questions and either one keeps a power: the power can keep itself,
    /// and any listener in the fight can keep it. A creature the fight drops
    /// takes its powers with it, so this is asked only of one that does not.
    #[tracing::instrument(level = "trace", skip_all)]
    fn strip_powers(&mut self, at: usize) {
        let creature = &self.live.creatures[at];
        let (carrier, id) = (creature.carrier(), creature.id);
        let kept: SmallVec<[bool; 16]> = self
            .live
            .powers
            .of(id)
            .map(|subject| {
                let itself = <V as sts2_core::Hooks>::decide_hook(
                    subject.listener(
                        carrier,
                        self.live.history(carrier.player),
                        &super::live::Onlookers {
                            creatures: &self.live.creatures,
                            powers: &self.live.powers,
                            kept: &self.live.kept,
                            belts: Some(&*self.belts),
                            attack: &self.live.attack_results,
                        },
                    ),
                    sts2_core::DecisionHook::ShouldRemoveSelfAfterOwnerDeath,
                )
                .is_decision_and(|remove| !remove);
                itself
                    || super::listen::any_decides::<V>(
                        self.walk(),
                        sts2_core::DecisionHook::ShouldRemovePowerOnDeath {
                            creature: id,
                            power: subject.power(),
                        },
                        false,
                    )
            })
            .collect();
        // `PowerModel.AfterRemoved` for each power that goes, which
        // `remove_power` asks.
        let going: SmallVec<[V::Power; 8]> = self
            .live
            .powers
            .of(id)
            .zip(kept)
            .filter(|(_, kept)| !kept)
            .map(|(held, _)| held.power())
            .collect();
        for power in going {
            self.remove_power_carried(id, power);
        }
    }

    /// A death `preventer` kept from happening: `Hook.AfterDeath` with the
    /// removal prevented, and then `AfterPreventingDeath`, asked of
    /// `preventer` alone.
    ///
    /// # Panics
    ///
    /// A model other than a relic or a potion keeping a creature alive, or
    /// anything asked for that nothing here carries out.
    #[cold]
    #[inline(never)]
    fn death_prevented(&mut self, at: usize, preventer: sts2_core::ModelId<V>) {
        let creature = &self.live.creatures[at];
        let (id, side) = (creature.id, creature.side);
        // The relic or potion is the dying player's own.
        let seat = self
            .live
            .seat_of(id)
            .unwrap_or_else(|| unimplemented!("{preventer:?} keeping {id:?}, no player, alive"));
        let max_hp: V::Number =
            num_traits::cast(creature.max_hp).expect("maximum health fits a game number");
        if let Some(effect) = self.after_death(id, side, true).first() {
            unimplemented!("a power asking for {effect:?} over a death kept from happening")
        }
        let hook = sts2_core::EventHook::DeathPrevented { max_hp };
        let asked = match preventer {
            sts2_core::ModelId::Relic(relic) => {
                super::listen::ask_relic(&*self.live, seat, self.belts, relic, hook)
            }
            sts2_core::ModelId::Potion(potion) => {
                let mut asked = sts2_core::HookEffects::new();
                V::potion_event_hook(potion, hook, &mut asked);
                asked
            }
            _ => unimplemented!("{preventer:?} keeping a creature alive"),
        };
        for effect in asked {
            match (preventer, effect) {
                (sts2_core::ModelId::Relic(_), HookEffect::Heal(amount)) => {
                    sts2_core::Fight::heal(self, id, amount)
                }
                (sts2_core::ModelId::Potion(potion), HookEffect::UseOnSaved) => {
                    self.use_on_saved(id, potion)
                }
                (_, effect) => {
                    unimplemented!("{preventer:?} asking for {effect:?} as it keeps {id:?} alive")
                }
            }
        }
    }

    /// `OnUseWrapper` on `saved`, for the `potion` that kept it alive:
    /// `RemoveBeforeUse`, `BeforePotionUsed`, the potion's own effect,
    /// `AfterPotionUsed` and `CheckForEmptyHand`.
    ///
    /// # Panics
    ///
    /// A listener asking for anything around the potion, or a potion that
    /// stops to ask a question.
    #[cold]
    #[inline(never)]
    fn use_on_saved(&mut self, saved: CombatId, potion: V::Potion) {
        let seat = self
            .live
            .seat_of(saved)
            .expect("a potion saves the player whose belt holds it");
        let at = self.live.players[seat]
            .potions
            .held()
            .iter()
            .position(|&held| held == potion)
            .expect("the potion that kept a creature alive is in the belt");
        self.live.players[seat].potions.spend(at);
        let hook = sts2_core::EventHook::PotionUsing {
            player: seat.creature(),
            target: Some(saved),
        };
        if let Some((listener, effect)) =
            super::listen::first_answer::<V>(seat, self.live, self.belts, hook)
        {
            unimplemented!("{listener:?} asking for {effect:?} before a potion")
        }
        let actor = std::mem::replace(&mut self.actor, saved);
        let played = V::on_use::<BoardKind<V>>(potion, self);
        self.actor = actor;
        if !matches!(played, sts2_core::Played::Done) {
            unimplemented!("{potion:?} stopping to ask as it keeps {saved:?} alive");
        }
        if self.ended().is_some() {
            return;
        }
        let hook = sts2_core::EventHook::PotionUsed {
            player: seat.creature(),
            in_fight: true,
            holding_potions: !self.live.players[seat].potions.is_empty(),
        };
        let walking = super::listen::Walking::begin(self.live, self.belts, hook);
        self.carry_out_walk(walking);
        if self.ended().is_some() || self.live.players[seat].cards.len(Pile::Hand) != 0 {
            return;
        }
        let hook = sts2_core::EventHook::HandEmptied {
            player: self.live.player(seat).id,
            phase: self.live.players[seat].phase,
        };
        let walking = super::listen::Walking::begin(self.live, self.belts, hook);
        self.carry_out_walk(walking);
    }

    /// `Hook.AfterDeath` for `creature`, which fought on `side`.
    ///
    /// The belt answers ahead of every enemy's powers, and what it asks for is
    /// carried out here. `PlayerCmd.GainEnergy` and `CardPileCmd.Draw` both
    /// return early while `CombatManager.IsEnding`, so a belt paid for the
    /// last kill gets nothing.
    ///
    /// What the dead creature's own powers ask for comes back. What it means
    /// depends on whether the creature stays in the fight.
    #[tracing::instrument(level = "trace", skip_all)]
    fn after_death(
        &mut self,
        creature: CombatId,
        side: V::CombatSide,
        removal_prevented: bool,
    ) -> sts2_core::HookEffects<V> {
        let monster = self
            .at(creature)
            .and_then(|at| match self.live.creatures[at].model {
                Some(sts2_core::ModelId::Monster(monster)) => Some(monster),
                _ => None,
            });
        let mut walking = super::listen::Walking::begin(
            self.live,
            self.belts,
            sts2_core::EventHook::AfterDeath {
                creature,
                side,
                monster,
                removal_prevented,
            },
        );
        let mut own = sts2_core::HookEffects::new();
        // The listener whose question a play set aside, which asks nothing
        // more until it is answered.
        let mut paused = None;
        while let Some((listener, effect)) =
            walking.as_mut().and_then(|walking| self.next_of(walking))
        {
            if paused == Some(listener) {
                unimplemented!("{listener:?} asking for {effect:?} behind its own question")
            }
            // What a listener asks is for its owner, and a monster's for the
            // actor.
            let seat =
                super::listen::owner(self.live, listener).unwrap_or_else(|| self.actor_seat());
            // Each listener runs under a pause boundary of its own, with a
            // context that blocks on a question. A question it asks pauses it
            // alone, and the caller runs on. What it deferred waits with the
            // question.
            let fresh = self
                .live
                .asking
                .is_none()
                .then(|| self.live.deferred.open_fresh());
            let choice = match fresh {
                Some(_) => std::mem::replace(&mut self.choice, Choice::Blocking),
                None => self.choice,
            };
            match (listener, effect) {
                (
                    Listener::Relic { .. },
                    sts2_core::HookEffect::GainEnergy(_) | sts2_core::HookEffect::Draw(_),
                ) if self.ending() => {}
                (Listener::Relic { seat, slot }, effect) => self.relic_asked(seat, slot, effect),
                // `PowerCmd.Remove(this)`, from a power that goes with the
                // creature that applied it.
                (Listener::Power { owner, power, .. }, HookEffect::Remove) if owner != creature => {
                    self.remove_power_carried(owner, power)
                }
                (Listener::Power { owner, .. }, effect) if owner == creature => own.push(effect),
                // `PowerCmd.Apply` on the power's owner, from itself.
                (Listener::Power { owner, .. }, HookEffect::ApplyToOwner(power, amount)) => {
                    self.land(owner, power, amount, Some(owner));
                }
                // `CreatureCmd.Stun` on the power's owner.
                (Listener::Power { owner, .. }, HookEffect::StunOwner { stun, then }) => {
                    self.stun_monster(owner, stun, then)
                }
                // `CreatureCmd.GainBlock`, unpowered, on the power's owner, from
                // no play: a death inside a card's play does not make the card
                // the block's source.
                (Listener::Power { owner, .. }, HookEffect::GainBlock(amount)) => {
                    self.gain_unpowered(owner, amount)
                }
                (Listener::Card { id }, HookEffect::ModifySelf { change, lasting }) => {
                    self.live.modify_self(seat, id, change, lasting)
                }
                (Listener::Monster { creature, .. }, HookEffect::EnterMoveFrom { from, to }) => {
                    self.enter_move_from(creature, from, to)
                }
                (Listener::Power { owner, power, .. }, HookEffect::ForgetCreature(gone)) => self
                    .live
                    .kept_mut(owner, power)
                    .creatures
                    .retain(|kept| *kept != gone),
                (listener, effect) => {
                    unimplemented!("{listener:?} asking for {effect:?} over another's death")
                }
            }
            self.choice = choice;
            if let Some(fresh) = fresh {
                match self.live.asking.is_some() {
                    true => {
                        let mut task = self.live.set_aside(seat, fresh);
                        // `CardSelectCmd` lists the pile as it asks. The play
                        // can change the pile before the question comes up,
                        // and the list stays as it was. A build that signals
                        // the choice before it lists reads the pile once the
                        // listener's queued action starts. Inside a play that
                        // waits for the play's own action. Outside one the
                        // queue runs it at once, so the list is read now.
                        let listed_now = !V::signals_choice_before_listing()
                            || !matches!(choice, Choice::Blocking);
                        if let super::live::Asked::Pile { pile, selection } = task.asking.question
                            && listed_now
                        {
                            let cards = self
                                .live
                                .offered(seat, pile)
                                .into_iter()
                                .map(|at| {
                                    let held = self.live.players[seat]
                                        .cards
                                        .get(pile, at)
                                        .expect("an offered card stands in its pile");
                                    let at = num_traits::NumCast::from(at).unwrap_or_else(|| {
                                        unimplemented!("a pile deeper than a fight can number")
                                    });
                                    CardAt::new(held.id, pile, at)
                                })
                                .collect();
                            task.asking.question = super::live::Asked::Among { cards, selection };
                        }
                        self.live.queues.enqueue_hook(seat, task);
                        paused = Some(listener);
                    }
                    false => self.live.deferred.keep_fresh(fresh),
                }
            }
        }
        own
    }

    /// `CreatureCmd.Escape` on `id`: its powers go with no hook, and it leaves
    /// the fight without dying.
    pub(super) fn escape_creature(&mut self, id: CombatId) {
        let Some(at) = self.at(id) else { return };
        if !self.live.creatures[at].alive {
            return;
        }
        // `RemoveAllPowersInternalExcept`, which asks no power anything.
        self.live.powers.clear(id);
        self.leave_machine(id);
        self.live.creatures.escape(id);
        self.end_if_ending();
    }

    /// `SetMoveImmediate(to)` on `creature`'s graph where it advertises
    /// `from`, and where the graph can leave the move it stands on.
    fn enter_move_from(&mut self, creature: CombatId, from: V::MoveId, to: V::MoveId) {
        let Some(at) = self.at(creature) else { return };
        if self.live.creatures[at]
            .next_move
            .as_ref()
            .map(|next| next.id)
            != Some(from)
        {
            return;
        }
        let ascension = self.live.ascension;
        let Some(held) = self
            .live
            .machines
            .iter_mut()
            .find(|held| held.creature == creature)
        else {
            unreachable!("a monster in the fight has its graph")
        };
        if let Ok(next) =
            V::set_move_immediate(held.monster, ascension, to, false, &mut held.machine)
        {
            self.live.creatures[at].next_move = Some((&next).into());
        }
    }

    /// `ModifyHpLost` in the phases after a hit found who takes it, then
    /// `AfterModifyingHpLostAfterOsty` for each listener that changed it.
    ///
    /// Only a listener to the second needs to know who changed the first. So
    /// the phases are asked again one listener at a time, and only when one
    /// listens. `Hook.ModifyHpLost` counts a listener as changing the number
    /// when its whole part moves.
    fn hp_lost_after_osty(
        &mut self,
        target: CombatId,
        amount: V::Number,
        from: LossFrom,
    ) -> V::Number {
        let lost = after_osty::<V>(self.walk(), target, amount, from);
        if !super::listen::heard::<V>(
            self.live,
            self.belts,
            sts2_core::ModelHook::AfterModifyingHpLostAfterOsty,
        ) {
            return lost;
        }
        let exact = exact::<V>;
        let mut number = exact(amount);
        let mut modifiers: SmallVec<[(CombatId, V::Power); 2]> = SmallVec::new();
        for phase in [
            ModifyHook::HpLostAfterOsty {
                target,
                in_fight: true,
            },
            ModifyHook::HpLostAfterOstyLate {
                target,
                dealer: from.dealer,
                powered: from.powered,
            },
        ] {
            super::listen::each::<V>(
                self.walk(),
                sts2_core::Overrides::of(phase.model_hook()),
                |listener, _| {
                    let before = number;
                    number = match <V as sts2_core::Hooks>::modify_hook(listener, phase) {
                        Modify::Nothing => number,
                        Modify::Added(more) => number + exact(more),
                        Modify::Scaled(by) => number * by.exact(),
                        Modify::AtMost(most) => number.min(exact(most)),
                        Modify::Replaced(with) => exact(with),
                        Modify::RaisedTo { floor, from } => {
                            raised(number, exact(floor), exact(from))
                        }
                        Modify::Less(less) => (number - exact(less)).max(Decimal::ZERO),
                        Modify::AtMostUnlessNothing(most) => match number.is_zero() {
                            true => number,
                            false => number.min(exact(most)),
                        },
                    };
                    if before.trunc() != number.trunc() {
                        match listener {
                            sts2_core::Listener::Power { holder, power, .. } => {
                                modifiers.push((holder, power))
                            }
                            // Every relic's `AfterModifyingHpLostAfterOsty`
                            // only flashes.
                            sts2_core::Listener::Relic { .. } => {}
                            _ => unimplemented!(
                                "a listener other than a power changing what a hit takes"
                            ),
                        }
                    }
                },
            );
        }
        // `Hook.AfterModifyingHpLostAfterOsty` over the modifiers, which does
        // nothing for one that does not override it.
        for (holder, power) in modifiers {
            if !sts2_core::PowerTable::overrides(power)
                .contains(sts2_core::ModelHook::AfterModifyingHpLostAfterOsty)
            {
                continue;
            }
            let Some(at) = self
                .live
                .powers
                .of(holder)
                .position(|held| held.power() == power)
            else {
                continue;
            };
            let mut asked = sts2_core::HookEffects::new();
            self.live.ask_power(
                Some(self.belts),
                holder,
                at,
                sts2_core::EventHook::HpLostModified,
                &mut asked,
            );
            for effect in asked {
                match effect {
                    HookEffect::Decrement => {
                        self.modify_amount(holder, power, -num_traits::one::<V::Number>())
                    }
                    effect => unimplemented!("a power asking for {effect:?} after a hit's loss"),
                }
            }
        }
        lost
    }

    /// `DoomPower.DoomKill`: each creature in `doomed` loses what hit points
    /// it has and dies, in order, then `AfterDiedToDoom`.
    ///
    /// Whether each death triggers Fatal is read once all of them are dead,
    /// as each listener reads it. Reattach answers yes only once every other
    /// segment is dead.
    ///
    /// # Panics
    ///
    /// A listener other than a relic to `AfterDiedToDoom`.
    pub(super) fn doom_kill(&mut self, doomed: &[CombatId]) {
        let seat = self.seat();
        let player = self.live.player(seat).id;
        let triggers = |board: &Self, creature: CombatId| {
            creature != player && sts2_core::Fight::death_triggers_fatal(board, creature)
        };
        let before: SmallVec<[bool; 4]> = doomed
            .iter()
            .map(|&creature| triggers(self, creature))
            .collect();
        for &creature in doomed {
            self.kill_outright(creature, false);
        }
        let fatal = doomed
            .iter()
            .zip(before)
            // The game's dead creature keeps the powers that answer. One the
            // fight drops here takes its powers with it, so its answer is
            // the one from before the deaths.
            .filter(|&(&creature, before)| match self.at(creature) {
                Some(_) => triggers(self, creature),
                None => before,
            })
            .count();
        let mut walking = super::listen::Walking::begin(
            self.live,
            self.belts,
            sts2_core::EventHook::DiedToDoom { fatal },
        );
        // Each listener runs under a pause boundary of its own, and nothing
        // waits for one that paused: it goes on as a queued action.
        let mut listening = super::live::Listening::new();
        while let Some((listener, effect)) =
            walking.as_mut().and_then(|walking| self.next_of(walking))
        {
            let boundary = listening.enter(self.live, listener);
            match listener {
                Listener::Relic { seat, slot } => self.relic_asked(seat, slot, effect),
                listener => unimplemented!("{listener:?} asking for {effect:?} after Doom kills"),
            }
            listening.leave(seat, self.live, boundary);
        }
        let seat = self.seat();
        listening.queue(seat, self.live);
    }

    /// `CreatureCmd.Kill`: the creature loses what hit points it has, and
    /// dies. A `force`d kill asks no hook whether it dies.
    pub(super) fn kill_outright(&mut self, creature: CombatId, force: bool) {
        let Some(at) = self.at(creature) else {
            return;
        };
        let hp = V::Number::from(self.live.creatures[at].hp);
        self.lose_hp(creature, hp);
        if hp > V::Number::zero() {
            self.current_hp_changed(creature, -hp);
        }
        if let Some(at) = self.at(creature) {
            self.die(at, force);
            self.end_if_ending();
        }
    }

    /// `CreatureCmd.SetCurrentHp`: `creature`'s hit points set to `hp`,
    /// capped at its maximum, with `AfterCurrentHpChanged` where they moved
    /// and a death where none are left.
    pub(super) fn set_current_hp(&mut self, creature: CombatId, hp: V::Number) {
        let Some(at) = self.at(creature) else {
            return;
        };
        let held = &mut self.live.creatures[at];
        let before = V::Number::from(held.hp);
        let set: V::Health = num_traits::cast(hp.max(V::Number::zero()))
            .unwrap_or_else(|| unimplemented!("setting hit points to {hp:?}"));
        held.hp = set.min(held.max_hp);
        held.alive = held.hp > V::Health::zero();
        let after = V::Number::from(held.hp);
        if after != before {
            self.current_hp_changed(creature, after - before);
        }
        if let Some(at) = self.at(creature)
            && !self.live.creatures[at].alive
        {
            self.kill(at);
        }
    }

    /// `HookEffect::SetEnemiesHp`: on the enemy `creature` names, or on each
    /// one that can be hit.
    pub(super) fn set_enemies_hp(&mut self, creature: Option<CombatId>, hp: V::Number) {
        let seat = self.seat();
        let enemies = match creature {
            Some(creature) => smallvec::smallvec![creature],
            None => self.hittable_opponents_of(self.live.player(seat).id),
        };
        for enemy in enemies {
            self.set_current_hp(enemy, hp);
        }
    }

    /// `Hook.AfterCreatureAddedToCombat` once the enemy `id` has joined the
    /// fight mid-way, which `CreatureCmd.Add` raises last.
    ///
    /// # Panics
    ///
    /// A listener asking for something no enemy's arrival carries out.
    fn enemy_added(&mut self, id: CombatId) {
        let hook = sts2_core::EventHook::CreatureAdded {
            creature: id,
            side: <V::CombatSide as SideKinds>::ENEMY,
            point: self.live.point,
        };
        let mut walking = super::listen::Walking::begin(self.live, self.belts, hook);
        while let Some((listener, effect)) =
            walking.as_mut().and_then(|walking| self.next_of(walking))
        {
            match effect {
                HookEffect::SetEnemiesHp { creature, hp } => self.set_enemies_hp(creature, hp),
                HookEffect::ApplyPower {
                    target,
                    power,
                    amount,
                    applier,
                } => self.land(target, power, amount, applier),
                effect => unimplemented!("{listener:?} asking for {effect:?} as an enemy arrives"),
            }
        }
    }

    /// `Hook.AfterCurrentHpChanged` for `creature`, whose hit points moved by
    /// `delta`, and what its listeners ask.
    fn current_hp_changed(&mut self, creature: CombatId, delta: V::Number) {
        let Some(at) = self.at(creature) else {
            return;
        };
        let held = &self.live.creatures[at];
        let monster = match held.model {
            Some(sts2_core::ModelId::Monster(monster)) => Some(monster),
            _ => None,
        };
        let pet_of = held.owner;
        let hook = sts2_core::EventHook::CurrentHpChanged {
            creature,
            monster,
            pet_of,
            delta,
            in_fight: true,
        };
        let walking = super::listen::Walking::begin(self.live, self.belts, hook);
        self.after_hit(walking);
    }

    /// The next answer of `walking`, asked of the fight this board writes on.
    pub(super) fn next_of(
        &mut self,
        walking: &mut super::listen::Walking<V>,
    ) -> Option<(Listener<V>, HookEffect<V>)> {
        walking.next(self.actor_seat(), self.live, self.belts)
    }

    /// What `walking`'s listeners ask for, each carried out as the walk
    /// reaches it: a relic's for the player, and the rest through the fight.
    pub(super) fn carry_out_walk(&mut self, mut walking: Option<super::listen::Walking<V>>) {
        while let Some((listener, effect)) =
            walking.as_mut().and_then(|walking| self.next_of(walking))
        {
            self.listener_asked(listener, effect);
        }
    }

    /// What `listener` asked for, carried out for its owner, through the
    /// fight.
    pub(super) fn listener_asked(&mut self, listener: Listener<V>, effect: HookEffect<V>) {
        match listener {
            Listener::Relic { seat, slot } => self.relic_asked(seat, slot, effect),
            listener => self.as_listener(listener, |board| {
                super::hook::apply::<V, _>(board, [effect]);
            }),
        }
    }

    /// `act` with this board acting for `listener`'s owner, whoever's
    /// command raised the hook. A model acts on its own `Owner`: a relic's
    /// or a card's player, or the creature that carries a power. A monster
    /// acts for itself, and a modifier for the board's actor.
    ///
    /// A player's listener reaches its own player. Any other keeps the
    /// player the board reached before.
    pub(super) fn as_listener<R>(
        &mut self,
        listener: Listener<V>,
        act: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let actor = match listener {
            Listener::Relic { seat, .. } => seat.creature(),
            Listener::Power { owner, .. } => owner,
            Listener::Card { .. } => super::listen::owner(self.live, listener)
                .map_or(self.actor, sts2_core::SeatIndex::creature),
            Listener::Monster { creature, .. } => creature,
            Listener::Modifier { .. } => self.actor,
        };
        let reaching = match self.live.seat_of(actor) {
            Some(_) => None,
            None => self.reaching.or_else(|| self.live.seat_of(self.actor)),
        };
        let actor = std::mem::replace(&mut self.actor, actor);
        let reaching = std::mem::replace(&mut self.reaching, reaching);
        let answer = act(self);
        self.actor = actor;
        self.reaching = reaching;
        answer
    }

    /// What the relic in `slot` asked for, carried out for its player. Its
    /// `CardPileCmd.Draw` runs the code after its `await` once the draw is
    /// over, behind a question the draw left.
    pub(super) fn relic_asked(
        &mut self,
        seat: sts2_core::SeatIndex,
        slot: u8,
        effect: HookEffect<V>,
    ) {
        let listener = Listener::Relic { seat, slot };
        let HookEffect::Draw(count) = effect else {
            self.as_listener(listener, |board| {
                super::hook::apply::<V, _>(board, [effect]);
            });
            return;
        };
        self.as_listener(listener, |board| sts2_core::Fight::draw(board, count));
        let relic = self.belts[seat][usize::from(slot)].relic;
        match self.pausable() && self.live.asking.is_some() {
            true => self
                .live
                .defer(super::live::Deferred::OwnCommandOver { seat, slot, relic }),
            false => super::relic::own_command_over::<V>(
                self.live.relic_owner(seat),
                &mut self.belts[seat],
                slot,
                relic,
            ),
        }
    }

    /// Carry out what the listeners ask for after a hit or a broken block,
    /// each as the walk reaches it. A power's lands on the creature carrying
    /// it.
    fn after_hit(&mut self, mut walking: Option<super::listen::Walking<V>>) {
        while let Some((listener, effect)) =
            walking.as_mut().and_then(|walking| self.next_of(walking))
        {
            // `CardPileCmd.Draw` and `CreatureCmd.Heal` for the relic's owner.
            if let (
                Listener::Relic { seat, slot },
                effect @ (HookEffect::Draw(_) | HookEffect::Heal(_)),
            ) = (listener, effect)
            {
                self.relic_asked(seat, slot, effect);
                continue;
            }
            // `PowerCmd.Apply` on the relic's owner, the player. Every relic
            // answering a hit gives it no `cardSource`, even inside a card's
            // play.
            if let (Listener::Relic { seat, .. }, HookEffect::ApplyToOwner(power, amount)) =
                (listener, effect)
            {
                let player = self.live.player(seat).id;
                let playing = self.playing.take();
                self.land(player, power, amount, Some(player));
                self.playing = playing;
                continue;
            }
            if let (
                Listener::Relic { .. },
                HookEffect::ApplyPower {
                    target,
                    power,
                    amount,
                    applier,
                },
            ) = (listener, effect)
            {
                let playing = self.playing.take();
                self.land(target, power, amount, applier);
                self.playing = playing;
                continue;
            }
            let Listener::Power {
                owner, power: this, ..
            } = listener
            else {
                unimplemented!("{listener:?} asking for {effect:?} after a hit")
            };
            match effect {
                HookEffect::RemoveFromOwner(power) => self.remove_power_carried(owner, power),
                // `PowerCmd.Remove(this)`.
                HookEffect::Remove => self.remove_power_carried(owner, this),
                HookEffect::AddToDrawAtRandom {
                    card,
                    count,
                    by_owner,
                } => {
                    let count: usize = num_traits::cast(count)
                        .unwrap_or_else(|| unimplemented!("{count:?} cards into the draw pile"));
                    for _ in 0..count {
                        self.add_to_draw_at_random(card, by_owner.then_some(owner));
                    }
                }
                HookEffect::KillOwner => self.kill_outright(owner, false),
                // `OstyCmd.Summon` for the player.
                HookEffect::Summon(amount) => sts2_core::Fight::summon_osty(self, amount),
                // `CreatureCmd.Damage` over `HittableEnemies`, from the
                // power's owner, unblockable and unpowered.
                HookEffect::DamageOpponentsUnblockable(amount) => {
                    // One `CreatureCmd.Damage` over them all.
                    let targets = self.hittable_opponents_of(owner);
                    self.damage_command(|board| {
                        for target in targets {
                            board.hit_unpowered(Some(owner), target, amount, true);
                        }
                    });
                }
                // The same, and blockable.
                HookEffect::DamageOpponents(amount) => {
                    // One `CreatureCmd.Damage` over them all.
                    let targets = self.hittable_opponents_of(owner);
                    self.damage_command(|board| {
                        for target in targets {
                            board.hit_unpowered(Some(owner), target, amount, false);
                        }
                    });
                }
                // `PowerCmd.Apply`, which every power answering a hit gives no
                // `cardSource`, even inside a card's play.
                HookEffect::ApplyPower {
                    target,
                    power,
                    amount,
                    applier,
                } => {
                    let playing = self.playing.take();
                    self.land(target, power, amount, applier);
                    self.playing = playing;
                }
                HookEffect::LoseMaxHpOf { target, amount } => {
                    if sts2_core::Fight::lose_max_hp(self, target, amount).is_some() {
                        unimplemented!("a question inside a power's maximum health loss");
                    }
                }
                // `CreatureCmd.Damage`, unpowered, from the power's owner. A
                // command of its own, so its results are not the attack's.
                HookEffect::Damage { target, amount } => {
                    let hit = resolve::<V>(
                        self.actor_seat(),
                        self.walk(),
                        None,
                        Effect::Attack {
                            dealer: owner,
                            target,
                            damage: Amount::Number(amount),
                            scaling: Scaling::Static,
                        },
                    );
                    match hit.effect {
                        Resolved::Attack {
                            dealer,
                            target,
                            damage,
                            powered,
                            card,
                        } => self.hit(Some(dealer), target, damage, powered, false, card, false),
                        effect => self.apply(ResolvedEffect { effect }),
                    }
                }
                HookEffect::StunOwner { stun, then } => self.stun_monster(owner, stun, then),
                effect => unimplemented!("a power asking for {effect:?} after a hit"),
            }
        }
    }
}

/// `amount` run through `phases` in order, which is how `Hook` modifies a
/// number. Each listener in a phase is handed what the one before it left. The
/// result is floored at zero, unlike [`hp_lost_by`]'s.
///
/// Inlined, so the hooks `phases` name are known where it is called, and a
/// number nothing listens to costs a lookup.
#[inline(always)]
pub(super) fn modified_by<V: Rules>(
    fight: Walk<'_, V>,
    amount: V::Number,
    phases: &[ModifyHook<V>],
) -> V::Number {
    match super::listen::heard_any::<V>(fight.live, fight.belts, hooks_of(phases)) {
        true => modified_by_listeners::<V>(fight, amount, phases),
        false => amount.max(V::Number::zero()),
    }
}

/// [`modified_by`] once something listens.
///
/// One walk asks every phase a listener overrides. An answer never reads the
/// number it is handed, so the phases need no walk each.
///
/// A run of shares multiplies together before it lands.
#[tracing::instrument(level = "trace", skip_all)]
fn modified_by_listeners<V: Rules>(
    fight: Walk<'_, V>,
    amount: V::Number,
    phases: &[ModifyHook<V>],
) -> V::Number {
    scaled_by::<V>(fight, amount, None, phases).max(V::Number::zero())
}

/// `Hook.ModifyHpLost` over `phases`, which floors nothing. A loss a cap takes
/// below nothing heals where the build's `LoseHpInternal` takes it so.
pub(super) fn hp_lost_by<V: Rules>(
    fight: Walk<'_, V>,
    amount: V::Number,
    phases: &[ModifyHook<V>],
) -> V::Number {
    scaled_by::<V>(fight, amount, None, phases)
}

/// A game number as the decimal the game works it out in.
pub(crate) fn exact<V: Rules>(number: V::Number) -> Decimal {
    Decimal::from(num_traits::ToPrimitive::to_i64(&number).expect("a game number fits a long"))
}

/// The whole part of `number`, which is `(int)` on a decimal.
pub(crate) fn whole<V: Rules>(number: Decimal) -> V::Number {
    num_traits::ToPrimitive::to_i64(&number.trunc())
        .and_then(num_traits::cast)
        .expect("a modified number is a game number")
}

/// [`modified_by`] over a number an enchantment scaled first, where `scale`
/// says so. The game holds the scaled number as a decimal until the
/// listeners are done.
///
/// A whole number stays one until something scales it, and the game's own
/// integers work out every addition before that.
fn scaled_by<V: Rules>(
    fight: Walk<'_, V>,
    amount: V::Number,
    scale: Option<Decimal>,
    phases: &[ModifyHook<V>],
) -> V::Number {
    scaled_by_signed::<V>(fight, amount, scale, phases, None).0
}

/// [`scaled_by`], and whether the decimal was above nothing before the floor.
#[inline(always)]
fn scaled_by_signed<V: Rules>(
    fight: Walk<'_, V>,
    amount: V::Number,
    scale: Option<Decimal>,
    phases: &[ModifyHook<V>],
    last: Option<(u8, Modify<V::Number>)>,
) -> (V::Number, bool) {
    let hooks = hooks_of(phases);
    // A number nothing listens to and nothing scales is the number itself.
    // Most energy costs are asked this way, once a card for every `legal`.
    if scale.is_none()
        && last.is_none()
        && !super::listen::heard_any::<V>(fight.live, fight.belts, hooks)
    {
        return (amount, amount > V::Number::zero());
    }
    // Its own walk rather than [`answers`], which keeps the walk inlined on
    // this, the hot path.
    let mut answers = Answers::new();
    // `IterateCombatHookListeners` hands a fight that is over no listeners,
    // so what an observation reads after the win is the number alone.
    if !(fight.live.won && phases.iter().all(ModifyHook::combat_listeners)) {
        super::listen::each::<V>(fight, hooks, |listener, overrides| {
            answer::<V>(listener, overrides, phases, &mut answers)
        });
    }
    answers.extend(last);
    answers.sort_by_key(|&(at, _)| at);
    let mut number = amount;
    let mut at = 0;
    if scale.is_none() {
        while let Some(&(_, answer)) = answers.get(at) {
            number = match answer {
                Modify::Nothing => number,
                Modify::Added(more) => number + more,
                Modify::AtMost(most) => number.min(most),
                Modify::Replaced(with) => with,
                Modify::RaisedTo { floor, from } => match number >= from && number < floor {
                    true => floor,
                    false => number,
                },
                Modify::Less(less) => (number - less).max(V::Number::zero()),
                Modify::AtMostUnlessNothing(most) => match number.is_zero() {
                    true => number,
                    false => number.min(most),
                },
                Modify::Scaled(_) => break,
            };
            at += 1;
        }
        if at == answers.len() {
            return (number, number > V::Number::zero());
        }
    }
    let number = exact::<V>(number);
    let number = scale.map_or(number, |scale| number * scale);
    let number = worked_out::<V>(number, &answers[at..]);
    (whole::<V>(number), number > Decimal::ZERO)
}

/// [`scaled_by`] before the floor, which is the decimal the game's hook
/// hands back. Never below nothing.
fn scaled_exactly<V: Rules>(
    fight: Walk<'_, V>,
    amount: Decimal,
    scale: Option<Decimal>,
    phases: &[ModifyHook<V>],
) -> Decimal {
    let mut answers = Answers::new();
    // `IterateCombatHookListeners` hands a fight that is over no listeners,
    // so what an observation reads after the win is the number alone.
    if !(fight.live.won && phases.iter().all(ModifyHook::combat_listeners)) {
        super::listen::each::<V>(fight, hooks_of(phases), |listener, overrides| {
            answer::<V>(listener, overrides, phases, &mut answers)
        });
    }
    answers.sort_by_key(|&(at, _)| at);
    let number = scale.map_or(amount, |scale| amount * scale);
    worked_out::<V>(number, &answers).max(Decimal::ZERO)
}

/// What the listeners answer for a hook's phases, each beside its phase. A
/// stable sort on the phase puts them in the order the game applies them:
/// phase by phase, and within a phase in the order the walk asked.
type Answers<N> = SmallVec<[(u8, Modify<N>); 8]>;

/// Every hook among `phases`, for one walk to ask them all.
fn hooks_of<V: Rules>(phases: &[ModifyHook<V>]) -> sts2_core::Overrides {
    phases
        .iter()
        .fold(sts2_core::Overrides::default(), |hooks, phase| {
            hooks.with(phase.model_hook())
        })
}

/// What `listener` answers for each of `phases` it overrides.
#[inline(always)]
fn answer<V: Rules>(
    listener: super::live::Heard<'_, V>,
    overrides: sts2_core::Overrides,
    phases: &[ModifyHook<V>],
    answers: &mut Answers<V::Number>,
) {
    for (at, phase) in (0u8..).zip(phases) {
        if overrides.contains(phase.model_hook()) {
            match <V as sts2_core::Hooks>::modify_hook(listener, *phase) {
                Modify::Nothing => {}
                answer => answers.push((at, answer)),
            }
        }
    }
}

/// `number` after each of `answers` in turn. Never below nothing.
fn worked_out<V: Rules>(mut number: Decimal, answers: &[(u8, Modify<V::Number>)]) -> Decimal {
    let exact = exact::<V>;
    for &(_, answer) in answers {
        number = match answer {
            Modify::Nothing => number,
            Modify::Scaled(by) => number * by.exact(),
            Modify::Added(more) => number + exact(more),
            Modify::AtMost(most) => number.min(exact(most)),
            Modify::Replaced(with) => exact(with),
            Modify::RaisedTo { floor, from } => raised(number, exact(floor), exact(from)),
            Modify::Less(less) => (number - exact(less)).max(Decimal::ZERO),
            Modify::AtMostUnlessNothing(most) => match number.is_zero() {
                true => number,
                false => number.min(exact(most)),
            },
        };
    }
    number
}

/// The most orb slots `OrbCmd.AddSlots` gives a player.
const MAX_ORB_SLOTS: u8 = 10;

/// `RaisedTo`: `floor` where `number` is at least `from` and below it.
fn raised(number: Decimal, floor: Decimal, from: Decimal) -> Decimal {
    match number >= from && number < floor {
        true => floor,
        false => number,
    }
}

/// The hit an hp loss comes from, as the late `ModifyHpLost` reads it.
#[derive(Clone, Copy)]
pub(super) struct LossFrom {
    pub(super) dealer: Option<CombatId>,
    pub(super) powered: bool,
}

/// `ModifyHpLost` in the phases after a hit found who takes it.
fn after_osty<V: Rules>(
    fight: Walk<'_, V>,
    target: CombatId,
    amount: V::Number,
    from: LossFrom,
) -> V::Number {
    hp_lost_by::<V>(
        fight,
        amount,
        &[
            ModifyHook::HpLostAfterOsty {
                target,
                in_fight: true,
            },
            ModifyHook::HpLostAfterOstyLate {
                target,
                dealer: from.dealer,
                powered: from.powered,
            },
        ],
    )
}

/// One hit after every listener in the fight has had its say, which is
/// `Hook.ModifyDamage`.
///
/// `dealer` is the creature swinging and `target` the one it hits. Each
/// listener reads whether either is its own.
///
/// The target is absent for a hit previewed against nobody, and the hook
/// runs anyway. `powered` is `IsPoweredAttack`, which each listener reads.
pub(crate) fn modified<V: Rules>(
    seat: sts2_core::SeatIndex,
    fight: Walk<'_, V>,
    dealer: Option<CombatId>,
    target: Option<CombatId>,
    card: Option<&CardInstance<V>>,
    damage: V::Number,
    powered: bool,
) -> V::Number {
    let (added, scale) = enchanted_hit::<V>(card, powered);
    let phases = hit_phases::<V>(seat, fight, dealer, target, card, powered);
    scaled_by::<V>(fight, damage + added, scale, &phases).max(V::Number::zero())
}

/// [`modified`] before the floor, which is the decimal `Hook.ModifyDamage`
/// hands back.
fn modified_exactly<V: Rules>(
    seat: sts2_core::SeatIndex,
    fight: Walk<'_, V>,
    dealer: Option<CombatId>,
    target: Option<CombatId>,
    card: Option<&CardInstance<V>>,
    damage: Decimal,
    powered: bool,
) -> Decimal {
    let (added, scale) = enchanted_hit::<V>(card, powered);
    let phases = hit_phases::<V>(seat, fight, dealer, target, card, powered);
    scaled_exactly::<V>(fight, damage + exact::<V>(added), scale, &phases)
}

/// What the dealing card's enchantment adds to a hit, and what it scales the
/// sum by. The enchantment has its say before any listener, and only when a
/// card deals the hit. Its scale stays a decimal until the listeners are done.
fn enchanted_hit<V: Rules>(
    card: Option<&CardInstance<V>>,
    powered: bool,
) -> (V::Number, Option<Decimal>) {
    let Some((enchanted, state)) = card.and_then(|held| {
        held.face
            .ident
            .enchantment
            .map(|enchanted| (enchanted, held.face.enchantment_state()))
    }) else {
        return (V::Number::zero(), None);
    };
    let added = enchanted_by::<V>(
        enchanted,
        &state,
        EnchantHook::DamageAdditive { powered },
        V::Number::zero(),
    );
    match <V as sts2_core::Hooks>::enchant_modify(
        enchanted,
        &state,
        EnchantHook::DamageMultiplicative { powered },
    ) {
        Modify::Nothing => (added, None),
        Modify::Scaled(by) => (added, Some(by.exact())),
        answer => unimplemented!("an enchantment scaling a hit with {answer:?}"),
    }
}

/// `amount` times what the card's enchantment scales a powered hit by, cut
/// to a whole number the way an observation casts a preview. Nothing for a
/// card whose enchantment scales nothing.
pub(crate) fn enchant_scaled<V: Rules>(
    held: &CardInstance<V>,
    amount: V::Number,
) -> Option<V::Number> {
    let (_, scale) = enchanted_hit::<V>(Some(held), true);
    let scaled = exact::<V>(amount) * scale?;
    Some(
        num_traits::NumCast::from(scaled.trunc())
            .unwrap_or_else(|| unimplemented!("an extra damage of {scaled}")),
    )
}

/// `Hook.ModifyDamage`'s three phases for one hit.
fn hit_phases<V: Rules>(
    seat: sts2_core::SeatIndex,
    fight: Walk<'_, V>,
    dealer: Option<CombatId>,
    target: Option<CombatId>,
    card: Option<&CardInstance<V>>,
    powered: bool,
) -> [ModifyHook<V>; 3] {
    // `cardSource` is the model as it stands now. An attack's own numbers
    // are the ones it was built with, but a listener reads the card live, as
    // after Dampen comes off in the middle of the hits.
    let card = card.map(|held| {
        let (playing, ident) = (fight.live.players[seat].cards.standing_ident(held.id))
            .unwrap_or((false, &held.face.ident));
        sts2_core::CardSource {
            id: held.id,
            card: ident.card,
            owner: seat.creature(),
            playing,
            upgraded: ident.upgrade > 0,
            enchanted: ident.enchantment.is_some(),
        }
    });
    [
        ModifyHook::DamageAdditive {
            target,
            dealer,
            card,
            powered,
        },
        ModifyHook::DamageMultiplicative {
            target,
            dealer,
            card,
            powered,
        },
        ModifyHook::DamageCap {
            target,
            dealer,
            powered,
        },
    ]
}

/// `amount` after an enchantment's `hook`, which the game asks before any
/// listener.
///
/// # Panics
///
/// An enchantment that does more to the number than add to it.
pub(super) fn enchanted_by<V: Rules>(
    enchanted: sts2_core::Enchanted<V>,
    state: &V::EnchantmentState,
    hook: EnchantHook,
    amount: V::Number,
) -> V::Number {
    match <V as sts2_core::Hooks>::enchant_modify(enchanted, state, hook) {
        Modify::Nothing => amount,
        Modify::Added(more) => amount + more,
        answer => unimplemented!("an enchantment answering {hook:?} with {answer:?}"),
    }
}

/// `PlayerCombatState.MaxEnergy`: the player's own, after every listener
/// has had its say.
pub(crate) fn max_energy<V: Rules>(
    fight: Walk<'_, V>,
    seat: sts2_core::SeatIndex,
    base: V::Count,
) -> V::Count {
    let player = seat.creature();
    let base: V::Number = num_traits::cast(base).expect("an energy fits a game number");
    let energy = modified_by::<V>(
        fight,
        base,
        &[ModifyHook::MaxEnergy {
            player,
            turn: fight.live.turn,
        }],
    );
    num_traits::cast(energy).expect("a max energy fits the build's count type")
}

/// One of an orb's numbers after every listener has had its say, which is
/// `OrbModel.ModifyOrbValue`. The player's orbs are the only ones.
pub(crate) fn orb_value<V: Rules>(
    fight: Walk<'_, V>,
    seat: sts2_core::SeatIndex,
    orb: V::Orb,
    base: V::Number,
) -> V::Number {
    let player = seat.creature();
    modified_by::<V>(fight, base, &[ModifyHook::OrbValue { player, orb }])
}

/// One block gain after every listener in the fight has had its say, which
/// is `Hook.ModifyBlock`.
///
/// The same hook whether the block is previewed or gained, as damage's is.
/// `powered` is `IsPoweredCardOrMonsterMoveBlock`, `moved` is
/// `IsCardOrMonsterMove`, and `card` is the card whose own play gives it. `play` is that play, while one is under way.
#[expect(
    clippy::too_many_arguments,
    reason = "the fields `Hook.ModifyBlock` hands its listeners"
)]
pub(super) fn blocked<V: Rules>(
    fight: Walk<'_, V>,
    recipient: CombatId,
    amount: V::Number,
    powered: bool,
    moved: bool,
    card: Option<V::Card>,
    play: Option<sts2_core::PlayAt>,
    source: Option<CardId<V>>,
) -> (V::Number, bool, SmallVec<[V::Relic; 1]>) {
    let player = source
        .and_then(|id| fight.live.card_seat(id))
        .map(sts2_core::SeatIndex::creature);
    let phases = [
        ModifyHook::BlockAdditive {
            target: recipient,
            powered,
            card,
            player,
        },
        ModifyHook::BlockMultiplicative {
            target: recipient,
            powered,
            moved,
            card,
            player,
            play,
            source,
        },
    ];
    // `MultiplayerScalingModel`, the last listener the walk asks, on an
    // enemy's powered block.
    let scaled = match fight.live.scaling.block() {
        Some(by)
            if powered
                && fight.live.creatures.iter().any(|creature| {
                    creature.id == recipient && creature.side == <V::CombatSide as SideKinds>::ENEMY
                }) =>
        {
            let by = sts2_core::Fixed::from_exact(by).expect("a scaling factor is held exactly");
            Some((1, Modify::Scaled(by)))
        }
        _ => None,
    };
    let (amount, positive) = scaled_by_signed::<V>(fight, amount, None, &phases, scaled);
    let amount = amount.max(V::Number::zero());
    // The `modifiers` `Hook.ModifyBlock` hands `AfterModifyingBlockAmount`:
    // the listeners whose answer moved the number. Only a relic acts on it.
    let mut modifying = SmallVec::new();
    let after = sts2_core::ModelHook::AfterModifyingBlockAmount;
    if super::listen::heard::<V>(fight.live, fight.belts, after) {
        for phase in phases {
            let hook = sts2_core::Overrides::of(phase.model_hook());
            super::listen::each::<V>(fight, hook, |listener, _| {
                let sts2_core::Listener::Relic { state, .. } = listener else {
                    return;
                };
                let moved = match <V as sts2_core::Hooks>::modify_hook(listener, phase) {
                    Modify::Nothing => false,
                    Modify::Added(added) => added != V::Number::zero(),
                    Modify::Scaled(share) => share.exact() != Decimal::ONE,
                    answer => unimplemented!("a relic answering {answer:?} for block"),
                };
                let (seat, slot) = super::relic::place_of::<V>(fight.belts, state);
                let held = &fight.belts[seat][slot];
                if moved && !modifying.contains(&held.relic) {
                    modifying.push(held.relic);
                }
            });
        }
    }
    (amount, positive, modifying)
}

/// `CardCmd.Afflict` on one copy, once `AfflictionModel.CanAfflict` lets it.
///
/// A card of a type the affliction refuses keeps what it has. So does a card
/// with another affliction, or with this one where it does not stack.
///
/// Whether the card carries it after, which is when `CardCmd.Afflict` writes
/// a `CardAfflictedEntry`. A card that had it already counts again.
fn afflict<V: Rules>(held: &mut Face<V>, affliction: V::Affliction) -> bool {
    use sts2_core::AfflictionTable as _;

    let kind = V::card_kind(held.ident.card)
        .unwrap_or_else(|| unimplemented!("afflicting a card whose type is worked out"));
    if !affliction.afflicts(kind) {
        return false;
    }
    if !affliction.afflicts_unplayable() {
        unimplemented!("{affliction:?}, which refuses a card with Unplayable");
    }
    match held.ident.affliction {
        None => {
            held.ident.affliction = Some(affliction);
            true
        }
        Some(already) if already == affliction && affliction.stackable() => {
            unimplemented!("{affliction:?} stacking onto a card that has it")
        }
        // `CanAfflict` refuses a card that has an affliction, unless it is
        // this one and it stacks.
        Some(_) => false,
    }
}

/// A listener whose `TryModifyPowerAmountReceived` changed an amount.
#[derive(Debug, Clone, Copy)]
pub(super) enum Receiver<V: Rules> {
    /// A power, by the creature carrying it and the power it is.
    Power(CombatId, V::Power),
    /// A relic, by the seat whose belt it is on.
    Relic(sts2_core::SeatIndex, V::Relic),
}

/// The listeners whose `TryModifyPowerAmountReceived` changed an amount.
pub(super) type Received<V> = SmallVec<[Receiver<V>; 1]>;

/// `Hook.ModifyPowerAmountGiven`, over the listeners in the fight: what
/// `amount` of `power` from `giver` to `target` becomes, and the belts that
/// changed it. Every addition lands, then every share multiplies what they
/// left. `source` is the card giving it.
///
/// # Panics
///
/// A listener other than a relic changing it, which nothing ported is.
pub(super) fn given_by<V: Rules>(
    fight: Walk<'_, V>,
    giver: CombatId,
    target: Option<CombatId>,
    power: V::Power,
    amount: V::Number,
    source: Option<CardId<V>>,
) -> (V::Number, SmallVec<[V::Relic; 2]>) {
    use sts2_core::ModelHook;

    let mut modifying = SmallVec::new();
    let mut amount = amount;
    let mut modified = |hook: ModelHook, asked: ModifyHook<V>, amount: &mut V::Number| {
        if !super::listen::heard::<V>(fight.live, fight.belts, hook) {
            return;
        }
        super::listen::each::<V>(fight, sts2_core::Overrides::of(hook), |listener, _| {
            let moved = match (
                <V as sts2_core::Hooks>::modify_hook(listener, asked),
                listener,
            ) {
                (Modify::Nothing, _) => false,
                (Modify::Added(added), sts2_core::Listener::Relic { .. }) => {
                    *amount = *amount + added;
                    added != V::Number::zero()
                }
                (Modify::Scaled(share), sts2_core::Listener::Relic { .. }) => {
                    let scaled = exact::<V>(*amount) * share.exact();
                    *amount = num_traits::ToPrimitive::to_i64(&scaled)
                        .filter(|_| scaled.fract().is_zero())
                        .and_then(num_traits::cast)
                        .unwrap_or_else(|| unimplemented!("{power:?} given as {scaled}"));
                    share.exact() != Decimal::ONE
                }
                (answer, _) => {
                    unimplemented!(
                        "a listener answering {answer:?} for what {power:?} is given with"
                    )
                }
            };
            if moved && let sts2_core::Listener::Relic { state, .. } = listener {
                let (seat, slot) = super::relic::place_of::<V>(fight.belts, state);
                let held = &fight.belts[seat][slot];
                modifying.push(held.relic);
            }
        });
    };
    let additive = ModifyHook::PowerAmountGivenAdditive {
        power,
        giver,
        target,
    };
    modified(
        ModelHook::ModifyPowerAmountGivenAdditive,
        additive,
        &mut amount,
    );
    let multiplicative = ModifyHook::PowerAmountGivenMultiplicative {
        power,
        giver,
        target,
        amount,
        source,
    };
    modified(
        ModelHook::ModifyPowerAmountGivenMultiplicative,
        multiplicative,
        &mut amount,
    );
    (amount, modifying)
}

/// `Hook.ModifyPowerAmountReceived`: `amount` of `power` handed to each
/// listener in turn, and what each hands back.
pub(super) fn received_by<V: Rules>(
    fight: Walk<'_, V>,
    target: CombatId,
    power: V::Power,
    amount: V::Number,
) -> (V::Number, Received<V>) {
    let mut modifiers = Received::<V>::new();
    let hook = sts2_core::ModelHook::TryModifyPowerAmountReceived;
    if !super::listen::heard::<V>(fight.live, fight.belts, hook) {
        return (amount, modifiers);
    }
    let mut amount = amount;
    super::listen::each::<V>(fight, sts2_core::Overrides::of(hook), |listener, _| {
        let asked = ModifyHook::PowerAmountReceived {
            power,
            target,
            amount,
        };
        match (
            <V as sts2_core::Hooks>::modify_hook(listener, asked),
            listener,
        ) {
            (Modify::Nothing, _) => {}
            (Modify::Replaced(with), sts2_core::Listener::Power { holder, power, .. }) => {
                amount = with;
                modifiers.push(Receiver::Power(holder, power));
            }
            (Modify::Replaced(with), sts2_core::Listener::Relic { state, .. }) => {
                amount = with;
                let (seat, slot) = super::relic::place_of::<V>(fight.belts, state);
                let held = &fight.belts[seat][slot];
                modifiers.push(Receiver::Relic(seat, held.relic));
            }
            (answer, _) => {
                unimplemented!("a listener answering {answer:?} for what {power:?} lands with")
            }
        }
    });
    (amount, modifiers)
}

/// One effect after the fight's listeners have had their say.
///
/// The one path a number takes. It is worked out from the card that prints
/// it, then run through the hooks the effect names. No step in between is one
/// a caller can reach. A preview and the play it previews call this with the
/// same effect, so the number a card shows is the number it deals.
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn resolve<V: Rules>(
    seat: sts2_core::SeatIndex,
    fight: Walk<'_, V>,
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
            dealer,
            target,
            card: playing.map(|held| held.face.ident.card),
            powered: scaling == Scaling::Dynamic,
            damage: {
                let powered = scaling == Scaling::Dynamic;
                match (damage, playing) {
                    (Amount::Var(key), Some(held)) => {
                        card_hit::<V>(seat, fight, Some(dealer), Some(target), held, key, powered)
                    }
                    (damage, _) => {
                        let damage = carried::<V>(seat, playing, fight, damage, Some(target));
                        modified::<V>(
                            seat,
                            fight,
                            Some(dealer),
                            Some(target),
                            playing,
                            damage,
                            powered,
                        )
                    }
                }
            },
        },
        Effect::GainBlock {
            recipient,
            amount,
            scaling,
        } => {
            // A block effect names who gains it, and no creature the card
            // points at.
            let amount = carried::<V>(seat, playing, fight, amount, None);
            // `Hook.ModifyBlock` asks the played card's enchantment whatever
            // the block's props, since `GainBlock` always runs it.
            let amount = match playing.and_then(|held| {
                held.face
                    .ident
                    .enchantment
                    .map(|e| (e, held.face.enchantment_state()))
            }) {
                Some((enchanted, state)) => {
                    enchanted_by::<V>(enchanted, &state, EnchantHook::BlockAdditive, amount)
                }
                None => amount,
            };
            let powered = scaling == Scaling::Dynamic;
            // A preview has no `CardPlay`, though the copy may have played
            // earlier in the turn. A copy being played stands in the play
            // pile.
            let play = playing
                .filter(|held| fight.live.players[seat].cards.in_play(held.id))
                .and_then(|held| fight.live.history.play_at(held.id));
            let source = playing.map(|held| held.id);
            // `CreatureCmd.GainBlock` always runs `Hook.ModifyBlock`. A card's
            // block and a monster move's are the powered ones.
            let (amount, positive, modifying) = blocked::<V>(
                fight,
                recipient,
                amount,
                powered,
                scaling != Scaling::Static,
                playing.map(|held| held.face.ident.card),
                play,
                source,
            );
            Resolved::GainBlock {
                recipient,
                amount,
                moved: scaling != Scaling::Static,
                play,
                source,
                modifying,
                positive,
            }
        }
    };
    ResolvedEffect { effect }
}

/// One of `held`'s numbers dealt as a hit, which is `Hook.ModifyDamage` over
/// its `BaseValue` and then the floor. A fraction the copy holds goes into the
/// hook whole, the way the game's decimal does.
pub(crate) fn card_hit<V: Rules>(
    seat: sts2_core::SeatIndex,
    fight: Walk<'_, V>,
    dealer: Option<CombatId>,
    target: Option<CombatId>,
    held: &CardInstance<V>,
    key: V::CardVar,
    powered: bool,
) -> V::Number {
    if !held.face.fractional() {
        let damage = var::<V>(seat, held, key, fight, target);
        return modified::<V>(seat, fight, dealer, target, Some(held), damage, powered);
    }
    let damage = exact_var::<V>(seat, held, key, fight, target);
    whole::<V>(modified_exactly::<V>(
        seat,
        fight,
        dealer,
        target,
        Some(held),
        damage,
        powered,
    ))
}

/// [`var`] as the game's decimal, which holds the fraction Thrash writes.
pub(crate) fn exact_var<V: Rules>(
    seat: sts2_core::SeatIndex,
    held: &CardInstance<V>,
    key: V::CardVar,
    fight: Walk<'_, V>,
    target: Option<CombatId>,
) -> Decimal {
    let calculated =
        (held.face.ident.card.vars().iter()).any(|number| number.id == key && number.calculated);
    match calculated {
        false => held.face.exact(key),
        true => exact::<V>(var::<V>(seat, held, key, fight, target)),
    }
}

/// The number an effect carries, before any hook of its own runs.
fn carried<V: Rules>(
    seat: sts2_core::SeatIndex,
    playing: Option<&CardInstance<V>>,
    fight: Walk<'_, V>,
    amount: Amount<V>,
    target: Option<CombatId>,
) -> V::Number {
    match amount {
        Amount::Number(number) => number,
        Amount::Var(key) => var::<V>(
            seat,
            playing
                .unwrap_or_else(|| unimplemented!("a card's variable with no card being played")),
            key,
            fight,
            target,
        ),
    }
}

/// What one of this copy's variables is worth right now.
///
/// A `CalculatedVar` counts something in the live fight. It is worth what it
/// counted, rather than the zero it was built with. `target` is the creature
/// the card is pointed at, which some count on.
pub(crate) fn var<V: Rules>(
    seat: sts2_core::SeatIndex,
    held: &CardInstance<V>,
    key: V::CardVar,
    fight: Walk<'_, V>,
    target: Option<CombatId>,
) -> V::Number {
    let live = fight.live;
    let number = held.face.number(key);
    match number.calculated {
        false => number.base,
        // `CalculatedVar.Calculate` counts nothing once `IsInProgress` is
        // false, so a fight won or lost leaves the base alone.
        true if live.won || live.lost => {
            held.face
                .number(<V::CardVar as sts2_core::CardVars>::CALCULATION_BASE)
                .base
        }
        true => {
            let standing = osty::<V>(&live.creatures, seat).filter(|osty| osty.alive);
            let osty = standing.map(|osty| osty.hp);
            let osty_max_hp = standing.map(|osty| osty.max_hp);
            let targeted: Option<SmallVec<[Stack<V>; 8]>> =
                target.map(|target| live.powers.of(target).map(HeldPower::stack).collect());
            let exhausted: SmallVec<[V::Card; 16]> = live.players[seat]
                .cards
                .iter(Pile::Exhaust)
                .map(|held| held.face.ident.card)
                .collect();
            let enemy_powers: SmallVec<[Stack<V>; 16]> = live
                .creatures
                .iter()
                .filter(|creature| {
                    creature.alive && creature.side != <V::CombatSide as SideKinds>::PLAYER
                })
                .flat_map(|creature| live.powers.of(creature.id).map(HeldPower::stack))
                .collect();
            let hand: SmallVec<[V::Card; 12]> = live.players[seat]
                .cards
                .iter(Pile::Hand)
                .map(|held| held.face.ident.card)
                .collect();
            let owner = sts2_core::CardOwner {
                powers: &powers::<V>(live),
                osty,
                osty_max_hp,
                draw: live.players[seat].cards.len(Pile::Draw),
                discard: live.players[seat].cards.len(Pile::Discard),
                exhaust: &exhausted,
                hand_besides: live.players[seat].cards.len(Pile::Hand)
                    - usize::from(live.players[seat].cards.find(Pile::Hand, held.id).is_some()),
                enemy_powers: &enemy_powers,
                strikes: live.players[seat].cards.count_all(|face| {
                    sts2_core::CardTable::tagged(
                        face.ident.card,
                        <<V::Card as sts2_core::CardTable>::Tag as sts2_core::CardTagKinds>::STRIKE,
                    )
                }),
                player: live.player(seat).id,
                targeted: target,
                target: targeted.as_deref(),
                history: live.history(seat.creature()),
                block: live
                    .creatures
                    .iter()
                    .find(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER)
                    .map_or_else(num_traits::zero, |player| player.block),
                // `GetWithModifiers(CostModifiers.All)`, where the play pile
                // holds it.
                cost_in_play: live.players[seat]
                    .cards
                    .in_play(held.id)
                    .then(|| super::legal::costs::<V>(seat, held, None, fight).0),
                statuses: {
                    let status = |face: &Face<V>| {
                        V::card_kind(face.ident.card) == Some(sts2_core::CardKind::Status)
                    };
                    live.players[seat].cards.count_all(status)
                        - live.players[seat]
                            .cards
                            .iter(Pile::Exhaust)
                            .filter(|held| status(&held.face))
                            .count()
                },
                orbs: live.players[seat]
                    .orbs
                    .as_ref()
                    .map_or(&[], |orbs| orbs.queue.as_slice()),
                hand: &hand,
                star_cards: live.players[seat].cards.count_all(|face| {
                    sts2_core::CardTable::star_cost(face.ident.card) != sts2_core::Cost::None
                }),
                osty_attacks_besides: {
                    let osty_attack = |card| {
                        sts2_core::CardTable::tagged(
                            card,
                            <<V::Card as sts2_core::CardTable>::Tag as sts2_core::CardTagKinds>::OSTY_ATTACK,
                        )
                    };
                    live.players[seat]
                        .cards
                        .count_all(|face| osty_attack(face.ident.card))
                        - usize::from(osty_attack(held.face.ident.card))
                },
            };
            V::calculated_card_var(held.face.ident.card, key, held, owner)
        }
    }
}

/// The player's own powers, which is what a card reads about the fight.
///
/// Inline, because a calculated number asks for this every time it is read,
/// and a report reads every card in hand.
pub(super) fn powers<V: Rules>(live: &Live<V>) -> SmallVec<[Stack<V>; 8]> {
    live.creatures
        .iter()
        .find(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER)
        .map(|creature| live.powers.of(creature.id).map(HeldPower::stack).collect())
        .unwrap_or_default()
}

/// The effect one of a card's variables becomes, for a card played at
/// `target`.
///
/// `DynamicVar`'s subclass decides it: what the number runs through is the
/// class the card filed it under, not the key it is filed by. A number that
/// runs through nothing has no effect to build, and is worth what it reads.
///
/// `dealer` is whoever [`dealer_of`] names for the modifier.
pub(crate) fn effect_of<V: Rules>(
    modifier: CardNumberModifier,
    key: V::CardVar,
    dealer: CombatId,
    target: Option<CombatId>,
) -> Option<Effect<V>> {
    match modifier {
        CardNumberModifier::Damage | CardNumberModifier::OstyDamage => {
            target.map(|target| Effect::Attack {
                dealer,
                target,
                damage: Amount::Var(key),
                scaling: Scaling::Dynamic,
            })
        }
        CardNumberModifier::UnpoweredDamage => target.map(|target| Effect::Attack {
            dealer,
            target,
            damage: Amount::Var(key),
            scaling: Scaling::Static,
        }),
        CardNumberModifier::Block => Some(Effect::GainBlock {
            recipient: dealer,
            amount: Amount::Var(key),
            scaling: Scaling::Dynamic,
        }),
        CardNumberModifier::UnpoweredBlock => Some(Effect::GainBlock {
            recipient: dealer,
            amount: Amount::Var(key),
            scaling: Scaling::Static,
        }),
        // Not an effect a card resolves: the fight asks the hook itself.
        CardNumberModifier::PowerGiven | CardNumberModifier::ExtraDamage => None,
    }
}

/// Who deals or gains a card's number: the player, or for an Osty's damage
/// `Owner.Osty`, which is Osty dead or alive. Nobody for Osty's damage on a
/// player who never summoned one, where the game hands the hooks a null
/// dealer.
pub(crate) fn dealer_of<V: Rules>(
    seat: sts2_core::SeatIndex,
    modifier: CardNumberModifier,
    creatures: &[Fighter<V>],
) -> Option<CombatId> {
    match modifier {
        CardNumberModifier::OstyDamage => osty::<V>(creatures, seat).map(|osty| osty.id),
        CardNumberModifier::Damage
        | CardNumberModifier::UnpoweredDamage
        | CardNumberModifier::Block
        | CardNumberModifier::UnpoweredBlock
        | CardNumberModifier::PowerGiven
        | CardNumberModifier::ExtraDamage => Some(seat.creature()),
    }
}

/// `Owner.Osty`: seat `seat`'s Osty, dead or alive, once one is summoned.
pub(super) fn osty<V: Rules>(
    creatures: &[Fighter<V>],
    seat: sts2_core::SeatIndex,
) -> Option<&Fighter<V>> {
    let player = seat.creature();
    let osty = Some(sts2_core::ModelId::Monster(
        <V::Monster as sts2_core::MonsterKinds>::OSTY,
    ));
    creatures
        .iter()
        .find(|creature| creature.owner == Some(player) && creature.model == osty)
}

impl<V: Rules> Board<'_, V> {
    /// `ForgeCmd.Forge` for the actor's player, from `source` where a
    /// power's hook forged.
    pub(super) fn forge_by(&mut self, card: V::Card, amount: V::Number, source: Option<V::Power>) {
        let seat = self.seat();
        self.ran_on();
        // `ForgeCmd.Forge` does nothing in a fight that is over or ending.
        if self.over() {
            return;
        }
        // It creates one when the fight holds none it can reach. The exhaust
        // pile is not one it can reach, and is forged all the same. A full
        // hand sends the new one to the discard pile.
        let held = self.live.players[seat]
            .cards
            .any_outside_exhaust(|held| held.ident.card == card);
        let created = (!held).then(|| {
            let id = self.live.card_ids.mint();
            let full =
                self.live.players[seat].cards.len(Pile::Hand) >= super::pile::MAX_CARDS_IN_HAND;
            let pile = if full { Pile::Discard } else { Pile::Hand };
            let index = num_traits::NumCast::from(self.live.players[seat].cards.len(pile))
                .unwrap_or_else(|| unimplemented!("a pile deeper than a fight can number"));
            self.live.players[seat]
                .cards
                .push(pile, CardInstance::new(CardIdent::new(card), id));
            CardAt::new(id, pile, index)
        });
        // `IncreaseSovereignBladeDamage`, over `AllCards`: every copy, in
        // every pile, the one being played among them.
        self.live.players[seat].cards.write_all_cards(
            |held| held.ident.card == card,
            |held| held.add_damage(amount),
        );
        // The blade goes through `AddGeneratedCardToCombat`, so it enters the
        // fight the way a generated card does. It is raised after the raise,
        // which is the order `ForgeCmd` adds and then increases in.
        if let Some(at) = created {
            self.card_entered_combat(at, card, false);
            let player = self.live.player(seat).id;
            self.card_generated(at, card, Some(player));
        }
        // `Hook.AfterForge`.
        if !super::listen::heard::<V>(self.live, self.belts, sts2_core::ModelHook::AfterForge) {
            return;
        }
        let player = self.live.player(seat).id;
        let hook = sts2_core::EventHook::Forged {
            amount,
            player,
            source,
        };
        let mut walking = super::listen::Walking::begin(self.live, self.belts, hook);
        while let Some((listener, effect)) =
            walking.as_mut().and_then(|walking| self.next_of(walking))
        {
            match (listener, effect) {
                (Listener::Power { power, .. }, HookEffect::ForgeOthers { card, amount }) => {
                    // `Players.Where(p => p.Creature.IsAlive && p != forger)`.
                    let others: SmallVec<[CombatId; 4]> = self
                        .live
                        .players
                        .seats()
                        .map(|seat| self.live.player(seat))
                        .filter(|fighter| fighter.id != player && fighter.is_alive())
                        .map(|fighter| fighter.id)
                        .collect();
                    for other in others {
                        let actor = std::mem::replace(&mut self.actor, other);
                        let reaching = self.reaching.take();
                        self.forge_by(card, amount, Some(power));
                        self.actor = actor;
                        self.reaching = reaching;
                    }
                }
                (listener, effect) => {
                    unimplemented!("{listener:?} asking for {effect:?} once a blade is forged")
                }
            }
        }
    }

    /// `Hook.AfterBlockGained`, once `CreatureCmd.GainBlock` gave `creature`
    /// `amount`. The game raises it for a gain the modifiers left at none.
    pub(super) fn block_gained(&mut self, creature: CombatId, amount: V::Number, positive: bool) {
        let gained = sts2_core::ModelHook::AfterBlockGained;
        if !super::listen::heard::<V>(self.live, self.belts, gained) {
            return;
        }
        let hook = sts2_core::EventHook::BlockGained {
            creature,
            amount,
            positive,
        };
        let mut walking = super::listen::Walking::begin(self.live, self.belts, hook);
        while let Some((listener, effect)) =
            walking.as_mut().and_then(|walking| self.next_of(walking))
        {
            match (listener, effect) {
                (Listener::Power { .. }, effect @ HookEffect::DamageRandomOpponent { .. }) => {
                    self.listener_asked(listener, effect)
                }
                (listener, effect) => {
                    unimplemented!("{listener:?} asking for {effect:?} once block is gained")
                }
            }
        }
    }

    /// `CreateClone` of the card `id` into `pile`, as
    /// `AddGeneratedCardToCombat` puts it there.
    /// [`Board::clone_named`], then `writes` on the clone where one landed.
    pub(super) fn clone_written(
        &mut self,
        id: sts2_core::CardId<V>,
        pile: Pile,
        writes: &[(sts2_core::Change<V>, sts2_core::Lasting)],
    ) {
        if let Some(at) = self.clone_named(id, pile) {
            for &(change, lasting) in writes {
                sts2_core::Fight::modify_card(self, at, change, lasting);
            }
        }
    }

    pub(super) fn clone_named(
        &mut self,
        id: sts2_core::CardId<V>,
        pile: Pile,
    ) -> Option<CardAt<V>> {
        let seat = self.seat();
        let face = self.live.players[seat]
            .cards
            .named(id)
            .unwrap_or_else(|| unimplemented!("a card cloned from outside the fight"))
            .face;
        self.clone_face(face, pile)
    }

    /// `CardPileCmd.AddGeneratedCardToCombat` of a clone of the card the fight
    /// keeps as `kept`.
    pub(super) fn clone_kept(
        &mut self,
        kept: sts2_core::CardId<V>,
        pile: Pile,
    ) -> Option<CardAt<V>> {
        let face = self
            .live
            .kept_faces
            .iter()
            .find(|(id, _)| *id == kept)
            .map(|&(_, face)| face)
            .unwrap_or_else(|| unimplemented!("cloning a kept card the fight lost"));
        self.clone_face(face, pile)
    }

    /// A clone of `face` into `pile`, marked a clone and generated.
    ///
    /// `DynamicVarSet.Clone` gives each of the clone's numbers its owner,
    /// and `CalculatedVar.SetOwner` gives a calculated number the base its
    /// `CalculationBase` prints. The card's own base can be older, as after
    /// Dampen downgrades it.
    fn clone_face(&mut self, mut face: Face<V>, pile: Pile) -> Option<CardAt<V>> {
        let seat = self.seat();
        face.recalculate();
        let id = self.live.card_ids.mint();
        // A full hand sends the card to the discard pile instead.
        let full = pile == Pile::Hand
            && self.live.players[seat].cards.len(Pile::Hand) >= super::pile::MAX_CARDS_IN_HAND;
        let pile = if full { Pile::Discard } else { pile };
        let index = num_traits::NumCast::from(self.live.players[seat].cards.len(pile))
            .unwrap_or_else(|| unimplemented!("a pile deeper than a fight can number"));
        self.live.players[seat].cards.push(pile, face.named(id));
        let at = CardAt::new(id, pile, index);
        self.card_entered_combat(at, face.ident.card, true);
        self.card_generated(at, face.ident.card, self.creator());
        Some(at)
    }

    /// `OrbCmd.EvokeLast`, which does nothing once the fight is ending.
    pub(super) fn evoke_last(&mut self) {
        self.evoke(true, true);
    }

    /// `OrbCmd.Evoke` on the first orb in the queue, or the last, leaving
    /// when `dequeue` says so.
    fn evoke(&mut self, last: bool, dequeue: bool) {
        let seat = self.seat();
        if self.over() {
            return;
        }
        let Some(orb) = self.live.players[seat].orbs.as_mut().and_then(|orbs| {
            let at = match last {
                true => orbs.queue.len().checked_sub(1)?,
                false => 0,
            };
            (at < orbs.queue.len()).then(|| match dequeue {
                true => orbs.queue.remove(at),
                false => orbs.queue[at],
            })
        }) else {
            return;
        };
        // An orb's commands name no `cardSource`, even inside a card's play.
        let playing = self.playing.take();
        let targets = V::orb_evoke(&orb, self);
        self.playing = playing;
        let hook = sts2_core::EventHook::OrbEvoked {
            player: seat.creature(),
            orb: orb.orb,
        };
        let mut walking = super::listen::Walking::begin(self.live, self.belts, hook);
        while let Some((listener, effect)) =
            walking.as_mut().and_then(|walking| self.next_of(walking))
        {
            match (listener, effect) {
                // `CreatureCmd.Damage` on the evoke's targets still alive.
                (Listener::Power { owner, .. }, HookEffect::DamageTargets(amount)) => {
                    let alive: SmallVec<[CombatId; 8]> = targets
                        .iter()
                        .copied()
                        .filter(|&target| {
                            self.at(target)
                                .is_some_and(|at| self.live.creatures[at].alive)
                        })
                        .collect();
                    self.damage_command(|board| {
                        for target in alive {
                            let hit = resolve::<V>(
                                seat,
                                board.walk(),
                                None,
                                Effect::Attack {
                                    dealer: owner,
                                    target,
                                    damage: Amount::Number(amount),
                                    scaling: Scaling::Static,
                                },
                            );
                            board.apply(hit);
                        }
                    });
                }
                (listener, effect) => {
                    unimplemented!("{listener:?} asking for {effect:?} as an orb is evoked")
                }
            }
        }
    }

    /// `PlayerCmd.AddPet`: `monster` for the player, at the health its own
    /// row starts on, standing after the last of the player's side.
    ///
    /// `CreatureCmd.Add` ends on `AfterCreatureAddedToCombat`.
    ///
    /// # Panics
    ///
    /// A listener asking for anything as the pet arrives.
    pub(super) fn add_pet(&mut self, monster: V::Monster) -> CombatId {
        let seat = self.seat();
        let id = self.live.next_creature();
        let at = self
            .live
            .creatures
            .iter()
            .rposition(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER)
            .map_or(0, |last| last + 1);
        let initial = V::monster_hp_bounds(
            monster,
            &sts2_core::MonsterModel::new(monster),
            self.ascension,
        )
        .max();
        let player = self.live.player(seat).id;
        self.live.creatures.insert(
            at,
            Fighter {
                id,
                model: Some(sts2_core::ModelId::Monster(monster)),
                side: <V::CombatSide as SideKinds>::PLAYER,
                hp: initial,
                max_hp: initial,
                block: V::Number::zero(),
                alive: true,
                next_move: Some(super::live::Telegraphed::unset()),
                owner: Some(player),
            },
        );
        let hook = sts2_core::EventHook::CreatureAdded {
            creature: id,
            side: <V::CombatSide as SideKinds>::PLAYER,
            point: self.live.point,
        };
        if let Some((listener, effect)) =
            super::listen::first_answer::<V>(seat, self.live, self.belts, hook)
        {
            unimplemented!("{listener:?} asking for {effect:?} as a pet arrives");
        }
        id
    }

    /// A hook this fight raises and nothing ported answers.
    ///
    /// # Panics
    ///
    /// Anything in the fight listening to `hook`.
    fn unheard(&self, hook: sts2_core::ModelHook) {
        if super::listen::heard::<V>(self.live, self.belts, hook) {
            unimplemented!("a listener to {}", hook.name());
        }
    }
}

/// [`Board`]'s kind, which names it for any borrow of the fight.
///
/// A play that stops holds no borrow, and is lent the board again at each
/// step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) struct BoardKind<V>(std::marker::PhantomData<fn() -> V>);

impl<V: Rules> sts2_core::FightKind<V> for BoardKind<V> {
    type Fight<'a> = Board<'a, V>;
}

/// Where a play stopped on this engine's board.
pub(super) type Resume<V> = <V as sts2_core::ContentVersion>::Resume<BoardKind<V>>;

/// How far a play got on this engine's board.
pub(super) type Played<V> = sts2_core::Played<V, Resume<V>>;

impl<V: Rules> Fight<V> for Board<'_, V> {
    type Kind = BoardKind<V>;
    type ResolvedEffect = ResolvedEffect<V>;
    type Creature<'a>
        = super::live::Seen<'a, V>
    where
        Self: 'a;

    fn actor(&self) -> CombatId {
        self.actor
    }

    fn target(&self) -> CombatId {
        // `resolve` refuses a play whose card names a creature and was pointed
        // at none, so a card that reads this was pointed at one. An auto-play
        // picks one where one stands.
        self.target
            .expect("a card that names a target is played at one")
    }

    fn creatures(&self) -> impl Iterator<Item = Self::Creature<'_>> {
        self.live.creatures.iter().map(|fighter| super::live::Seen {
            fighter,
            powers: &self.live.powers,
        })
    }

    /// A fight whose actor has left it answers for the player, which is the
    /// side every effect with no creature behind it belongs to.
    fn side(&self) -> V::CombatSide {
        // A monster that left the fight during its own move is still the
        // enemy, and its move still reaches the player. The player never
        // leaves.
        self.at(self.actor)
            .map(|at| self.live.creatures[at].side)
            .unwrap_or(<V::CombatSide as SideKinds>::ENEMY)
    }

    fn resolve(&self, effect: Effect<V>) -> Self::ResolvedEffect {
        let seat = self.actor_seat();
        if self.hits_wait() && matches!(effect, Effect::Attack { .. }) {
            return ResolvedEffect {
                effect: Resolved::Held(effect),
            };
        }
        resolve::<V>(seat, self.walk(), self.acting().as_ref(), effect)
    }

    fn orb_value(&self, orb: V::Orb, base: V::Number) -> V::Number {
        orb_value::<V>(self.walk(), self.seat(), orb, base)
    }

    fn attack(
        &mut self,
        scaling: Scaling,
        hits: impl FnOnce(&mut Self),
    ) -> sts2_core::AttackResults<V> {
        self.attack_from(self.actor, scaling, hits)
    }

    fn attack_from(
        &mut self,
        attacker: CombatId,
        scaling: Scaling,
        hits: impl FnOnce(&mut Self),
    ) -> sts2_core::AttackResults<V> {
        self.ran_on();
        let choice = self.choice;
        if let Choice::Move { .. } = choice {
            self.choice = Choice::Move { attacking: true };
        }
        let results = self.attack_landing(attacker, scaling, hits);
        self.choice = choice;
        results
    }

    fn damage_command(&mut self, hits: impl FnOnce(&mut Self)) {
        Board::damage_command(self, hits);
    }

    fn apply_unblockable(&mut self, effect: Self::ResolvedEffect) {
        self.ran_on();
        match effect.effect {
            Resolved::Attack {
                dealer,
                target,
                damage,
                powered,
                card,
            } => {
                // `CreatureCmd.Damage` lands nothing from a dead dealer.
                if self
                    .at(dealer)
                    .is_some_and(|at| self.live.creatures[at].alive)
                {
                    self.hit(Some(dealer), target, damage, powered, true, card, false);
                }
            }
            _ => unreachable!("an effect other than a hit landed as an unblockable hit"),
        }
    }

    #[tracing::instrument(level = "trace", skip_all)]
    fn apply(&mut self, effect: Self::ResolvedEffect) {
        self.ran_on();
        match effect.effect {
            // `AttackCommand.Execute` stops before any hit whose attacker is
            // dead. A Spectral Knight that Thorns kills on its second hit
            // lands no third. One `CreatureCmd.Damage` over several targets
            // asks only before its first, so Thorns on one does not spare
            // the rest.
            Resolved::Attack { dealer, .. }
                if !(self.commanding && std::mem::replace(&mut self.dealer_asked, true))
                    && self
                        .at(dealer)
                        .is_none_or(|at| !self.live.creatures[at].alive) => {}
            Resolved::Attack {
                dealer,
                target,
                damage,
                powered,
                card,
            } => self.hit(Some(dealer), target, damage, powered, false, card, true),
            Resolved::Held(effect) => self.held.push(effect),
            Resolved::GainBlock {
                recipient,
                amount,
                moved,
                play,
                source,
                modifying,
                positive,
            } => {
                // `CreatureCmd.GainBlock` gives nothing once the fight is over
                // or ending, as after a hit that killed the player.
                if self.over() {
                    return;
                }
                // `Hook.AfterModifyingBlockAmount`, asked of each relic that
                // changed the gain, before it lands. They are the belt of the
                // player who gains it.
                let seat = match modifying.is_empty() {
                    true => sts2_core::SeatIndex(0),
                    false => self.live.seat_of(recipient).unwrap_or_else(|| self.seat()),
                };
                for relic in modifying {
                    let hook = sts2_core::EventHook::BlockAmountModified { amount, source };
                    if let Some(effect) =
                        super::listen::ask_relic(&*self.live, seat, self.belts, relic, hook).first()
                    {
                        unimplemented!("{relic:?} asking for {effect:?} once it changed block")
                    }
                }
                let Some(at) = self.at(recipient) else { return };
                // A fraction below one gains, and adds nothing to the block.
                if positive {
                    self.live.creatures[at].block = self.live.creatures[at].block + amount;
                    self.live.history.block_gained(sts2_core::BlockGained {
                        receiver: recipient,
                        moved,
                        play,
                    });
                }
                self.block_gained(recipient, amount.max(V::Number::zero()), positive);
            }
        }
    }

    fn remove_power(&mut self, creature: CombatId, power: V::Power) {
        self.ran_on();
        self.remove_power_carried(creature, power);
    }

    fn add_at_random(&mut self, card: V::Card, pile: Pile) -> Option<CardAt<V>> {
        self.ran_on();
        self.add_at_random_here(card, pile, self.creator())
    }

    fn modify_power_amount(
        &mut self,
        creature: CombatId,
        power: V::Power,
        amount: V::Number,
        applier: Option<CombatId>,
    ) {
        self.modify_amount_from(creature, power, amount, applier);
    }

    fn powers_on(&self, creature: CombatId) -> Vec<sts2_core::HeldInstance<V>> {
        use sts2_core::PowerModel as _;

        self.live
            .powers
            .of(creature)
            .map(|held| sts2_core::HeldInstance {
                power: held.power(),
                amount: held.amount.amount(),
                applier: held.applier,
                fresh: held.state == V::PowerState::new(held.power()),
            })
            .collect()
    }

    fn set_max_hp(&mut self, creature: CombatId, hp: V::Health) {
        self.ran_on();
        // `SetMaxHp` kills a creature it sets to nothing, which no caller
        // asks.
        if hp <= num_traits::zero() {
            unimplemented!("a maximum health set to nothing");
        }
        let Some(at) = self.at(creature) else { return };
        let held = &mut self.live.creatures[at];
        held.max_hp = hp;
        held.hp = held.hp.min(hp);
    }

    fn set_max_and_current_hp(&mut self, creature: CombatId, hp: V::Health) {
        self.ran_on();
        let Some(at) = self.at(creature) else { return };
        let before = self.live.creatures[at].hp;
        self.live.creatures[at].max_hp = hp;
        // `SetCurrentHp`, which brings a dead creature back above zero and
        // tells its listeners what changed.
        self.live.creatures[at].hp = hp;
        self.live.creatures[at].alive = hp > V::Health::zero();
        if hp != before {
            let delta: V::Number = num_traits::cast(hp - before)
                .unwrap_or_else(|| unimplemented!("a change of {hp:?} hit points"));
            self.current_hp_changed(creature, delta);
        }
    }

    fn niche_int(&mut self, min: V::Number, max: V::Number) -> V::Number {
        use sts2_core::rng::GameRng as _;

        let cast = |n: V::Number| -> i32 {
            num_traits::cast(n).unwrap_or_else(|| unimplemented!("a niche draw at {n:?}"))
        };
        let span = sts2_core::rng::Span::new(cast(min), cast(max))
            .unwrap_or_else(|_| unimplemented!("a niche draw from {min:?} to {max:?}"));
        num_traits::cast(self.rngs.niche.next_int_range(span))
            .expect("a draw inside the span is a game number")
    }

    fn escape(&mut self) {
        self.ran_on();
        self.escape_creature(self.actor);
    }

    fn damage_unpowered(
        &mut self,
        dealer: Option<CombatId>,
        target: CombatId,
        amount: V::Number,
        unblockable: bool,
    ) {
        self.hit_unpowered(dealer, target, amount, unblockable);
    }

    fn kill(&mut self, creature: CombatId) {
        self.kill_outright(creature, false);
    }

    fn self_note(&self) -> V::Number {
        let playing = self.playing.expect("a card behavior has a card");
        self.live.noted(playing)
    }

    fn set_self_note(&mut self, note: V::Number) {
        let playing = self.playing.expect("a card behavior has a card");
        self.live.note(playing, note);
    }

    fn waited_attack_results(&mut self) -> sts2_core::AttackResults<V> {
        self.live
            .waited_attack
            .take()
            .unwrap_or_else(|| unimplemented!("reading an attack's results it never waited on"))
    }

    fn attack_results(&self) -> sts2_core::AttackResults<V> {
        if !self.held.is_empty() {
            unimplemented!("reading an attack's results while its hits wait on a question");
        }
        self.landed.clone().unwrap_or_default()
    }

    fn keep_creature(&mut self, holder: CombatId, power: V::Power, creature: CombatId) {
        let creatures = &mut self.live.kept_mut(holder, power).creatures;
        if !creatures.contains(&creature) {
            creatures.push(creature);
        }
    }

    fn random_orb(&mut self, orbs: &[V::Orb]) -> Option<V::Orb> {
        use sts2_core::rng::GameRng as _;

        self.rngs.orbs.choose(orbs).ok().copied()
    }

    fn afflict(&mut self, to: sts2_core::Writing<V>, affliction: V::Affliction) {
        self.ran_on();
        self.write_cards([HookEffect::Afflict { to, affliction }]);
    }

    fn clear_affliction(&mut self, to: sts2_core::Writing<V>) {
        self.write_cards([HookEffect::ClearAffliction(to)]);
    }

    fn modify_cards(&mut self, to: sts2_core::Writing<V>, change: sts2_core::Change<V>) {
        self.ran_on();
        self.write_cards([HookEffect::ModifyCards { to, change }]);
    }

    fn add_replays(&mut self, to: sts2_core::Writing<V>, amount: V::Number) {
        self.ran_on();
        self.write_cards([HookEffect::AddReplays { to, amount }]);
    }

    fn energy(&self) -> V::Number {
        let seat = self.seat();
        num_traits::cast(self.live.players[seat].energy).expect("energy fits a game number")
    }

    fn end_turn(&mut self) {
        let seat = self.seat();
        self.live.players[seat].ending_turn = true;
    }

    fn doom_kill(&mut self, creatures: &[CombatId]) {
        if !creatures.is_empty() {
            Board::doom_kill(self, creatures);
        }
    }

    fn add_card_reward(&mut self) {
        self.live.extra_rewards.push(super::live::ExtraReward::Card);
    }

    fn death_triggers_fatal(&self, creature: CombatId) -> bool {
        let Some(at) = self.at(creature) else {
            return true;
        };
        let carrier = self.live.creatures[at].carrier();
        self.live.powers.of(creature).all(|subject| {
            !<V as sts2_core::Hooks>::decide_hook(
                subject.listener(
                    carrier,
                    self.live.history(carrier.player),
                    &super::live::Onlookers {
                        creatures: &self.live.creatures,
                        powers: &self.live.powers,
                        kept: &self.live.kept,
                        belts: Some(&*self.belts),
                        attack: &self.live.attack_results,
                    },
                ),
                sts2_core::DecisionHook::ShouldOwnerDeathTriggerFatal,
            )
            .is_decision_and(|fatal| !fatal)
        })
    }

    fn note_on_power(&mut self, creature: CombatId, power: V::Power, note: V::Number) {
        // The instance `PowerCmd.Apply` handed back, which is the last of its
        // kind.
        if let Some(held) = self
            .live
            .powers
            .of_mut(creature)
            .filter(|held| held.power() == power)
            .last()
        {
            <V as sts2_core::Hooks>::power_noted(&mut held.state, note);
        }
    }

    fn keep_card_on_power(&mut self, creature: CombatId, power: V::Power, card: CardAt<V>) {
        let seat = self.seat();
        let at = self.standing(card);
        let mut face = self.live.players[seat]
            .cards
            .get(card.pile(), at)
            .expect("a card stands where it was found")
            .face;
        // `CardCmd.ClearAffliction` on the clone.
        face.ident.affliction = None;
        let kept = self.live.card_ids.mint();
        self.live.kept_faces.push((kept, face));
        if let Some(held) = self
            .live
            .powers
            .of_mut(creature)
            .filter(|held| held.power() == power)
            .last()
        {
            <V as sts2_core::Hooks>::power_kept_card(&mut held.state, kept);
        }
    }

    fn procure_random_potion(&mut self, stream: V::RunRngType) {
        let seat = self.seat();
        self.ran_on();
        let procure = sts2_core::HookEffect::ProcureRandomPotions {
            count: 1,
            stream,
            in_combat: true,
            into_gained: false,
        };
        self.live.for_run.push((seat, procure));
    }

    fn gain_max_hp(&mut self, creature: CombatId, amount: V::Number) {
        self.ran_on();
        let Some(at) = self.at(creature) else { return };
        let gained: V::Health =
            num_traits::cast(amount).expect("a maximum health gain fits the build's health");
        let held = &mut self.live.creatures[at];
        held.max_hp = held.max_hp + gained;
        self.heal(creature, amount);
    }

    fn lose_max_hp(&mut self, creature: CombatId, amount: V::Number) -> Option<V::Health> {
        self.ran_on();
        let at = self.at(creature)?;
        let held = &self.live.creatures[at];
        let max_hp = V::Number::from(held.max_hp) - amount;
        let hp = V::Number::from(held.hp);
        // `SetMaxHp` at one or more.
        let max: V::Health = num_traits::cast(max_hp.max(num_traits::one()))
            .expect("a maximum health fits the build's health");
        if max_hp < hp {
            self.hit_unpowered(None, creature, hp - max_hp, true);
            if sts2_core::Fight::waiting(self) {
                return Some(max);
            }
        }
        // `SetMaxHpInternal` takes the health down to the new maximum, which
        // a hit Buffer stopped leaves above it.
        sts2_core::Fight::set_max_hp(self, creature, max);
        None
    }

    fn gold(&self) -> V::Number {
        let seat = self.seat();
        self.live.players[seat].gold
    }

    fn write_encounter(&mut self, write: impl FnOnce(&mut V::EncounterState)) {
        write(&mut self.live.encounter_state);
    }

    fn lose_gold(&mut self, amount: V::Number) {
        let seat = self.seat();
        self.ran_on();
        self.live.players[seat].gold =
            (self.live.players[seat].gold - amount).max(V::Number::zero());
    }

    fn gain_gold(&mut self, amount: V::Number) {
        let seat = self.seat();
        self.ran_on();
        // `Hook.ModifyGoldGained`, over the fight's listeners in the order
        // the game asks them, as Bowler Hat scales a gain.
        let hook = sts2_core::ModifyHook::GoldGained {
            player: seat.creature(),
        };
        let mut gained = exact::<V>(amount);
        super::listen::each::<V>(
            self.walk(),
            sts2_core::Overrides::of(hook.model_hook()),
            |listener, _| match <V as sts2_core::Hooks>::modify_hook(listener, hook) {
                Modify::Nothing => {}
                Modify::Scaled(by) => gained *= by.exact(),
                // The listener's own number in place of what it was handed.
                Modify::Replaced(with) => gained = exact::<V>(with),
                answer => unimplemented!("a listener answering {hook:?} with {answer:?}"),
            },
        );
        // `AfterModifyingGoldGained`, which only flashes what answered.
        if let Some((listener, effect)) = super::listen::first_answer::<V>(
            seat,
            self.live,
            self.belts,
            sts2_core::EventHook::GoldGainModified,
        ) {
            unimplemented!("{listener:?} asking for {effect:?} as a gold gain is modified");
        }
        let amount = whole::<V>(gained);
        if amount <= V::Number::zero() {
            return;
        }
        self.live.players[seat].gold = self.live.players[seat].gold + amount;
        // `AfterGoldGained`, once the gold has landed.
        let mut walking = super::listen::Walking::begin(
            self.live,
            self.belts,
            sts2_core::EventHook::GoldGained {
                player: seat.creature(),
            },
        );
        while let Some((listener, effect)) =
            walking.as_mut().and_then(|walking| self.next_of(walking))
        {
            match (listener, effect) {
                // `CreatureCmd.GainMaxHp` on the relic's owner.
                (Listener::Relic { seat, .. }, HookEffect::GainMaxHp(amount)) => {
                    sts2_core::Fight::gain_max_hp(self, seat.creature(), amount)
                }
                (listener, effect) => {
                    unimplemented!("{listener:?} asking for {effect:?} as a fight gains gold")
                }
            }
        }
    }

    fn gain_stars(&mut self, amount: V::Number) {
        let seat = self.seat();
        self.ran_on();
        // The pool is a count, so a card printing a negative one is spending
        // rather than gaining. No card on this build does.
        if amount < V::Number::zero() {
            unimplemented!("a card spending stars as it plays");
        }
        // `PlayerCmd.GainStars`, which gains nothing once the fight is ending.
        if self.over() {
            return;
        }
        self.live.modify_stars(seat, amount);
        // `Hook.AfterStarsGained`.
        let gained = sts2_core::ModelHook::AfterStarsGained;
        if !super::listen::heard::<V>(self.live, self.belts, gained) {
            return;
        }
        let hook = sts2_core::EventHook::StarsGained {
            player: seat.creature(),
            amount,
        };
        let mut walking = super::listen::Walking::begin(self.live, self.belts, hook);
        while let Some((listener, effect)) =
            walking.as_mut().and_then(|walking| self.next_of(walking))
        {
            match (listener, effect) {
                // `CreatureCmd.Damage` over `HittableEnemies`, unpowered, from
                // the power's owner.
                (Listener::Power { owner, .. }, HookEffect::DamageOpponents(amount)) => {
                    // One `CreatureCmd.Damage` over them all.
                    let targets = self.hittable_opponents_of(owner);
                    self.damage_command(|board| {
                        for target in targets {
                            board.hit_unpowered(Some(owner), target, amount, false);
                        }
                    });
                }
                (listener, effect) => {
                    unimplemented!("{listener:?} asking for {effect:?} once stars are gained")
                }
            }
        }
    }

    fn modify_energy(&mut self, amount: V::Number) {
        let seat = self.seat();
        self.ran_on();
        let size = |amount: V::Number| {
            num_traits::NumCast::from(amount)
                .unwrap_or_else(|| unimplemented!("an energy change of {amount:?}"))
        };
        // `PlayerCombatState` holds the pool as a count and clamps it at zero.
        // `PlayerCmd.GainEnergy` gains nothing once the fight is ending.
        match amount < V::Number::zero() {
            true => {
                self.live.players[seat].energy =
                    self.live.players[seat].energy.saturating_sub(size(-amount))
            }
            false if self.over() || amount.is_zero() => {}
            false => {
                // `Hook.ModifyEnergyGain`, then `AfterModifyingEnergyGain`.
                let player = self.live.player(seat).id;
                let modifying = sts2_core::ModelHook::ModifyEnergyGain;
                let gained = match super::listen::heard::<V>(self.live, self.belts, modifying) {
                    false => amount,
                    true => {
                        let hook = sts2_core::ModifyHook::EnergyGain { player };
                        let gained = modified_by::<V>(self.walk(), amount, &[hook]);
                        let after = sts2_core::EventHook::EnergyGainModified;
                        if let Some((listener, effect)) =
                            super::listen::first_answer::<V>(seat, self.live, self.belts, after)
                        {
                            unimplemented!(
                                "{listener:?} asking for {effect:?} once an energy gain is worked out"
                            )
                        }
                        gained
                    }
                };
                if gained > V::Number::zero() {
                    self.live.players[seat].energy = self.live.players[seat].energy + size(gained);
                }
            }
        }
    }

    fn apply_power(&mut self, at: CombatId, power: V::Power, amount: PowerAmount<V::Number>) {
        self.land(at, power, amount, Some(self.actor));
    }

    fn apply_power_from(
        &mut self,
        at: CombatId,
        power: V::Power,
        amount: PowerAmount<V::Number>,
        applier: Option<CombatId>,
    ) {
        self.land(at, power, amount, applier);
    }

    fn apply_power_copy(
        &mut self,
        at: CombatId,
        copy: V::PowerState,
        amount: PowerAmount<V::Number>,
        applier: Option<CombatId>,
    ) {
        let power = sts2_core::PowerModel::<V>::power(&copy);
        self.land_copy(at, power, amount, applier, Some(copy));
    }

    fn apply_to_opponents(&mut self, power: V::Power, amount: PowerAmount<V::Number>) {
        // The list is taken before the first lands, as an attack's is. A
        // monster's move is handed `PlayerCreatures`, a dead player among
        // them, and `PowerCmd.Apply` lands on each.
        let targets = match self.side() == <V::CombatSide as SideKinds>::ENEMY {
            true => super::turn::seats(self.live)
                .map(sts2_core::SeatIndex::creature)
                .collect(),
            false => self.opponents(),
        };
        self.land_each(&targets, power, amount, Some(self.actor));
    }

    fn intent_damage(&self) -> V::Number {
        self.intent_damage
    }

    fn intent_repeats(&self) -> V::Count {
        self.intent_repeats
    }

    fn draw(&mut self, count: V::Number) -> sts2_core::Drew<V> {
        let seat = self.seat();
        let mut drew = sts2_core::Drew::new();
        // `IsOverOrEnding` draws nothing.
        if self.stopped() {
            return drew;
        }
        // A negative count draws nothing: the game's own loop runs no times.
        let count = num_traits::NumCast::from(count).unwrap_or(0usize);
        if !self.pausable() {
            self.ended = draw(
                seat,
                self.live,
                self.rngs.reborrow(),
                self.belts,
                self.ascension,
                count,
                Some(&mut drew),
            );
            return drew;
        }
        // A question a card this effect auto-played asked stands the draw
        // still, before it starts.
        if self.live.asking.is_some() {
            self.live.defer(super::live::Deferred::Draw {
                count,
                hand_draw: false,
                resumed: false,
                drew: Some(drew),
            });
            return sts2_core::Drew::new();
        }
        self.ended = draw_in_effect(
            seat,
            self.live,
            self.rngs.reborrow(),
            self.belts,
            self.ascension,
            count,
            false,
            false,
            Some(drew),
        );
        // A draw that stood still left its copies for the effect's resume.
        match self.live.asking {
            Some(_) => sts2_core::Drew::new(),
            None => self.live.drew.take().unwrap_or_default(),
        }
    }

    fn find_card(&self, id: CardId<V>) -> Option<sts2_core::CardAt<V>> {
        let seat = self.seat();
        self.live.players[seat].cards.find_any(id)
    }

    fn ascension(&self) -> V::AscensionLevel {
        self.ascension
    }

    fn var(&self, key: V::CardVar) -> V::Number {
        let seat = self.seat();
        let held = self
            .acting()
            .unwrap_or_else(|| unimplemented!("a number printed by no card being played"));
        // A power's number runs its hook where the power lands, which knows
        // the applier and the target.
        if held.face.number(key).modifier.is_some_and(|modifier| {
            !matches!(
                modifier,
                CardNumberModifier::PowerGiven | CardNumberModifier::ExtraDamage
            )
        }) {
            // The hooks it runs through name creatures to run against, and a
            // bare read has none. `Fight::resolve` is where both arrive.
            unimplemented!(
                "the {} read without the creatures it runs against",
                key.name()
            );
        }
        var::<V>(seat, &held, key, self.walk(), self.target)
    }

    fn power_var(&self, key: V::CardVar) -> V::Number {
        let seat = self.seat();
        let held = self
            .acting()
            .unwrap_or_else(|| unimplemented!("a number printed by no card being played"));
        var::<V>(seat, &held, key, self.walk(), self.target)
    }

    fn history(&self) -> sts2_core::History<'_, V> {
        self.live.history(self.actor)
    }

    fn playing(&self) -> CardId<V> {
        self.playing
            .unwrap_or_else(|| unimplemented!("naming the card being played with none played"))
    }

    fn waiting(&self) -> bool {
        self.live.asking.is_some()
    }

    fn playing_ident(&self) -> CardIdent<V> {
        self.acting()
            .unwrap_or_else(|| unimplemented!("reading the card being played with none played"))
            .face
            .ident
    }

    fn upgraded(&self) -> bool {
        self.acting()
            .unwrap_or_else(|| unimplemented!("the upgrade of no card being played"))
            .face
            .ident
            .upgrade
            != 0
    }

    fn transform(&mut self, card: CardAt<V>, into: CardIdent<V>) {
        let seat = self.seat();
        self.ran_on();
        // A new model for the replacement, so a new id. The original's own is
        // gone with it.
        let id = self.live.card_ids.mint();
        // `Transform` puts the replacement where the original stood, in the
        // pile it stood in. The place came with the name, so this looks once.
        let at = self.standing(card);
        self.live.players[seat]
            .cards
            .replace(card.pile(), at, CardInstance::new(into, id));
        // `AfterCardEnteredCombat` for the replacement, then
        // `AfterCardChangedPiles` from no pile.
        self.card_entered_combat(CardAt::new(id, card.pile(), card.index()), into.card, false);
        let hook = sts2_core::EventHook::CardChangedPiles {
            card: into.card,
            id,
            from: None,
        };
        if let Some((listener, effect)) =
            super::listen::first_answer::<V>(seat, self.live, self.belts, hook)
        {
            unimplemented!("{listener:?} asking for {effect:?} as a card changes piles")
        }
        // `CardCmd.Transform` raises this once every replacement is in, made
        // by the card's owner.
        let player = self.live.player(seat).id;
        self.card_generated(
            CardAt::new(id, card.pile(), card.index()),
            into.card,
            Some(player),
        );
    }

    fn transform_all(&mut self, cards: &[(CardAt<V>, CardIdent<V>)]) {
        let seat = self.seat();
        self.ran_on();
        // Each replacement is made before the command runs, so takes its id
        // in the order given. Every original then leaves, each noting where
        // it stood as it left.
        let ids: SmallVec<[CardId<V>; 4]> =
            cards.iter().map(|_| self.live.card_ids.mint()).collect();
        let mut left = SmallVec::<[_; 4]>::new();
        for (&(card, into), id) in cards.iter().zip(ids) {
            // An earlier one's leaving moved it, so it is found by its id.
            let at = self.live.players[seat]
                .cards
                .find(card.pile(), card.id())
                .unwrap_or_else(|| unimplemented!("transforming a card that left its pile"));
            self.live.players[seat].cards.remove(card.pile(), at);
            left.push((card.pile(), at, into, id));
        }
        if left.windows(2).any(|pair| pair[0].0 != pair[1].0) {
            unimplemented!("transforming cards from more than one pile at once");
        }
        left.sort_by_key(|&(_, at, _, _)| at);
        for &(pile, at, into, id) in &left {
            self.live.players[seat]
                .cards
                .insert(pile, at, CardInstance::new(into, id));
            let index = num_traits::NumCast::from(at).expect("a pile's index fits a hand index");
            self.card_entered_combat(CardAt::new(id, pile, index), into.card, false);
            let hook = sts2_core::EventHook::CardChangedPiles {
                card: into.card,
                id,
                from: None,
            };
            if let Some((listener, effect)) =
                super::listen::first_answer::<V>(seat, self.live, self.belts, hook)
            {
                unimplemented!("{listener:?} asking for {effect:?} as a card changes piles")
            }
        }
        // `AfterCardGeneratedForCombat` for each, once every replacement is
        // in, made by the card's owner.
        let player = self.live.player(seat).id;
        for (pile, at, into, id) in left {
            let index = num_traits::NumCast::from(at).expect("a pile's index fits a hand index");
            self.card_generated(CardAt::new(id, pile, index), into.card, Some(player));
        }
    }

    fn forge(&mut self, card: V::Card, amount: V::Number) {
        self.forge_by(card, amount, None);
    }

    fn apply_to_actor(&mut self, power: V::Power, amount: PowerAmount<V::Number>) {
        self.land(self.actor, power, amount, Some(self.actor));
    }

    fn apply_fraction_to_actor(&mut self, power: V::Power) {
        let zero = PowerAmount::Counted(V::Number::zero());
        self.land_fresh(self.actor, power, zero, Some(self.actor), None, None, true);
    }

    /// `OrbCmd.Channel`, which does nothing once the fight is ending.
    ///
    /// # Panics
    ///
    /// A listener to `AfterOrbChanneled` asking for anything but damage on
    /// every enemy.
    fn channel(&mut self, orb: V::Orb) {
        let seat = self.seat();
        self.ran_on();
        if self.over() {
            return;
        }
        let base = V::orb_slots(self.live.players[seat].character);
        let orbs = self.live.players[seat]
            .orbs
            .get_or_insert_with(|| sts2_core::Orbs {
                capacity: V::Count::zero(),
                queue: Vec::new(),
            });
        // `AddSlots(player, 1)`, for a player with no slots of their own.
        if base == 0 && orbs.capacity == V::Count::zero() {
            orbs.capacity = num_traits::one();
        }
        let full = num_traits::cast::<_, usize>(orbs.capacity)
            .is_some_and(|capacity| orbs.queue.len() >= capacity);
        if full {
            self.evoke_next(true);
        }
        // `TryEnqueue`, which refuses an orb with no slot to go in.
        let Some(orbs) = self.live.players[seat].orbs.as_mut() else {
            return;
        };
        if orbs.capacity == V::Count::zero() {
            return;
        }
        let Some(orbs) = self.live.players[seat].orbs.as_mut() else {
            return;
        };
        orbs.queue.push(V::channelled(orb));
        self.live.history.channelled(orb, seat.creature());
        let mut walking = super::listen::Walking::begin(
            self.live,
            self.belts,
            sts2_core::EventHook::OrbChanneled {
                player: seat.creature(),
            },
        );
        while let Some((listener, effect)) =
            walking.as_mut().and_then(|walking| self.next_of(walking))
        {
            match (listener, effect) {
                // `CreatureCmd.Damage` over `HittableEnemies`, unpowered and
                // from the relic's owner.
                (Listener::Relic { seat, .. }, HookEffect::DamageOpponents(amount)) => {
                    let player = seat.creature();
                    // One `CreatureCmd.Damage` over them all.
                    let targets = self.hittable_opponents_of(player);
                    self.damage_command(|board| {
                        for target in targets {
                            board.hit_unpowered(Some(player), target, amount, false);
                        }
                    });
                }
                (listener, effect) => {
                    unimplemented!("{listener:?} asking for {effect:?} as an orb is channelled")
                }
            }
        }
    }

    /// `OrbCmd.EvokeNext`, which does nothing once the fight is ending.
    ///
    /// # Panics
    ///
    /// A listener to `AfterOrbEvoked`.
    fn evoke_next(&mut self, dequeue: bool) {
        self.ran_on();
        self.evoke(false, dequeue);
    }

    /// `OstyCmd.Summon` for the player.
    ///
    /// A living Osty grows by `amount`, a dead one comes back on `amount`, and
    /// a player with none gains one.
    ///
    /// # Panics
    ///
    /// A listener to any hook the summon raises, or a summon for more health
    /// than a creature holds.
    fn summon_osty(&mut self, amount: V::Number) {
        let seat = self.seat();
        self.ran_on();
        use sts2_core::ModelHook;

        self.unheard(ModelHook::ModifySummonAmount);
        if amount == V::Number::zero() {
            return;
        }
        let health: V::Health = num_traits::cast(amount)
            .unwrap_or_else(|| unimplemented!("summoning Osty for {amount:?}"));
        match osty::<V>(&self.live.creatures, self.seat()).map(|osty| (osty.id, osty.alive)) {
            // `GainMaxHp`, which heals by what the maximum grew.
            Some((id, true)) => {
                if let Some(at) = self.at(id) {
                    let creature = &mut self.live.creatures[at];
                    creature.max_hp = creature.max_hp + health;
                }
                self.heal(id, amount);
            }
            // `SetMaxHp`, then a `Heal` that stands it back up.
            Some((id, false)) => {
                if let Some(at) = self.at(id) {
                    self.live.creatures[at].max_hp = health;
                }
                self.heal(id, amount);
                if let Some((listener, effect)) = super::listen::first_answer::<V>(
                    seat,
                    self.live,
                    self.belts,
                    sts2_core::EventHook::OstyRevived,
                ) {
                    unimplemented!("{listener:?} asking for {effect:?} as Osty revives")
                }
            }
            // `AddPet<Osty>`, which rolls nothing for a creature on the
            // player's side. Then `DieForYouPower`, `SetMaxHp` and `Heal`.
            None => {
                let id = self.add_pet(<V::Monster as sts2_core::MonsterKinds>::OSTY);
                self.land(
                    id,
                    <V::Power as sts2_core::PowerKinds>::DIE_FOR_YOU,
                    PowerAmount::Counted(num_traits::one()),
                    None,
                );
                if let Some(at) = self.at(id) {
                    let creature = &mut self.live.creatures[at];
                    creature.max_hp = health;
                    creature.hp = creature.hp.min(health);
                }
                self.heal(id, amount);
            }
        }
        self.unheard(ModelHook::AfterSummon);
    }

    fn osty(&self) -> Option<CombatId> {
        osty::<V>(&self.live.creatures, self.seat())
            .filter(|osty| osty.alive)
            .map(|osty| osty.id)
    }

    fn orbs_held(&self) -> usize {
        let seat = self.seat();
        self.live.players[seat]
            .orbs
            .as_ref()
            .map_or(0, |orbs| orbs.queue.len())
    }

    fn channelled(&self) -> Vec<sts2_core::Channelled<V>> {
        let seat = self.seat();
        self.live.players[seat]
            .orbs
            .as_ref()
            .map_or_else(Vec::new, |orbs| orbs.queue.clone())
    }

    /// `IsHittable`: alive, and no listener refusing a hit on it.
    fn random_opponent(&mut self) -> Option<CombatId> {
        use sts2_core::rng::GameRng as _;

        let hittable = self.hittable_opponents();
        self.rngs.targets.choose(&hittable).ok().copied()
    }

    fn discard_and_draw(&mut self, cards: &[CardAt<V>], draw: V::Number) {
        let seat = self.seat();
        self.ran_on();
        if cards.is_empty() || self.over() {
            return;
        }
        let (sly, ended) = super::turn::act_on::<V>(
            seat,
            self.live,
            self.rngs.reborrow(),
            self.belts,
            sts2_core::PileAct::Discard,
            cards,
            false,
        );
        self.ended = self.ended.or(ended);
        if !self.stopped() && draw > V::Number::zero() {
            sts2_core::Fight::draw(self, draw);
        }
        if !self.stopped() {
            let pausable = self.pausable();
            self.ended = super::turn::play_sly::<V>(
                seat,
                self.live,
                self.rngs.reborrow(),
                self.belts,
                sly,
                pausable,
            );
        }
    }

    fn card_costs(&self, card: CardAt<V>, global: bool) -> (sts2_core::Cost, sts2_core::Cost) {
        let seat = self.seat();
        let at = self.standing(card);
        let held = self.live.players[seat]
            .cards
            .get(card.pile(), at)
            .expect("a card stands where it was found");
        match global {
            false => held.face.costs(|cost| cost, |cost| cost),
            true => super::legal::costs::<V>(
                seat,
                &held,
                Some(card.pile()),
                super::listen::Walk {
                    live: self.live,
                    belts: self.belts,
                },
            ),
        }
    }

    fn printed_costs(&self, card: CardAt<V>) -> (sts2_core::Cost, sts2_core::Cost) {
        let seat = self.seat();
        let at = self.standing(card);
        let held = self.live.players[seat]
            .cards
            .get(card.pile(), at)
            .expect("a card stands where it was found");
        (held.face.cost, held.face.star_cost)
    }

    fn card_keyword(&self, card: CardAt<V>, keyword: V::CardKeyword) -> bool {
        let seat = self.seat();
        let at = self.standing(card);
        self.live.players[seat]
            .cards
            .get(card.pile(), at)
            .expect("a card stands where it was found")
            .face
            .has_keyword(keyword)
    }

    fn card_keyword_this_turn(&self, card: CardAt<V>, keyword: V::CardKeyword) -> bool {
        let seat = self.seat();
        let at = self.standing(card);
        self.live.players[seat]
            .cards
            .get(card.pile(), at)
            .expect("a card stands where it was found")
            .face
            .has_keyword_this_turn(keyword)
    }

    fn card_local_keyword(&self, card: CardAt<V>, keyword: V::CardKeyword) -> bool {
        let seat = self.seat();
        let at = self.standing(card);
        self.live.players[seat]
            .cards
            .get(card.pile(), at)
            .expect("a card stands where it was found")
            .face
            .has_local_keyword(keyword)
    }

    fn local_cost(&self, card: CardAt<V>) -> sts2_core::Cost {
        let seat = self.seat();
        let at = self.standing(card);
        self.live.players[seat]
            .cards
            .get(card.pile(), at)
            .expect("a card stands where it was found")
            .face
            .costs(|cost| cost, |cost| cost)
            .0
    }

    fn random_card_in(&mut self, pile: Pile) -> Option<CardAt<V>> {
        use sts2_core::rng::GameRng as _;

        let cards = sts2_core::Fight::cards_in(self, pile);
        self.rngs.selection.choose(&cards).ok().copied()
    }

    fn auto_play_from_draw(&mut self, count: usize, force_exhaust: bool) {
        let seat = self.seat();
        if self.over() {
            return;
        }
        let pausable = self.pausable();
        let ascension = self.ascension;
        let ended = auto_play_from_draw::<V>(
            seat,
            self.live,
            self.rngs.reborrow(),
            self.belts,
            ascension,
            AutoPlayFromDraw {
                left: count,
                taken: SmallVec::new(),
                force_exhaust,
            },
            pausable,
            false,
        );
        if ended.is_some() {
            self.ended = ended;
        }
    }

    fn add_orb_slots(&mut self, amount: V::Number) {
        let seat = self.seat();
        self.ran_on();
        if self.over() {
            return;
        }
        let orbs = self.live.players[seat]
            .orbs
            .get_or_insert_with(|| sts2_core::Orbs {
                capacity: V::Count::zero(),
                queue: Vec::new(),
            });
        let capacity: V::Number =
            num_traits::cast(orbs.capacity).expect("an orb capacity fits a game number");
        let most: V::Number = num_traits::cast(MAX_ORB_SLOTS).expect("ten fits a game number");
        let added = amount.min(most - capacity);
        orbs.capacity = num_traits::cast(capacity + added).expect("an orb capacity fits a count");
    }

    fn remove_orb_slots(&mut self, amount: V::Number) {
        let seat = self.seat();
        if self.over() {
            return;
        }
        let Some(orbs) = self.live.players[seat].orbs.as_mut() else {
            return;
        };
        let capacity: V::Number =
            num_traits::cast(orbs.capacity).expect("an orb capacity fits a game number");
        let left = (capacity - amount.min(capacity)).max(V::Number::zero());
        orbs.capacity = num_traits::cast(left).expect("an orb capacity fits a count");
        let kept: usize = num_traits::cast(left).expect("an orb capacity fits a length");
        orbs.queue.truncate(kept);
    }

    fn damage_unblockable(&mut self, target: CombatId, amount: V::Number) {
        let seat = self.seat();
        self.ran_on();
        let dealer = Some(self.actor);
        let damage = modified::<V>(seat, self.walk(), dealer, Some(target), None, amount, false);
        let card = self
            .playing
            .and_then(|id| self.live.players[seat].cards.named(id))
            .map(|held| held.face.ident.card);
        self.hit(dealer, target, damage, false, true, card, false);
    }

    fn lose_block(&mut self, creature: CombatId, amount: V::Number, remover: Option<CombatId>) {
        self.ran_on();
        let zero = V::Number::zero();
        if self.over() || amount <= zero {
            return;
        }
        let Some(at) = self.at(creature) else { return };
        let held = &mut self.live.creatures[at];
        if !held.alive {
            return;
        }
        let block = held.block;
        held.block = (block - amount).max(zero);
        if block > zero && held.block <= zero {
            let pet_of = remover
                .and_then(|remover| self.at(remover))
                .and_then(|at| self.live.creatures[at].owner);
            let walking = super::listen::Walking::begin(
                self.live,
                self.belts,
                sts2_core::EventHook::BlockBroken {
                    creature,
                    breaker: remover,
                    pet_of,
                },
            );
            self.after_hit(walking);
        }
    }

    fn card_number(&self, card: CardAt<V>, key: V::CardVar) -> Decimal {
        let seat = self.seat();
        let at = self.standing(card);
        let held = self.live.players[seat]
            .cards
            .get(card.pile(), at)
            .expect("a card stands where it was found");
        exact_var::<V>(seat, &held, key, self.walk(), None)
    }

    fn card_damage(&self, card: CardAt<V>, damage: Decimal) -> Decimal {
        let seat = self.seat();
        let at = self.standing(card);
        let held = self.live.players[seat]
            .cards
            .get(card.pile(), at)
            .expect("a card stands where it was found");
        let player = self.live.player(seat).id;
        modified_exactly::<V>(
            seat,
            self.walk(),
            Some(player),
            None,
            Some(&held),
            damage,
            true,
        )
    }

    fn card_replays(&self, card: CardAt<V>) -> V::Number {
        let seat = self.seat();
        let at = self.standing(card);
        let held = self.live.players[seat]
            .cards
            .get(card.pile(), at)
            .expect("a card stands where it was found");
        held.face.play_count() - num_traits::one()
    }

    fn over_or_ending(&self) -> bool {
        self.over()
    }

    fn card_target_type(&self, card: CardIdent<V>) -> V::TargetType {
        super::legal::target_type::<V>(card, self.live)
    }

    fn auto_play_card(&mut self, card: CardAt<V>) {
        let seat = self.seat();
        if self.over() || !self.live.player(seat).alive {
            return;
        }
        self.standing(card);
        let pausable = self.pausable();
        self.ended = super::play::auto_play::<V>(
            seat,
            self.live,
            self.rngs.reborrow(),
            self.belts,
            card.id(),
            card.pile(),
            None,
            pausable,
        );
    }

    fn auto_play_departed(&mut self, id: CardId<V>) {
        let seat = self.seat();
        use sts2_core::{CardTable as _, TargetKinds as _};

        if self.over() || !self.live.player(seat).alive {
            return;
        }
        let Some(card) = self.live.players[seat].cards.last_state(id) else {
            unimplemented!("auto-playing {id:?}, which the fight never held");
        };
        let ident = card.face.ident;
        if ident.card.unplayable() {
            unimplemented!("auto-playing {ident:?}, unplayable, after it left the fight");
        }
        // `Hook.ShouldPlay`, with the auto-play's type.
        if super::listen::first_deciding::<V>(
            super::listen::Walk {
                live: self.live,
                belts: self.belts,
            },
            sts2_core::DecisionHook::ShouldPlay {
                card: ident.card,
                player: self.seat().creature(),
                affliction: ident.affliction,
                auto: true,
            },
            false,
        )
        .is_some()
        {
            unimplemented!("{ident:?} refused an auto-play after it left the fight");
        }
        let kind = super::legal::target_type::<V>(ident, self.live);
        if kind == V::TargetType::ANY_ENEMY || kind == V::TargetType::ANY_ALLY {
            unimplemented!("auto-playing {ident:?}, which takes a target, after it left the fight");
        }
        // `CardPileCmd.RemoveFromCombat` ends on `RemoveFromState`, so
        // `CardPileCmd.Add` refuses the card. With no pile it has no combat
        // state, and `OnPlayWrapper` returns before it plays.
        super::turn::unheard::<V>(
            self.live,
            self.belts,
            sts2_core::ModelHook::BeforeCardAutoPlayed,
        );
    }

    fn auto_play_card_at(&mut self, card: CardAt<V>, target: CombatId) {
        let seat = self.seat();
        if self.over() || !self.live.player(seat).alive {
            return;
        }
        self.standing(card);
        let pausable = self.pausable();
        self.ended = super::play::auto_play::<V>(
            seat,
            self.live,
            self.rngs.reborrow(),
            self.belts,
            card.id(),
            card.pile(),
            Some(target),
            pausable,
        );
    }

    fn auto_play_card_at_random(&mut self, card: CardAt<V>) {
        let seat = self.seat();
        if self.over() || !self.live.player(seat).alive {
            return;
        }
        self.standing(card);
        // A question an earlier play asked stands this one still, target
        // and all, until the question is answered.
        if self.live.asking.is_some() {
            if !self.pausable() {
                unimplemented!("auto-playing {card:?} while a question waits outside an effect");
            }
            self.live
                .defer(super::live::Deferred::AutoPlayAtRandom { id: card.id() });
            return;
        }
        let target = sts2_core::Fight::random_opponent(self);
        let pausable = self.pausable();
        self.ended = super::play::auto_play::<V>(
            seat,
            self.live,
            self.rngs.reborrow(),
            self.belts,
            card.id(),
            card.pile(),
            target,
            pausable,
        );
    }

    fn stable_shuffle_cards(&mut self, cards: &mut [CardAt<V>], stream: V::RunRngType) {
        let seat = self.seat();
        use sts2_core::rng::GameRng as _;

        let rng = match stream {
            stream if stream == V::RunRngType::SHUFFLE => &mut self.rngs.shuffle,
            stream if stream == V::RunRngType::COMBAT_CARD_SELECTION => &mut self.rngs.selection,
            stream => unimplemented!("cards in the fight shuffled off {stream:?}"),
        };
        let cards_held = &self.live.players[seat].cards;
        // `CardModel.CompareTo`: its `ModelId`, then its upgrade.
        rng.stable_shuffle_by_key(cards, |&card| {
            let at: usize =
                num_traits::NumCast::from(card.index()).expect("a pile index fits this machine");
            let held = cards_held
                .get(card.pile(), at)
                .expect("a card stands where it was found");
            (held.face.ident.card.order(), held.face.ident.upgrade)
        });
    }

    fn generate_play_count(&mut self, card: CardId<V>) -> V::Number {
        let seat = self.seat();
        let held = self.live.players[seat]
            .cards
            .named(card)
            .unwrap_or_else(|| unimplemented!("a play count for a card that left the fight"));
        let target = self.target;
        super::play::play_count::<V, _>(seat, self.live, &mut self.rngs, self.belts, &held, target)
    }

    fn next_energy_cost(&mut self, below: u8) -> V::Number {
        use sts2_core::rng::GameRng as _;

        let bound = sts2_core::rng::Bound::try_new(
            num_traits::NumCast::from(below).expect("a cost bound fits the game's RNG integer"),
        )
        .expect("a random cost has somewhere to land");
        let cost = self.rngs.energy_costs.next_int(bound);
        num_traits::NumCast::from(cost).expect("an energy cost fits a game number")
    }

    /// `CapturedXValue` is what the play spent, which the history's latest
    /// started play records: the one being resolved.
    fn x_value(&self) -> V::Number {
        let started = self
            .live
            .history(self.actor)
            .started_this_turn()
            .last()
            .expect("an X-cost card resolving its X is a play started this turn");
        let x = num_traits::cast(started.x).expect("an energy fits a game number");
        modified_by::<V>(self.walk(), x, &[ModifyHook::XValue { player: self.actor }])
    }

    /// `LastStarsSpent` is what the play spent, which the history's latest
    /// started play records: the one being resolved.
    fn star_x_value(&self) -> V::Number {
        let started = self
            .live
            .history(self.actor)
            .started_this_turn()
            .last()
            .expect("an X star cost resolving its X is a play started this turn");
        let x = num_traits::cast(started.stars).expect("a star count fits a game number");
        modified_by::<V>(self.walk(), x, &[ModifyHook::XValue { player: self.actor }])
    }

    fn random_target(&mut self) -> Option<CombatId> {
        use sts2_core::rng::GameRng as _;

        if self.hits_wait() {
            unimplemented!("a random target drawn while an attack's hits wait on a question");
        }
        let alive = sts2_core::Fight::opponents(self);
        self.rngs.targets.choose(&alive).ok().copied()
    }

    /// `CreatureCmd.Heal`, which is `HealInternal` and the revival it reports.
    ///
    /// `SetCurrentHpInternal` caps at the maximum, and a creature whose health
    /// crosses back over zero is alive again. A fight that is already ending
    /// heals no monster, which is what leaves a segment killed with the last
    /// of them dead.
    fn heal(&mut self, at: CombatId, amount: V::Number) {
        self.ran_on();
        // `CombatManager.IsEnding` heals no creature but a player. A hit
        // that took the last enemy to nothing has the fight ending already,
        // so Sic 'Em's summon then grows Osty without healing it, and Feed's
        // kill still heals its owner.
        let player = super::turn::seats(self.live).any(|seat| seat.creature() == at);
        if !player && self.ending() {
            return;
        }
        let Some(creature) = self.live.creatures.iter_mut().find(|c| c.id == at) else {
            return;
        };
        let healed: V::Health = num_traits::cast(amount)
            .unwrap_or_else(|| unimplemented!("healing by {amount:?}, which is not a health"));
        creature.hp = (creature.hp + healed).min(creature.max_hp);
        creature.alive = creature.hp > V::Health::zero();
        // `Hook.AfterCurrentHpChanged` with the amount asked for, however much
        // of it the maximum let land.
        if amount > V::Number::zero() {
            self.current_hp_changed(at, amount);
        }
    }

    fn model<R>(&mut self, with: impl FnOnce(&mut V::MonsterState) -> R) -> R {
        let actor = self.actor;
        match self
            .live
            .machines
            .iter_mut()
            .find(|held| held.creature == actor)
        {
            Some(held) => with(&mut held.state),
            None => match &mut self.left {
                Some(left) => with(left),
                None => unreachable!("a model read outside its monster's move"),
            },
        }
    }

    fn enemy_models(&mut self, mut with: impl FnMut(&mut V::MonsterState)) {
        // Every creature with a move graph is an enemy.
        for held in self.live.machines.iter_mut() {
            with(&mut held.state);
        }
    }

    fn write_power(
        &mut self,
        at: CombatId,
        power: V::Power,
        write: impl FnOnce(&mut V::PowerState),
    ) {
        // The instance applied last, which for an instanced power is the one
        // just built.
        let held = self
            .live
            .powers
            .of_mut(at)
            .filter(|held| held.power() == power)
            .last();
        if let Some(held) = held {
            write(&mut held.state);
        }
    }

    fn give_cards(&mut self, card: V::Card, count: usize) {
        self.ran_on();
        // Every opponent takes its own copies, into its own discard pile, one
        // player after another.
        for opponent in self.opponents() {
            let seat = self.live.seat_of(opponent);
            let was = std::mem::replace(&mut self.reaching, seat);
            self.add_generated_cards(CardIdent::new(card), Pile::Discard, count);
            self.reaching = was;
        }
    }

    fn character(&self) -> V::Character {
        let seat = self.seat();
        self.live.players[seat].character
    }

    fn multiplayer(&self) -> bool {
        self.live.players.len() > 1
    }

    fn generate_card(&mut self, cards: &[V::Card]) -> Option<V::Card> {
        self.ran_on();
        use sts2_core::rng::GameRng as _;

        self.rngs.generation.choose(cards).ok().copied()
    }

    fn generate_cards<const N: usize>(
        &mut self,
        cards: &mut [V::Card],
        stream: V::RunRngType,
    ) -> Result<[CardIdent<V>; N], sts2_core::rng::Error> {
        if stream != V::RunRngType::COMBAT_CARD_GENERATION {
            unimplemented!("cards generated from {stream:?}")
        }
        self.rngs
            .generation
            .take_random::<_, N>(cards)
            .map(|taken| taken.map(CardIdent::new))
    }

    fn shuffle(&mut self) {
        let seat = self.seat();
        // `IsOverOrEnding`, which a fight already over is.
        if self.stopped() {
            return;
        }
        let ascension = self.ascension;
        if !self.pausable() {
            self.ended = shuffle::<V>(seat, self.live, self.rngs.reborrow(), self.belts, ascension);
            return;
        }
        if self.live.asking.is_some() {
            unimplemented!("a shuffle behind a question an effect already asked");
        }
        // Inside an effect, a listener's question stands the effect still.
        let (pile, selection, act) =
            match shuffle_asking::<V>(seat, self.live, self.rngs.reborrow(), self.belts, ascension)
            {
                Ok(Some(asks)) => asks,
                Ok(None) => return,
                Err(ended) => {
                    self.ended = Some(ended);
                    return;
                }
            };
        match super::turn::select::<V>(
            seat,
            self.live,
            self.rngs.reborrow(),
            self.belts,
            pile,
            selection,
            act,
        ) {
            // The pick needed no answer, so the walk goes on.
            Ok(ended) => {
                self.ended = self.ended.or(ended);
                if let Some(waiting) = self.live.after_pick.take()
                    && let Some(asks) = after_shuffle_walk::<V>(self, *waiting)
                {
                    unimplemented!("a second listener asking for {asks:?} after a shuffle");
                }
            }
            // The listeners behind the pick wait for its answer.
            Err(question) => {
                self.live.asking = Some(super::live::Asking {
                    then: super::live::Then::Pick { act },
                    question,
                    signalled: false,
                });
            }
        }
    }

    fn clone_this(&mut self, pile: Pile) -> Option<CardAt<V>> {
        // `IsInProgress`, which a fight already over fails.
        if self.stopped() {
            return None;
        }
        let playing = self
            .playing
            .unwrap_or_else(|| unimplemented!("a card cloning itself with none being played"));
        self.clone_named(playing, pile)
    }

    fn clone_card(&mut self, card: CardAt<V>, pile: Pile) -> Option<CardAt<V>> {
        self.ran_on();
        if self.stopped() {
            return None;
        }
        self.clone_named(card.id(), pile)
    }

    fn orb_capacity(&self) -> usize {
        let seat = self.seat();
        self.live.players[seat].orbs.as_ref().map_or(0, |orbs| {
            num_traits::cast(orbs.capacity).expect("an orb capacity fits a length")
        })
    }

    fn orb_passive_at(&mut self, place: usize) {
        let seat = self.seat();
        if self.over() {
            return;
        }
        let Some(mut orb) = self.live.players[seat]
            .orbs
            .as_ref()
            .and_then(|orbs| orbs.queue.get(place).copied())
        else {
            return;
        };
        let playing = self.playing.take();
        V::orb_passive(&mut orb, self);
        self.playing = playing;
        if let Some(held) = self.live.players[seat]
            .orbs
            .as_mut()
            .and_then(|orbs| orbs.queue.get_mut(place))
            && held.orb == orb.orb
        {
            *held = orb;
        }
    }

    fn add_card(&mut self, card: CardIdent<V>, pile: Pile) -> Option<CardAt<V>> {
        self.ran_on();
        // `IsInProgress`, which a fight already over fails.
        if self.stopped() {
            return None;
        }
        Some(self.put(card, pile))
    }

    fn add_generated_cards(
        &mut self,
        card: CardIdent<V>,
        pile: Pile,
        count: usize,
    ) -> Vec<CardAt<V>> {
        self.ran_on();
        let creator = self.creator();
        self.generate(card, pile, count, creator)
    }

    fn cards_in(&self, pile: Pile) -> Vec<CardAt<V>> {
        let seat = self.seat();
        self.live.players[seat]
            .cards
            .iter(pile)
            .enumerate()
            .map(|(at, held)| {
                let at = num_traits::NumCast::from(at)
                    .unwrap_or_else(|| unimplemented!("a pile of more cards than a fight counts"));
                CardAt::new(held.id, pile, at)
            })
            .collect()
    }

    fn card_ident(&self, card: CardAt<V>) -> CardIdent<V> {
        let seat = self.seat();
        let at = self.standing(card);
        self.live.players[seat]
            .cards
            .get(card.pile(), at)
            .expect("a card stands where it was found")
            .face
            .ident
    }

    fn random_card(&mut self, cards: &[CardAt<V>], stream: V::RunRngType) -> Option<CardAt<V>> {
        if cards.is_empty() {
            return None;
        }
        let rng = match stream {
            stream if stream == V::RunRngType::COMBAT_CARD_GENERATION => &mut self.rngs.generation,
            stream if stream == V::RunRngType::COMBAT_CARD_SELECTION => &mut self.rngs.selection,
            stream => unimplemented!("a combat card selected from {stream:?}"),
        };
        let bound = sts2_core::rng::Bound::try_new(
            num_traits::NumCast::from(cards.len())
                .expect("a combat pile's length fits the game's RNG integer"),
        )
        .expect("a nonempty candidate list has a nonzero bound");
        let at: usize = num_traits::NumCast::from(rng.next_int(bound))
            .expect("a bounded draw is a candidate index");
        Some(cards[at])
    }

    fn take_random_cards(&mut self, cards: &mut [CardAt<V>], count: usize) -> Vec<CardAt<V>> {
        self.ran_on();
        use sts2_core::rng::GameRng as _;

        self.rngs.selection.unstable_shuffle(cards);
        cards.iter().take(count).copied().collect()
    }

    fn remove_card(&mut self, card: CardAt<V>) -> CardIdent<V> {
        let seat = self.seat();
        self.ran_on();
        let at = self.standing(card);
        let ident = self.live.players[seat]
            .cards
            .remove(card.pile(), at)
            .face
            .ident;
        if let Some(deck) = self.live.players[seat].deck_versions.of(card.id()) {
            self.live.players[seat].removed_from_deck.push(deck);
        }
        ident
    }

    fn deck_version(&self, card: sts2_core::CardId<V>) -> Option<sts2_core::DeckCardId> {
        let seat = self.seat();
        self.live.players[seat].deck_versions.of(card)
    }

    fn write_deck_version(&mut self, write: V::DeckWrite) {
        let seat = self.seat();
        self.ran_on();
        let playing = self
            .playing
            .unwrap_or_else(|| unimplemented!("a deck version written with no card being played"));
        if let Some(card) = self.live.players[seat].deck_versions.of(playing) {
            self.live
                .for_run
                .push((seat, HookEffect::WriteDeckCard { card, write }));
        }
    }

    fn modify_card(
        &mut self,
        card: CardAt<V>,
        change: sts2_core::Change<V>,
        lasting: sts2_core::Lasting,
    ) {
        let seat = self.seat();
        let at = self.standing(card);
        self.live.players[seat]
            .cards
            .write(card.pile(), at, |held| {
                held.modify(change, lasting);
                true
            });
    }

    fn act_on(&mut self, act: sts2_core::PileAct, cards: &[CardAt<V>]) {
        let seat = self.seat();
        // A question a hook of an earlier command asked stands this one still
        // inside an effect, as it does a draw.
        if self.live.asking.is_some() && self.pausable() {
            self.live.defer(super::live::Deferred::ActOn {
                act,
                cards: cards.iter().copied().collect(),
            });
            return;
        }
        self.ran_on();
        let pausable = self.pausable();
        let (sly, ended) = super::turn::act_on::<V>(
            seat,
            self.live,
            self.rngs.reborrow(),
            self.belts,
            act,
            cards,
            pausable,
        );
        self.ended = self.ended.or(ended);
        if !self.stopped() {
            let pausable = self.pausable();
            self.ended = super::turn::play_sly::<V>(
                seat,
                self.live,
                self.rngs.reborrow(),
                self.belts,
                sly,
                pausable,
            );
        }
    }

    fn modify_this(&mut self, change: sts2_core::Change<V>, lasting: sts2_core::Lasting) {
        let seat = self.seat();
        self.ran_on();
        let playing = self
            .playing
            .unwrap_or_else(|| unimplemented!("a card writing on itself with none being played"));
        self.live.modify_self(seat, playing, change, lasting);
    }

    fn hittable_opponents(&self) -> sts2_core::Opponents {
        Board::hittable_opponents(self).into_iter().collect()
    }

    fn add_this(&mut self, pile: Pile, position: sts2_core::PilePosition) {
        let seat = self.seat();
        self.ran_on();
        let playing = self
            .playing
            .unwrap_or_else(|| unimplemented!("a card moving itself with none being played"));
        if pile == Pile::Hand {
            unimplemented!("a card putting itself into the hand");
        }
        // A replay of a card its first play already moved finds it outside
        // the play pile, and moves it from there.
        if self.live.players[seat].cards.in_play(playing) {
            self.live.players[seat]
                .cards
                .leave_play(playing, Some(pile), position);
        } else {
            self.live.players[seat]
                .cards
                .move_named(playing, pile, position);
        }
    }

    fn stun(
        &mut self,
        target: CombatId,
        stun: &'static sts2_core::NextMove<V>,
        then: sts2_core::StunResume,
    ) {
        self.stun_monster(target, stun, then);
    }

    fn this_has_keyword(&self, keyword: V::CardKeyword) -> bool {
        self.acting()
            .unwrap_or_else(|| unimplemented!("the keywords of no card being played"))
            .face
            .has_keyword(keyword)
    }

    fn create_monster(&mut self, monster: V::Monster) -> CombatId {
        let model = sts2_core::MonsterModel::new(monster);
        let (id, hp) = super::monster::create::<V>(self.live, monster, &model, self.rngs.niche);
        self.live.created.push(super::live::Created {
            fighter: Fighter {
                id,
                model: Some(sts2_core::ModelId::Monster(monster)),
                side: <V::CombatSide as SideKinds>::ENEMY,
                hp,
                max_hp: hp,
                block: V::Number::zero(),
                alive: true,
                next_move: None,
                owner: None,
            },
            monster,
            powers: Vec::new(),
        });
        id
    }

    fn add_created(&mut self, created: CombatId, position: usize) {
        let at = self
            .live
            .created
            .iter()
            .position(|held| held.fighter.id == created)
            .unwrap_or_else(|| panic!("{created:?} was never made"));
        let super::live::Created {
            fighter,
            monster,
            powers,
        } = self.live.created.remove(at);
        super::monster::place::<V>(
            self.live,
            monster,
            None,
            None,
            sts2_core::MonsterModel::new(monster),
            position,
            created,
            fighter.hp,
        );
        for held in powers {
            self.live.powers.push(created, held);
        }
        self.arrived(created, monster);
        self.roll_joined(created);
        self.enemy_added(created);
    }

    fn summon(&mut self, monster: V::Monster, position: usize) -> CombatId {
        let id = super::monster::summon(self.live, monster, position, None, self.rngs.niche);
        self.arrived(id, monster);
        self.roll_joined(id);
        self.enemy_added(id);
        id
    }

    fn summon_into_last_free_slot(&mut self, monster: V::Monster) -> Option<CombatId> {
        self.ran_on();
        use sts2_core::EncounterTable as _;

        // A slot counts as held by an enemy still in the fight, and only the
        // living keep a move graph.
        let held: SmallVec<[u8; 8]> = self
            .live
            .machines
            .iter()
            .filter(|held| {
                self.live
                    .creatures
                    .iter()
                    .any(|creature| creature.id == held.creature && creature.alive)
            })
            .filter_map(|held| held.slot)
            .collect();
        let slot = (0..self.live.encounter.slots())
            .rev()
            .find(|slot| !held.contains(slot))?;
        Some(self.summon_into_slot(monster, slot))
    }

    fn summon_into_first_free_slot(&mut self, monster: V::Monster) -> Option<CombatId> {
        self.ran_on();
        use sts2_core::EncounterTable as _;

        let held: SmallVec<[u8; 8]> = self
            .live
            .machines
            .iter()
            .filter(|held| {
                self.live
                    .creatures
                    .iter()
                    .any(|creature| creature.id == held.creature)
            })
            .filter_map(|held| held.slot)
            .collect();
        let slot = (0..self.live.encounter.slots()).find(|slot| !held.contains(slot))?;
        Some(self.summon_into_slot(monster, slot))
    }

    fn monster_ai_choose<T: Copy>(&mut self, items: &[T]) -> Option<T> {
        use sts2_core::rng::GameRng as _;

        self.rngs.ai.choose(items).ok().copied()
    }

    fn summon_into_slot(&mut self, monster: V::Monster, slot: u8) -> CombatId {
        self.ran_on();
        // `SortEnemiesBySlotName`: after every enemy in an earlier slot.
        let position = self
            .live
            .machines
            .iter()
            .filter(|held| held.slot.is_some_and(|held| held < slot))
            .count();
        let id = super::monster::summon(self.live, monster, position, Some(slot), self.rngs.niche);
        self.arrived(id, monster);
        self.enemy_added(id);
        id
    }
}

/// A selection a listener asks for after a shuffle: from `pile`, as many as
/// the selection allows, and what happens to them.
pub(super) type ShuffleAsks = (Pile, sts2_core::Selection, sts2_core::PileAct);

/// `CardPileCmd.Shuffle`: the discard pile shuffled into the draw pile, then
/// `ModifyShuffleOrder` over the result.
///
/// # Panics
///
/// A listener asking the player something after the shuffle, which only a
/// draw that can pause carries out.
#[must_use = "a listener after the shuffle can end the fight"]
pub(super) fn shuffle<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut Belts<V>,
    ascension: V::AscensionLevel,
) -> Option<super::Ended> {
    match shuffle_asking::<V>(seat, live, rngs, belts, ascension) {
        Ok(None) => None,
        Ok(Some(asks)) => {
            unimplemented!("a shuffle asking for {asks:?} inside a draw that cannot pause")
        }
        Err(ended) => Some(ended),
    }
}

/// `Hook.AfterEnergySpent`, once a card's `SpendEnergy` took `amount` off the
/// player's pool. How what its listeners asked for ended the fight, if it
/// did.
pub(super) fn energy_spent<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut Belts<V>,
    amount: V::Number,
) -> Option<super::Ended> {
    if !super::listen::heard::<V>(live, belts, sts2_core::ModelHook::AfterEnergySpent) {
        return None;
    }
    let walking = super::listen::Walking::begin(
        live,
        belts,
        sts2_core::EventHook::EnergySpent {
            player: seat.creature(),
            amount,
        },
    );
    let ascension = live.ascension;
    let mut board = Board::for_card(seat, live, rngs, belts, ascension, None, None);
    board.carry_out_walk(walking);
    board.ended()
}

/// `CreatureCmd.GainBlock` of `amount`, unpowered, on `owner` from no play
/// and outside any effect, then `Hook.AfterBlockGained`.
pub(super) fn gain_unpowered_block<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut Belts<V>,
    ascension: V::AscensionLevel,
    owner: CombatId,
    amount: V::Number,
) {
    Board::for_card(seat, live, rngs, belts, ascension, None, None).gain_unpowered(owner, amount);
}

/// [`shuffle`], handing back the selection a listener asks for after it,
/// which is `CardSelectCmd.FromCombatPile` inside `AfterShuffle`. Or how a
/// listener's command after it ended the fight, as Smokestack's damage over
/// a Status card one adds can.
///
/// # Panics
///
/// A listener asking a question after another ended the fight.
pub(super) fn shuffle_asking<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut Belts<V>,
    ascension: V::AscensionLevel,
) -> Result<Option<ShuffleAsks>, super::Ended> {
    // `Hook.ModifyShuffleOrder` walks its listeners while the cards still
    // stand in the piles they are shuffled from, so the order they move in
    // is that walk's. Each moves its card in the list once it is shuffled.
    let hook = sts2_core::EventHook::ShuffleOrder { initial: false };
    let mut asked: SmallVec<[(Listener<V>, HookEffect<V>); 2]> = SmallVec::new();
    let mut walking = super::listen::Walking::begin(live, belts, hook);
    while let Some(answer) = walking
        .as_mut()
        .and_then(|walking| walking.next(seat, live, belts))
    {
        asked.push(answer);
    }
    live.players[seat].cards.reshuffle(rngs.shuffle);
    for (listener, effect) in asked {
        match (listener, effect) {
            // The shuffled list is the discard and draw piles, and a card
            // anywhere else stays put.
            (Listener::Card { id }, HookEffect::ToTopOfDrawPile) => {
                if let Some(at) = live.players[seat].cards.find(Pile::Draw, id) {
                    live.players[seat].cards.move_to_top(Pile::Draw, at);
                }
            }
            (listener, effect) => {
                unimplemented!("{listener:?} asking for {effect:?} over a shuffle")
            }
        }
    }
    // `Hook.AfterShuffle`.
    let hook = sts2_core::EventHook::Shuffled {
        player: seat.creature(),
    };
    let Some(walking) = super::listen::Walking::begin(live, belts, hook) else {
        return Ok(None);
    };
    let mut board = Board::for_card(seat, live, rngs, belts, ascension, None, None);
    let asks = after_shuffle_walk::<V>(&mut board, walking);
    match (board.ended(), asks) {
        (None, asks) => Ok(asks),
        (Some(ended), None) => Err(ended),
        (Some(ended), Some(asks)) => {
            unimplemented!("a shuffle asking for {asks:?} in a fight that ended {ended:?}")
        }
    }
}

/// The rest of an `AfterShuffle` walk, carried out on `board` until a
/// listener asks the player to pick, and that pick. The listeners after it
/// wait for the pick in [`Live::after_pick`], unasked, as the game awaits
/// the selection before it goes on.
///
/// # Panics
///
/// A listener asking for anything else after a shuffle.
fn after_shuffle_walk<V: Rules>(
    board: &mut Board<'_, V>,
    mut walking: super::listen::Walking<V>,
) -> Option<ShuffleAsks> {
    while let Some((listener, effect)) = board.next_of(&mut walking) {
        match effect {
            HookEffect::SelectFromPile {
                pile,
                selection,
                act,
            } => {
                board.live.after_pick = Some(Box::new(walking));
                return Some((pile, selection, act));
            }
            effect @ HookEffect::AddToDrawAtRandom { .. } => {
                board.as_listener(listener, |board| board.after_shuffle(effect))
            }
            // `CreatureCmd.GainBlock`, unpowered, on a relic's owner.
            effect @ HookEffect::GainBlock(_) if matches!(listener, Listener::Relic { .. }) => {
                board.as_listener(listener, |board| board.after_shuffle(effect))
            }
            effect => unimplemented!("{listener:?} asking for {effect:?} after a shuffle"),
        }
    }
    None
}

impl<V: Rules> Board<'_, V> {
    /// `CreatureCmd.GainBlock` of `amount`, unpowered, on `owner` from no
    /// play: `Hook.ModifyBlock`, which Shadowmeld answers whatever the props,
    /// then the block, then `Hook.AfterBlockGained`. Nothing once the fight
    /// is over.
    pub(super) fn gain_unpowered(&mut self, owner: CombatId, amount: V::Number) {
        // No card is the source, not the one being played, nor the one whose
        // attack is landing, so neither one's enchantment is asked.
        let playing = self.playing.take();
        let attacking_as = self.attacking_as.take();
        let block = Fight::resolve(
            self,
            Effect::GainBlock {
                recipient: owner,
                amount: Amount::Number(amount),
                scaling: Scaling::Static,
            },
        );
        Fight::apply(self, block);
        self.playing = playing;
        self.attacking_as = attacking_as;
    }

    /// Stop a command that starts while a question waits, where nothing
    /// holds it for the answer.
    ///
    /// A move returns on an attack that asked, so its command here ran on
    /// past the attack. An effect stands still where its code hands back
    /// [`sts2_core::Played::Waiting`], so its command here needs a step cut.
    /// A draw or an auto-play waits behind the question on its own.
    #[inline]
    fn ran_on(&self) {
        if self.live.asking.is_some() {
            self.ran_on_asking();
        }
    }

    /// [`Board::ran_on`] once a question waits.
    #[cold]
    fn ran_on_asking(&self) {
        let seat = self.seat();
        if matches!(self.choice, Choice::Move { attacking: false }) {
            panic!("a move ran on past its attack while the player's answer waits");
        }
        if matches!(self.choice, Choice::Blocking) && self.landed.is_none() {
            let card = self
                .playing
                .and_then(|id| self.live.players[seat].cards.named(id))
                .map(|card| card.face.ident.card);
            unimplemented!("an effect of {card:?} running on past a question");
        }
    }

    /// `AttackCommand.Execute`: the hooks either side of `hits`.
    fn attack_landing(
        &mut self,
        attacker: CombatId,
        scaling: Scaling,
        hits: impl FnOnce(&mut Self),
    ) -> sts2_core::AttackResults<V> {
        let attacking = self
            .at(attacker)
            .is_some_and(|at| self.live.creatures[at].alive);
        if self.over() || !attacking {
            return sts2_core::AttackResults::new();
        }
        let monster = self
            .at(attacker)
            .and_then(|at| match self.live.creatures[at].model {
                Some(sts2_core::ModelId::Monster(monster)) => Some(monster),
                _ => None,
            });
        let command = sts2_core::AttackId(self.live.attacks);
        self.live.attacks += 1;
        let source = self.playing.map(|id| {
            let card = self.live.players[self.seat()]
                .cards
                .named(id)
                .expect("the card making an attack is in the fight");
            (id, card.face.ident.card)
        });
        if let Some((listener, effect)) = super::listen::first_answer::<V>(
            self.actor_seat(),
            self.live,
            self.belts,
            sts2_core::EventHook::BeforeAttack {
                command,
                attacker,
                source,
                powered: scaling == Scaling::Dynamic,
            },
        ) {
            unimplemented!("{listener:?} asking for {effect:?} before an attack")
        }
        // A command inside another's hits keeps its own results.
        let outer = self.landed.replace(sts2_core::AttackResults::new());
        let acting = self.acting();
        let as_was = std::mem::replace(&mut self.attacking_as, acting);
        hits(self);
        self.attacking_as = as_was;
        let results = std::mem::replace(&mut self.landed, outer).unwrap_or_default();
        let attack = super::live::HeldAttack {
            command,
            attacker,
            monster,
            powered: scaling == Scaling::Dynamic,
            source,
        };
        if !self.held.is_empty() || self.half.is_some() {
            self.live.defer(super::live::Deferred::Hits {
                frame: self.frame(),
                attack,
                half: self.half.take(),
                command: self.held_command.take(),
                hits: std::mem::take(&mut self.held),
                landed: results.clone(),
            });
            return results;
        }
        let results = self.attack_over(attack, results);
        // A card waiting behind this attack's question reads them as it
        // goes on.
        if self.live.asking.is_some() {
            self.live.waited_attack = Some(results.clone());
        }
        results
    }

    /// Whether a hit resolved now waits: inside an attack whose earlier hit
    /// raised a question, on a board that can stand still for it.
    fn hits_wait(&self) -> bool {
        self.live.asking.is_some() && self.pausable() && self.landed.is_some()
    }

    /// The end of an attack once all its hits have landed: `History
    /// .CreatureAttacked`, then `AfterAttack` over `results`.
    fn attack_over(
        &mut self,
        attack: super::live::HeldAttack<V>,
        results: sts2_core::AttackResults<V>,
    ) -> sts2_core::AttackResults<V> {
        let super::live::HeldAttack {
            command,
            attacker,
            monster,
            powered,
            source,
        } = attack;
        // `History.CreatureAttacked`, before `AfterAttack` and whether or not
        // the fight is ending.
        let pet_of = (self.live.creatures.iter())
            .find(|creature| creature.id == attacker)
            .and_then(|creature| creature.owner);
        self.live.history.attacked(monster, pet_of);
        // `IterateCombatHookListeners` hands a fight that is ending no
        // listeners.
        if self.over() {
            return results;
        }
        // The results stand on the fight while each listener reads them, as
        // the command itself is handed to each in the game. An earlier
        // listener's own commands leave them as they were.
        let Some(mut walking) = super::listen::Walking::begin(
            self.live,
            self.belts,
            sts2_core::EventHook::AfterAttack {
                command,
                attacker,
                pet_of,
                source,
                monster,
                powered,
            },
        ) else {
            return results;
        };
        while let Some((listener, effect)) = {
            self.live.attack_results.extend_from_slice(&results);
            let next = self.next_of(&mut walking);
            self.live.attack_results.clear();
            next
        } {
            match (listener, effect) {
                (Listener::Power { owner, power, .. }, HookEffect::ModifyAmount(offset)) => {
                    self.modify_amount(owner, power, offset)
                }
                // Written on the card that asked, wherever it stands.
                (Listener::Card { id }, HookEffect::ModifySelf { change, lasting }) => {
                    let seat = self.listener_seat(listener);
                    self.live.modify_self(seat, id, change, lasting)
                }
                // `CreatureCmd.GainBlock`, unpowered, on the relic's owner.
                (Listener::Relic { .. }, HookEffect::GainBlock(amount)) => {
                    let player = self.live.player(self.listener_seat(listener)).id;
                    self.gain_unpowered(player, amount);
                }
                // The same, on the power's owner.
                (Listener::Power { owner, .. }, HookEffect::GainBlock(amount)) => {
                    self.gain_unpowered(owner, amount);
                }
                // A fight here has one player, whose piles take them all.
                (Listener::Power { .. }, HookEffect::GiveCardsPerUnblockedHit { card, per }) => {
                    let player = self.live.player(self.listener_seat(listener)).id;
                    let hits = results
                        .iter()
                        .filter(|hit| hit.receiver == player && hit.unblocked > V::Number::zero())
                        .count();
                    let count = num_traits::cast::<_, usize>(per)
                        .unwrap_or_else(|| unimplemented!("{per:?} cards for each hit"))
                        * hits;
                    self.add_generated_cards(CardIdent::new(card), Pile::Discard, count);
                }
                (
                    Listener::Power { owner, .. },
                    HookEffect::ApplyToOwnerPerUnblockedHit { power, per },
                ) => {
                    // Each hit that took health off any target counts once.
                    // A hit on a pet carries its owner's result after the
                    // pet's, which the count leaves out.
                    let mut hits = 0usize;
                    let mut stood_in_for = None;
                    let mut counted = None;
                    for result in &results {
                        if stood_in_for.take() == Some(result.receiver) {
                            continue;
                        }
                        stood_in_for = self
                            .at(result.receiver)
                            .and_then(|at| self.live.creatures[at].owner);
                        if result.unblocked > V::Number::zero() && counted != Some(result.hit) {
                            counted = Some(result.hit);
                            hits += 1;
                        }
                    }
                    if hits > 0 {
                        let hits: V::Number = num_traits::cast(hits).expect("a hit count");
                        self.land(owner, power, PowerAmount::Counted(per * hits), Some(owner));
                    }
                }
                (listener, effect) => {
                    unimplemented!("{listener:?} asking for {effect:?} after an attack")
                }
            }
        }
        results
    }

    /// What an `AfterShuffle` listener asked for, carried out. A relic is
    /// the only listener that adds cards then, and its owner is the player.
    fn after_shuffle(&mut self, effect: HookEffect<V>) {
        let seat = self.seat();
        match effect {
            HookEffect::AddToDrawAtRandom {
                card,
                count,
                by_owner,
            } => {
                let count: usize = num_traits::cast(count)
                    .unwrap_or_else(|| unimplemented!("{count:?} cards into the draw pile"));
                let creator = by_owner.then(|| self.live.player(seat).id);
                for _ in 0..count {
                    self.add_to_draw_at_random(card, creator);
                }
            }
            // `CreatureCmd.GainBlock`, unpowered, on a relic's owner, which
            // the board acts for.
            effect @ HookEffect::GainBlock(_) => super::hook::apply::<V, _>(self, [effect]),
            effect => unimplemented!("{effect:?} after a shuffle"),
        }
    }
}

/// `CardPileCmd.Draw`, which shuffles the discard pile back in when the draw
/// pile runs dry.
///
/// A fight with every card in hand draws short rather than manufacture one.
/// `hand_draw` is `fromHandDraw`, which the turn's own draw is. How the fight
/// ended, if a card auto-played as it was drawn ended it, which stops the
/// draw.
#[tracing::instrument(level = "trace", skip_all)]
#[must_use = "a card auto-played as it is drawn can end the fight"]
pub(super) fn draw<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut Belts<V>,
    ascension: V::AscensionLevel,
    count: usize,
    drew: Option<&mut sts2_core::Drew<V>>,
) -> Option<super::Ended> {
    match draw_pausing::<V>(
        seat, live, rngs, belts, ascension, count, false, false, drew,
    ) {
        Drawn::Done(ended) => ended,
        Drawn::Paused { asks, .. } => {
            unimplemented!("a shuffle asking for {asks:?} inside a draw that cannot pause")
        }
        Drawn::Asked { .. } => {
            unimplemented!(
                "a card auto-played as it was drawn asking inside a draw that cannot pause"
            )
        }
    }
}

/// What a board was built for, which the hits it held are landed on again.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Frame<V: Rules> {
    actor: CombatId,
    target: Option<CombatId>,
    playing: Option<CardId<V>>,
    intent_damage: V::Number,
    intent_repeats: V::Count,
    choice: Choice,
}

impl<'a, V: Rules> Board<'a, V> {
    /// Take `id`'s move graph out of the fight. The model of the monster
    /// whose move this is stays on the board until the move is over.
    fn leave_machine(&mut self, id: CombatId) {
        if self.moving() && id == self.actor {
            self.left = self
                .live
                .machines
                .iter()
                .find(|held| held.creature == id)
                .map(|held| held.state);
        }
        self.live.machines.retain(|held| held.creature != id);
    }

    /// What this board was built for.
    fn frame(&self) -> Frame<V> {
        Frame {
            actor: self.actor,
            target: self.target,
            playing: self.playing,
            intent_damage: self.intent_damage,
            intent_repeats: self.intent_repeats,
            choice: self.choice,
        }
    }

    /// A board as `frame` was, inside the attack whose hits it held.
    fn in_frame(
        live: &'a mut Live<V>,
        rngs: CombatRngs<'a>,
        belts: &'a mut Belts<V>,
        ascension: V::AscensionLevel,
        frame: Frame<V>,
    ) -> Self {
        let Frame {
            actor,
            target,
            playing,
            intent_damage,
            intent_repeats,
            choice,
        } = frame;
        Board {
            live,
            rngs,
            belts,
            actor,
            target,
            playing,
            intent_damage,
            intent_repeats,
            ascension,
            skipped: Vec::new(),
            ended: None,
            choice,
            held: Vec::new(),
            half: None,
            held_command: None,
            left: None,
            landed: None,
            commanding: false,
            dealer_asked: false,
            attacking_as: None,
            reaching: None,
        }
    }
}

/// Land the hits an attack held while a question waited, each resolved now,
/// then end the attack. A question one of them raises holds the rest again.
pub(super) fn land_held<V: Rules>(
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut Belts<V>,
    ascension: V::AscensionLevel,
    held: super::live::Deferred<V>,
) -> Option<super::Ended> {
    let super::live::Deferred::Hits {
        frame,
        attack,
        half,
        command,
        hits,
        landed,
    } = held
    else {
        unreachable!("only held hits land as them")
    };
    let mut board = Board::in_frame(live, rngs, belts, ascension, frame);
    board.landed = Some(landed);
    let mut hits = hits.into_iter();
    match (half, command) {
        // The command goes on: what it landed, the hit that waited, and the
        // rest of its hits. Then its hooks run over all of them.
        (Some(half), Some(command)) => board.damage_command(|board| {
            board.dealer_asked = true;
            board.live.landing.extend(command.landed);
            board.commanding = false;
            let landed = board.land_rest(half);
            board.commanding = true;
            board.live.landing.extend(landed);
            for effect in hits.by_ref().take(command.hits) {
                let hit = Fight::resolve(&*board, effect);
                Fight::apply(board, hit);
            }
        }),
        (Some(half), None) => {
            let mut dying = SmallVec::<[CombatId; 2]>::new();
            if let Some(landed) = board.land_rest(half) {
                let hit = board.next_hit();
                board.settle_hit(landed, hit, &mut dying);
            }
            board.kill_together(&dying);
        }
        (None, Some(_)) => unreachable!("a command waits only behind its hit that asked"),
        (None, None) => {}
    }
    for effect in hits {
        let hit = Fight::resolve(&board, effect);
        Fight::apply(&mut board, hit);
    }
    let results = board.landed.take().unwrap_or_default();
    if board.held.is_empty() {
        let results = board.attack_over(attack, results);
        board.live.waited_attack = Some(results);
    } else {
        let hits = std::mem::take(&mut board.held);
        board.live.defer(super::live::Deferred::Hits {
            frame,
            attack,
            half: board.half.take(),
            command: board.held_command.take(),
            hits,
            landed: results,
        });
    }
    let ended = board.ended();
    let skipped = std::mem::take(&mut board.skipped);
    super::play::merge::<V>(&mut live.skip_tick, skipped);
    ended
}

/// [`draw`] inside a card's or a potion's effect, which a shuffle's question
/// can stand still. The question parks, and the rest of the draw waits in
/// [`Live::deferred`] with the effect behind it. `drew` gathers what it
/// draws, for the effect to read once it is over.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_in_effect<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut Belts<V>,
    ascension: V::AscensionLevel,
    count: usize,
    hand_draw: bool,
    resumed: bool,
    mut drew: Option<sts2_core::Drew<V>>,
) -> Option<super::Ended> {
    let (mut count, mut resumed) = (count, resumed);
    loop {
        match draw_pausing::<V>(
            seat,
            live,
            rngs.reborrow(),
            belts,
            ascension,
            count,
            hand_draw,
            resumed,
            drew.as_mut(),
        ) {
            // What an effect's draw drew waits for the effect.
            Drawn::Done(ended) => {
                live.drew = drew;
                return ended;
            }
            // The auto-played card's play waits first, then the draw.
            Drawn::Asked { left } => {
                live.defer(super::live::Deferred::Draw {
                    count: left,
                    hand_draw,
                    resumed: true,
                    drew,
                });
                return None;
            }
            Drawn::Paused {
                left,
                asks: (pile, selection, act),
            } => {
                match super::turn::select::<V>(
                    seat,
                    live,
                    rngs.reborrow(),
                    belts,
                    pile,
                    selection,
                    act,
                ) {
                    Ok(Some(ended)) => return Some(ended),
                    Ok(None) => {}
                    Err(question) => {
                        live.asking = Some(super::live::Asking {
                            then: super::live::Then::Pick { act },
                            question,
                            signalled: false,
                        });
                        live.defer(super::live::Deferred::Draw {
                            count: left,
                            hand_draw,
                            resumed: true,
                            drew,
                        });
                        return None;
                    }
                }
                (count, resumed) = (left, true);
            }
        }
    }
}

/// What a `PowerCmd.Apply` over several targets still has to land. That is
/// `power` at `amount` from `applier` on each of `targets`, inside the effect
/// of the card `playing` names.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Lands<V: Rules> {
    targets: sts2_core::Opponents,
    power: V::Power,
    amount: PowerAmount<V::Number>,
    applier: Option<CombatId>,
    playing: Option<CardId<V>>,
}

/// The rest of a `PowerCmd.Apply` over several targets, which a question
/// stood still.
pub(super) fn land_waiting<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut Belts<V>,
    ascension: V::AscensionLevel,
    lands: Lands<V>,
) -> Option<super::Ended> {
    let mut board =
        Board::for_card(seat, live, rngs, belts, ascension, None, lands.playing).blocking();
    board.land_each(&lands.targets, lands.power, lands.amount, lands.applier);
    let ended = board.ended();
    let skipped = std::mem::take(&mut board.skipped);
    super::play::merge::<V>(&mut live.skip_tick, skipped);
    ended
}

/// Where `CardPileCmd.AutoPlayFromDrawPile` from the top stands: `left` cards
/// still to take, after the cards `taken` into the play pile.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct AutoPlayFromDraw<V: Rules> {
    pub(super) left: usize,
    pub(super) taken: SmallVec<[CardId<V>; 4]>,
    pub(super) force_exhaust: bool,
}

/// The rest of an `AfterShuffle` walk once its pick is made, and how it
/// ended the fight.
///
/// # Panics
///
/// A second listener asking the player to pick.
pub(super) fn after_pick_walk<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut Belts<V>,
    waiting: super::listen::Walking<V>,
) -> Option<super::Ended> {
    let ascension = live.ascension;
    let mut board = Board::for_card(seat, live, rngs, belts, ascension, None, None);
    if let Some(asks) = after_shuffle_walk::<V>(&mut board, waiting) {
        unimplemented!("a second listener asking for {asks:?} after a shuffle");
    }
    board.ended()
}

/// `CardPileCmd.AutoPlayFromDrawPile` from the top, carried on from `from`.
/// Each card taken first shuffles if the draw pile is empty. A shuffle's
/// question stands the rest of the taking still, and a played card's
/// question stands the rest of the plays still. `resumed` carries on from
/// such a shuffle once its pick is made.
#[allow(clippy::too_many_arguments)]
pub(super) fn auto_play_from_draw<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut Belts<V>,
    ascension: V::AscensionLevel,
    from: AutoPlayFromDraw<V>,
    pausable: bool,
    resumed: bool,
) -> Option<super::Ended> {
    let AutoPlayFromDraw {
        left,
        mut taken,
        force_exhaust,
    } = from;
    // The `AfterShuffle` listeners that waited for the pick this resumes
    // from.
    if resumed && let Some(waiting) = live.after_pick.take() {
        let mut board = Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
        if let Some(asks) = after_shuffle_walk::<V>(&mut board, *waiting) {
            unimplemented!("a second listener asking for {asks:?} after a shuffle");
        }
    }
    for taking in 0..left {
        // `ShuffleIfNecessary`, which needs a discard pile to shuffle in.
        let asks = match live.players[seat].cards.is_empty(Pile::Draw)
            && !live.players[seat].cards.is_empty(Pile::Discard)
        {
            true => match shuffle_asking::<V>(seat, live, rngs.reborrow(), belts, ascension) {
                Ok(asks) => asks,
                Err(ended) => return Some(ended),
            },
            false => None,
        };
        if let Some((pile, selection, act)) = asks {
            if !pausable {
                unimplemented!("a shuffle asking for {pile:?} outside an effect's auto-play");
            }
            match super::turn::select::<V>(seat, live, rngs.reborrow(), belts, pile, selection, act)
            {
                Ok(Some(ended)) => return Some(ended),
                Ok(None) => {}
                Err(question) => {
                    live.asking = Some(super::live::Asking {
                        then: super::live::Then::Pick { act },
                        question,
                        signalled: false,
                    });
                    live.defer(super::live::Deferred::AutoPlayFromDraw(AutoPlayFromDraw {
                        left: left - taking,
                        taken,
                        force_exhaust,
                    }));
                    return None;
                }
            }
        }
        if live.players[seat].cards.is_empty(Pile::Draw) {
            break;
        }
        taken.push(live.players[seat].cards.play(Pile::Draw, 0).id);
    }
    for (at, &id) in taken.iter().enumerate() {
        // A question an earlier card's play asked stands the rest still in
        // the play pile until it is answered.
        if pausable && live.asking.is_some() {
            for &id in &taken[at..] {
                live.defer(super::live::Deferred::AutoPlayTaken { id, force_exhaust });
            }
            return None;
        }
        let ended = super::play::auto_play_taken::<V>(
            seat,
            live,
            rngs.reborrow(),
            belts,
            id,
            force_exhaust,
            pausable,
        );
        if ended.is_some() {
            return ended;
        }
    }
    None
}

/// How a draw that can pause stopped.
pub(super) enum Drawn {
    /// As many cards as there were to draw, and how the fight ended if it did.
    Done(Option<super::Ended>),
    /// A shuffle inside it asked for a selection, with `left` cards still to
    /// draw once it is answered.
    Paused { left: usize, asks: ShuffleAsks },
    /// A card auto-played as it was drawn asked the player something, with
    /// `left` cards still to draw once its play is over.
    Asked { left: usize },
}

impl Drawn {
    /// The same stop, with `left` cards still to draw behind it.
    fn with_left(self, left: usize) -> Drawn {
        match self {
            Drawn::Done(ended) => Drawn::Done(ended),
            Drawn::Paused { asks, .. } => Drawn::Paused { left, asks },
            Drawn::Asked { .. } => Drawn::Asked { left },
        }
    }
}

/// [`draw`], stopping where a shuffle inside it asks the player something.
/// `resumed` is a draw that carries on from such a stop, which has asked
/// `ShouldDraw` already. `drew` gathers the copies it draws itself, and
/// none a listener's draw inside it draws.
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_pausing<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut Belts<V>,
    ascension: V::AscensionLevel,
    count: usize,
    hand_draw: bool,
    resumed: bool,
    mut drew: Option<&mut sts2_core::Drew<V>>,
) -> Drawn {
    // `Hook.ShouldDraw`, once for the whole draw, and `AfterPreventingDraw`
    // when a listener stops it.
    let player = live.player(seat).id;
    let hook = sts2_core::DecisionHook::ShouldDraw {
        player,
        hand_draw,
        side: live.side,
    };
    // The `AfterShuffle` listeners that waited for the pick this resumes
    // from.
    if resumed && let Some(waiting) = live.after_pick.take() {
        let mut board = Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
        if let Some(asks) = after_shuffle_walk::<V>(&mut board, *waiting) {
            unimplemented!("a second listener asking for {asks:?} after a shuffle");
        }
    }
    // The late pass for the card whose auto-play asked, which the draw it
    // came in by owes first.
    if resumed && let Some((id, hand_draw)) = live.drawn_late.take() {
        let drawn = live.players[seat]
            .cards
            .named(id)
            .unwrap_or_else(|| unimplemented!("the late pass for {id:?}, which left the fight"));
        match card_drawn_late::<V>(
            seat,
            live,
            rngs.reborrow(),
            belts,
            ascension,
            &drawn,
            hand_draw,
            None,
        ) {
            Drawn::Done(None) => {}
            Drawn::Done(Some(ended)) => return Drawn::Done(Some(ended)),
            stopped => return stopped.with_left(count),
        }
    }
    // The listeners' draws the question stopped, the innermost first, each
    // with what the listeners after it asked.
    if resumed && !live.hook_draws.is_empty() {
        let waiting = std::mem::take(&mut live.hook_draws);
        for at in (0..waiting.len()).rev() {
            let HookDraw { left, drawn, then } = waiting[at].clone();
            let stopped = match draw_pausing::<V>(
                seat,
                live,
                rngs.reborrow(),
                belts,
                ascension,
                left,
                false,
                true,
                None,
            ) {
                Drawn::Done(Some(ended)) => return Drawn::Done(Some(ended)),
                Drawn::Done(None) => match carry_out_drawn::<V>(
                    seat,
                    live,
                    rngs.reborrow(),
                    belts,
                    ascension,
                    drawn,
                    *then,
                    None,
                ) {
                    Drawn::Done(None) => continue,
                    Drawn::Done(Some(ended)) => return Drawn::Done(Some(ended)),
                    stopped => stopped,
                },
                stopped @ (Drawn::Paused { left, .. } | Drawn::Asked { left }) => {
                    live.hook_draws.insert(0, HookDraw { left, drawn, then });
                    stopped
                }
            };
            let mut owed: SmallVec<[HookDraw<V>; 2]> = waiting[..at].iter().cloned().collect();
            owed.extend(live.hook_draws.drain(..));
            live.hook_draws = owed;
            return stopped.with_left(count);
        }
    }
    if !resumed && super::listen::any_decides::<V>(Walk { live, belts }, hook, false) {
        if let Some((listener, effect)) =
            super::listen::first_answer::<V>(seat, live, belts, sts2_core::EventHook::DrawPrevented)
        {
            unimplemented!("{listener:?} asking for {effect:?} after preventing a draw");
        }
        return Drawn::Done(None);
    }
    let mut ended = None;
    for drawing in 0..count {
        // `IsOverOrEnding`, and `CardPile.MaxCardsInHand`: a full hand stops
        // the draw.
        if ended.is_some()
            || live.players[seat].cards.len(Pile::Hand) >= super::pile::MAX_CARDS_IN_HAND
        {
            return Drawn::Done(ended);
        }
        if live.players[seat].cards.is_empty(Pile::Draw) {
            if live.players[seat].cards.is_empty(Pile::Discard) {
                return Drawn::Done(ended);
            }
            match shuffle_asking::<V>(seat, live, rngs.reborrow(), belts, ascension) {
                Ok(None) => {}
                Ok(Some(asks)) => {
                    return Drawn::Paused {
                        left: count - drawing,
                        asks,
                    };
                }
                // `IsOverOrEnding` stops the draw.
                Err(ended) => return Drawn::Done(Some(ended)),
            }
        }
        let drawn = live.players[seat]
            .cards
            .draw()
            .expect("a draw pile just checked for cards");
        if let Some(drew) = drew.as_deref_mut() {
            drew.push(drawn.id);
        }
        // `History.CardDrawn`, before the hook, so a listener counts this
        // draw among the turn's.
        live.history
            .draw(drawn.face.ident.card, hand_draw, seat.creature());
        // `Hook.AfterCardDrawn` makes its early pass over every listener first.
        let hook = sts2_core::EventHook::CardDrawnEarly {
            card: drawn.face.ident.card,
            id: drawn.id,
            player: seat.creature(),
            hand_draw,
        };
        let mut walking = super::listen::Walking::begin(live, belts, hook);
        while let Some((listener, effect)) = walking
            .as_mut()
            .and_then(|walking| walking.next(seat, live, belts))
        {
            match effect {
                // `IsOverOrEnding` stops an auto-play before it starts.
                HookEffect::AutoPlay(card) => {
                    if ended.is_none() {
                        ended = super::play::auto_play::<V>(
                            seat,
                            live,
                            rngs.reborrow(),
                            belts,
                            card,
                            Pile::Hand,
                            None,
                            true,
                        );
                    }
                }
                effect => unimplemented!("{listener:?} asking for {effect:?} as a card is drawn"),
            }
        }
        // An auto-play that asks stands the draw still. Its late pass and the
        // rest of the draw wait for the play to end.
        if live.asking.is_some() {
            live.drawn_late = Some((drawn.id, hand_draw));
            return Drawn::Asked {
                left: count - drawing - 1,
            };
        }
        match card_drawn_late::<V>(
            seat,
            live,
            rngs.reborrow(),
            belts,
            ascension,
            &drawn,
            hand_draw,
            ended,
        ) {
            Drawn::Done(now) => ended = now,
            stopped => return stopped.with_left(count - drawing - 1),
        }
    }
    Drawn::Done(ended)
}

/// The late pass of `Hook.AfterCardDrawn` for `drawn`, after `ended` from
/// what came before it. A listener's draw inside it can stop on a question.
/// That stop carries no count of its own, and the caller names what is left
/// of its draw.
///
/// Inlined into the draw, which runs it once for each card drawn.
#[inline(always)]
#[allow(clippy::too_many_arguments)]
fn card_drawn_late<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    belts: &mut Belts<V>,
    ascension: V::AscensionLevel,
    drawn: &super::live::CardInstance<V>,
    hand_draw: bool,
    ended: Option<super::Ended>,
) -> Drawn {
    // `IterateCombatHookListeners` hands a fight that is over or ending no
    // listeners, as one an auto-play in the early pass won, which `ended`
    // already says.
    if ended.is_some() {
        return Drawn::Done(ended);
    }
    // `Hook.AfterCardDrawn`, raised for each card inside the loop rather
    // than once for the whole draw. So a card that spends the pool as it
    // arrives has spent it before the next card is taken.
    let hook = sts2_core::EventHook::CardDrawn {
        card: drawn.face.ident.card,
        id: drawn.id,
        player: seat.creature(),
        ethereal: drawn.face.ethereal(),
        hand_draw,
        side: live.side,
    };
    match super::listen::Walking::begin(live, belts, hook) {
        Some(walking) => {
            carry_out_drawn::<V>(seat, live, rngs, belts, ascension, drawn.id, walking, ended)
        }
        None => Drawn::Done(ended),
    }
}

/// A listener's draw that a question stopped inside another draw. It still
/// owes `left` cards, and `then` is the rest of the walk over the card
/// `drawn`, which waits for it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct HookDraw<V: Rules> {
    left: usize,
    drawn: CardId<V>,
    then: Box<super::listen::Walking<V>>,
}

/// The late `AfterCardDrawn` walk over the card `drawn`, after `ended`. Each
/// listener acts as the walk reaches it. A listener's `CardPileCmd.Draw`
/// runs inside the draw that took the card. A shuffle's question in it
/// pauses that draw too, and the rest of the walk waits.
#[allow(clippy::too_many_arguments)]
fn carry_out_drawn<V: Rules>(
    seat: sts2_core::SeatIndex,
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    belts: &mut Belts<V>,
    ascension: V::AscensionLevel,
    drawn: CardId<V>,
    mut walking: super::listen::Walking<V>,
    mut ended: Option<super::Ended>,
) -> Drawn {
    while let Some((listener, effect)) = walking.next(seat, live, belts) {
        match (listener, effect) {
            // Written on the card that asked, wherever it stands.
            (Listener::Card { id }, HookEffect::ModifySelf { change, lasting }) => {
                live.modify_self(seat, id, change, lasting)
            }
            // Only while the card is still in hand, which the
            // enchantment checks before it draws.
            (Listener::Card { id }, HookEffect::RandomSelfCost { below }) => {
                if live.players[seat].cards.find(Pile::Hand, id).is_some() {
                    let cost = random_cost::<V>(&mut rngs, below);
                    live.modify_self(
                        seat,
                        id,
                        sts2_core::Change::SetCost(sts2_core::CostPool::Energy, cost),
                        sts2_core::Lasting::ThisFight,
                    );
                }
            }
            // Wherever the card stands.
            (_, HookEffect::RandomCost { card, below }) => {
                let cost = random_cost::<V>(&mut rngs, below);
                live.modify_self(
                    seat,
                    card,
                    sts2_core::Change::SetCost(sts2_core::CostPool::Energy, cost),
                    sts2_core::Lasting::ThisFight,
                );
            }
            (_, HookEffect::Draw(count)) => {
                if ended.is_some() {
                    continue;
                }
                // A negative count draws nothing: the game's loop runs no times.
                let owed = num_traits::NumCast::from(count).unwrap_or(0usize);
                let below = live.hook_draws.len();
                match draw_pausing::<V>(
                    seat,
                    live,
                    rngs.reborrow(),
                    belts,
                    ascension,
                    owed,
                    false,
                    false,
                    None,
                ) {
                    Drawn::Done(now) => ended = now,
                    stopped @ (Drawn::Paused { left, .. } | Drawn::Asked { left }) => {
                        live.hook_draws.insert(
                            below,
                            HookDraw {
                                left,
                                drawn,
                                then: Box::new(walking),
                            },
                        );
                        return stopped;
                    }
                }
            }
            (listener, effect) => {
                let mut board =
                    Board::for_card(seat, live, rngs.reborrow(), belts, ascension, None, None);
                board.listener_asked(listener, effect);
                ended = ended.or(board.ended());
            }
        }
    }
    Drawn::Done(ended)
}

/// `CombatEnergyCosts.NextInt(below)`, as a cost.
fn random_cost<V: Rules>(rngs: &mut CombatRngs<'_>, below: u8) -> V::Number {
    let bound = sts2_core::rng::Bound::try_new(
        num_traits::NumCast::from(below).expect("a cost bound fits the game's RNG integer"),
    )
    .expect("a random cost has somewhere to land");
    let cost = rngs.energy_costs.next_int(bound);
    num_traits::NumCast::from(cost).expect("an energy cost fits a game number")
}

/// `CombatManager.IsEnding`: no enemy stands that is both alive and
/// primary, and no listener holds the fight open.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn ending<V: Rules>(walk: Walk<'_, V>) -> bool {
    // `IsAlive` is `CurrentHp > 0`. A creature a damage command took to
    // nothing is dead to it before the command kills it.
    let primary_enemy_remains = walk.live.creatures.iter().any(|creature| {
        creature.side == <V::CombatSide as SideKinds>::ENEMY
            && creature.alive
            && creature.hp > V::Health::zero()
            && !walk.live.powers.of(creature.id).any(|held| {
                <V as sts2_core::Hooks>::decide_hook(
                    held.listener(
                        creature.carrier(),
                        walk.live.history(creature.carrier().player),
                        &super::live::Onlookers {
                            creatures: &walk.live.creatures,
                            powers: &walk.live.powers,
                            kept: &walk.live.kept,
                            belts: Some(walk.belts),
                            attack: &walk.live.attack_results,
                        },
                    ),
                    sts2_core::DecisionHook::ShouldCountForCombatEnd,
                )
                .is_decision_and(|counts| !counts)
            })
    });
    !primary_enemy_remains
        && !super::listen::any_decides::<V>(
            walk,
            sts2_core::DecisionHook::ShouldStopCombatFromEnding,
            true,
        )
}

#[cfg(test)]
mod tests {
    /// The one seat of the solo fights these play.
    const SOLO: sts2_core::SeatIndex = sts2_core::SeatIndex(0);

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
        let creatures = [Fighter {
            id: player,
            model: Some(ModelId::Character(Character::Regent)),
            side: CombatSide::Player,
            hp: 10,
            max_hp: 10,
            block: 0,
            alive: true,
            next_move: None,
            owner: None,
        }];
        let card = CardInstance::new(
            CardIdent::new(Cards::Status(StatusCard::Infection)),
            CardId::new(0),
        );

        let mut fight = Live::of(creatures);
        fight.powers.push(
            player,
            HeldPower::new(
                power,
                crate::combat::live::Stacked::counted(2),
                Some(player),
                None,
            ),
        );
        resolve::<V>(
            SOLO,
            Walk {
                live: &fight,
                belts: &Belts::new(crate::run::Belt::new(), Vec::new()),
            },
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
