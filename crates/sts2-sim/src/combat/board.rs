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
    Effect, EffectResolution, EnchantHook, Fight, HookEffect, Modify, ModifyHook, Pile,
    PowerAmount, Rules, RunStreamKinds, Scaling, SideKinds, Stack,
};

use super::listen::{Listener, Walk};
use super::live::{CardInstance, Face, Fighter, HeldPower, Live};
use super::powers::Powers as _;
use crate::run::Belt;
use crate::stream::CombatRngs;

/// The fight an effect is resolved against.
pub(super) struct Board<'a, V: Rules> {
    pub(super) live: &'a mut Live<V>,
    /// The run streams an effect can draw from while it resolves.
    rngs: CombatRngs<'a>,
    /// The player's relic belt, for hooks raised while an effect resolves.
    relics: &'a mut Belt<V>,
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
    /// A card's or a potion's own effect, whose play can stand still while a
    /// card it auto-plays asks a question. [`super::live::Deferred`] holds
    /// what waits.
    pausable: bool,
    /// A monster's move, which is pausable only inside its attack.
    ///
    /// A move's attack is `Execute(null)`, which blocks on a question. Every
    /// other command it runs takes a `ThrowingPlayerChoiceContext`, which
    /// throws on one.
    moving: bool,
    /// The hits of the attack under way that wait on a question, in order.
    held: Vec<Effect<V>>,
    /// The model of a monster that left the fight during its own move. The
    /// game's model object outlives the creature, so the rest of the move
    /// still reads and writes it.
    left: Option<V::MonsterState>,
    /// The results of the attack command whose hits are landing, which
    /// `AttackCommand.Results` gathers.
    landed: Option<sts2_core::AttackResults<V>>,
}

/// What one hit took off one creature's hit points, before its block is
/// counted in.
struct Lost<V: Rules> {
    receiver: CombatId,
    lost: V::Number,
    overkill: V::Number,
    killed: bool,
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
        /// `IsPoweredCardOrMonsterMoveBlock`.
        powered: bool,
        /// The play that gives it.
        play: Option<sts2_core::PlayAt>,
        /// `cardSource`: which copy gives it.
        source: Option<CardId<V>>,
        /// The relics whose block hooks changed it.
        modifying: SmallVec<[V::Relic; 1]>,
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
}

/// `OstyCmd.Summon` from a hook, outside any card or move.
pub(super) fn summon_osty<V: Rules>(
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    relics: &mut Belt<V>,
    ascension: V::AscensionLevel,
    amount: V::Number,
) {
    Board::for_card(live, rngs, relics, ascension, None, None).summon_osty(amount);
}

impl<'a, V: Rules> Board<'a, V> {
    /// A board for one card, played from the player.
    pub(super) fn for_card(
        live: &'a mut Live<V>,
        rngs: CombatRngs<'a>,
        relics: &'a mut Belt<V>,
        ascension: V::AscensionLevel,
        target: Option<CombatId>,
        playing: Option<CardId<V>>,
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
            ended: None,
            pausable: false,
            moving: false,
            held: Vec::new(),
            left: None,
            landed: None,
        }
    }

    /// This board for a card's or a potion's own effect, which a card it
    /// auto-plays can pause.
    pub(super) fn pausable(mut self) -> Self {
        self.pausable = true;
        self
    }

    /// A board for one move, made by a monster.
    pub(super) fn for_move(
        live: &'a mut Live<V>,
        rngs: CombatRngs<'a>,
        relics: &'a mut Belt<V>,
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
            ended: None,
            pausable: false,
            moving: true,
            held: Vec::new(),
            left: None,
            landed: None,
        }
    }

    /// The card whose effect this is, as the fight holds it now.
    ///
    /// # Panics
    ///
    /// A card the fight no longer holds.
    fn acting(&self) -> Option<CardInstance<V>> {
        self.playing.map(|id| {
            self.live
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
        let at: usize = num_traits::NumCast::from(card.index())
            .unwrap_or_else(|| unimplemented!("a pile deeper than this machine can index"));
        match self.live.cards.get(card.pile(), at) {
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
    pub(super) fn land(
        &mut self,
        at: CombatId,
        power: V::Power,
        amount: PowerAmount<V::Number>,
        applier: Option<CombatId>,
    ) {
        self.ran_on();
        // The command returns on a zero amount, and on a creature that cannot
        // hold powers. A monster the fight has taken out is one, because its
        // combat state is gone with it.
        if amount == PowerAmount::Counted(V::Number::zero()) {
            return;
        }
        // `IsEnding`, and `CanReceivePowers`, which a creature no listener
        // lets be hit fails.
        if self.over() {
            return;
        }
        let alive = self
            .live
            .creatures
            .iter()
            .any(|creature| creature.id == at && creature.alive);
        if super::listen::any_decides::<V>(
            self.walk(),
            sts2_core::DecisionHook::ShouldAllowHitting {
                creature: at,
                alive,
            },
            false,
        ) {
            return;
        }
        let id = at;
        let Some(at) = self.at(at) else { return };

        // `FindExistingInstanceForStacking`: a creature already carrying this
        // power takes the amount through `ModifyAmount`, which is the whole of
        // what happens. Everything below to the stacking itself is the other
        // branch, the one that builds an instance.
        // An instanced power builds another instance beside any there.
        let held = self
            .live
            .powers
            .of(id)
            .filter(|held| held.power() == power)
            .count();
        let fresh = held == 0 || sts2_core::PowerTable::instanced(power);
        // `FindExistingInstanceForStacking`: a counter stacks into the one
        // already there through `ModifyAmount`. An endless instance is one
        // instance however many times it lands.
        if !fresh {
            if let PowerAmount::Counted(more) = amount {
                self.modify_amount_from(id, power, more, applier);
            }
            return;
        }

        let (amount, modifiers) = match amount {
            PowerAmount::Counted(amount) => {
                let (amount, modifiers) = self.received(id, power, amount, applier);
                (PowerAmount::Counted(amount), modifiers)
            }
            PowerAmount::Endless => {
                if super::listen::heard::<V>(
                    self.live,
                    self.relics,
                    sts2_core::ModelHook::TryModifyPowerAmountReceived,
                ) {
                    unimplemented!("an endless {power:?} landing where a listener changes amounts")
                }
                (PowerAmount::Endless, SmallVec::new())
            }
        };
        self.apply_fresh(at, power, amount, applier);
        // The hook can take the creature out of the fight.
        let Some(found) = self.at(id) else {
            return;
        };
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
        // `ApplyInternal`, which puts nothing on for an amount of zero.
        let landed = amount != PowerAmount::Counted(V::Number::zero());
        if landed {
            self.live
                .powers
                .push(id, HeldPower::new(power, amount.into(), applier));
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
        if !super::listen::heard_any::<V>(self.live, self.relics, AROUND) {
            return (amount, Received::<V>::new());
        }
        // `Hook.BeforePowerAmountChanged`, which no listener here answers with
        // a command.
        let source = self.playing;
        if super::listen::heard::<V>(self.live, self.relics, ModelHook::BeforePowerAmountChanged) {
            let target_side = self
                .live
                .creatures
                .iter()
                .find(|creature| creature.id == target)
                .expect("a power lands on a creature in the fight")
                .side;
            let hook = sts2_core::EventHook::PowerAmountChanging {
                power,
                amount,
                target,
                target_side,
                applier,
                player: self.live.player().id,
                source,
            };
            let asked = super::listen::notify::<V>(self.live, self.relics, hook);
            if let Some((listener, effect)) = asked.into_iter().next() {
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
            for relic in modifying {
                let hook = sts2_core::EventHook::PowerAmountGivenModified { power };
                if let Some(effect) = super::listen::ask_relic(self.relics, relic, hook).first() {
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
                Receiver::Relic(relic) => {
                    let hook = sts2_core::EventHook::PowerAmountReceivedModified { power };
                    let asked = super::listen::ask_relic(self.relics, relic, hook);
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
                Some(self.relics),
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
        let entering: SmallVec<[_; 2]> =
            V::monster_starting_powers(monster, self.live.ascension, side, &model).collect();
        for (power, amount) in entering {
            self.land(id, power, amount, Some(id));
        }
        let aimed: SmallVec<[_; 2]> =
            V::monster_starting_opponent_powers(monster, self.live.ascension).collect();
        // `CombatState.GetOpponentsOf`, pets included.
        let opponents: SmallVec<[CombatId; 4]> = self
            .live
            .creatures
            .iter()
            .filter(|creature| creature.side != <V::CombatSide as SideKinds>::ENEMY)
            .map(|creature| creature.id)
            .collect();
        for (power, amount) in aimed {
            for &opponent in &opponents {
                self.land(opponent, power, amount, Some(id));
            }
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
        super::turn::roll_moves_of::<V>(self.live, self.relics, ascension, self.rngs.ai, |at| {
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
            Some(self.relics),
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
        if !self.live.powers.of(id).any(|held| held.power() == power) {
            return;
        }
        let (offset, modifiers) = self.received(id, power, offset, applier);
        let Some(held) = self
            .live
            .powers
            .of_mut(id)
            .find(|held| held.power() == power)
        else {
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
        self.power_received(modifiers, power);
        self.amount_changed(id, power, offset, applier);
        // `ShouldRemoveDueToAmount`, and `PowerCmd.Remove` for a power it
        // says is done.
        if spent {
            self.remove_power_carried(id, power);
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
            sts2_core::StunResume::Interrupted => machine.machine.interpose(),
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
        let upgraded: Vec<(CardId<V>, u8)> = [Pile::Hand, Pile::Draw, Pile::Discard, Pile::Exhaust]
            .into_iter()
            .flat_map(|pile| self.live.cards.iter(pile))
            .filter(|held| held.face.ident.upgrade > 0)
            .map(|held| (held.id, held.face.ident.upgrade))
            .collect();
        if !(self.over()) {
            for &(card, _) in &upgraded {
                self.live.cards.write_named(card, |face| {
                    face.downgrade();
                    true
                });
            }
        }
        upgraded
    }

    /// `PowerCmd.Remove`, with what its `AfterRemoved` asks carried out.
    pub(super) fn remove_power_carried(&mut self, id: CombatId, power: V::Power) {
        for effect in self.live.remove_power_asking(id, power) {
            match effect {
                HookEffect::KillCreature(creature) => self.kill_outright(creature),
                // `CardCmd.Upgrade` on a copy that has left the fight writes on
                // a model the fight no longer holds.
                HookEffect::UpgradeCopy { card, .. } if self.live.cards.named(card).is_none() => {}
                HookEffect::UpgradeCopy { card, times } => {
                    self.live.cards.write_named(card, |face| {
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
        let asked = super::listen::notify::<V>(
            self.live,
            self.relics,
            sts2_core::EventHook::PowerAmountChanged {
                creature: id,
                power,
                delta,
                applier,
            },
        );
        let mut cards: SmallVec<[HookEffect<V>; 4]> = SmallVec::new();
        for (listener, effect) in asked {
            match (listener, effect) {
                (Listener::Power { owner, .. }, HookEffect::ApplyToOwner(lent, amount)) => {
                    self.land(owner, lent, amount, applier)
                }
                // `CardPileCmd.Draw` for the player.
                (Listener::Power { .. }, HookEffect::Draw(count)) => {
                    sts2_core::Fight::draw(self, count)
                }
                // `CreatureCmd.Damage`, unpowered, from the power's owner.
                (Listener::Power { owner, .. }, HookEffect::Damage { target, amount }) => {
                    self.hit_unpowered(Some(owner), target, amount, false)
                }
                (_, effect) => cards.push(effect),
            }
        }
        self.write_cards(cards);
    }

    /// `CreatureCmd.Damage` on `target` once hooks worked `damage` out:
    /// `BeforeDamageReceived`, block, then hit points, then the deaths the
    /// hit caused. `dealer` is nothing for damage no creature deals, and
    /// `card` for damage no card deals. A target that has left the fight
    /// takes nothing, which is what the command does against a creature with
    /// no combat state.
    pub(super) fn hit(
        &mut self,
        dealer: Option<CombatId>,
        target: CombatId,
        damage: V::Number,
        powered: bool,
        unblockable: bool,
        card: Option<V::Card>,
    ) {
        let Some(at) = self.at(target) else { return };
        if !self.live.creatures[at].alive {
            return;
        }
        let asked = super::listen::notify::<V>(
            self.live,
            self.relics,
            sts2_core::EventHook::DamageReceiving {
                target,
                amount: damage,
                dealer,
                powered,
                card,
            },
        );
        self.after_hit(asked);
        // What answered can have moved the creatures.
        let Some(at) = self.at(target) else { return };
        let zero = V::Number::zero();
        // `PetOwner?.Creature ?? originalTarget`: a pet's hit meets its
        // owner's block.
        let shield = self.live.creatures[at].owner.unwrap_or(target);
        let Some(shield_at) = self.at(shield) else {
            return;
        };
        // `ValueProp.Unblockable` meets no block at all.
        let blocked = match unblockable {
            true => zero,
            false => self.live.creatures[shield_at].block.min(damage).max(zero),
        };
        self.live.creatures[shield_at].block = self.live.creatures[shield_at].block - blocked;
        // `ModifyHpLost` over what the block left, before and after the
        // damage finds who takes it. A power that caps a hit rather
        // than shrinks it acts here.
        let before = modified_by::<V>(
            self.walk(),
            (damage - blocked).max(zero),
            &[
                ModifyHook::HpLostBeforeOsty { target },
                ModifyHook::HpLostBeforeOstyLate { target },
            ],
        );
        let taker = super::listen::unblocked_damage_target::<V>(self.walk(), target, powered);
        let unblocked = self.hp_lost_after_osty(taker, before);
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
            let rest = self.hp_lost_after_osty(target, overkill);
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
        // Each result's hooks, in the command's order. A creature the
        // hit killed is not told it was hit: it dies once they are
        // done.
        let mut dying: SmallVec<[CombatId; 2]> = SmallVec::new();
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
            if let Some(landed) = self.landed.as_mut() {
                landed.push(sts2_core::DamageResult {
                    receiver,
                    total,
                    unblocked: lost,
                    overkill,
                    killed,
                });
            }
            if aimed && block_broken {
                let asked = super::listen::notify::<V>(
                    self.live,
                    self.relics,
                    sts2_core::EventHook::BlockBroken { creature: target },
                );
                self.after_hit(asked);
            }
            if lost > zero {
                self.current_hp_changed(receiver, -lost);
            }
            // `History.DamageReceived`, while the fight is in progress. One
            // that is ending plays no card after it, so nothing reads what it
            // records then, and it is not asked.
            if !self.stopped() {
                self.live
                    .history
                    .damage_received(sts2_core::DamageReceived {
                        receiver,
                        dealer,
                        unblocked: lost,
                        powered,
                    });
            }
            let asked = super::listen::notify::<V>(
                self.live,
                self.relics,
                sts2_core::EventHook::DamageGiven {
                    dealer,
                    pet_of,
                    target: receiver,
                    total,
                    unblocked: lost,
                    powered,
                    fully_blocked: aimed && fully_blocked,
                },
            );
            self.after_hit(asked);
            // `AfterDamageReceived` carries the `DamageResult`, so this
            // is what the hit took off rather than what it was swung
            // for.
            match killed {
                false => {
                    let asked = super::listen::notify::<V>(
                        self.live,
                        self.relics,
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
                            player: self.live.player().id,
                            side: self.live.side,
                            fully_blocked: aimed && fully_blocked,
                            turn: self.live.turn,
                        },
                    );
                    self.after_hit(asked);
                }
                true => dying.push(receiver),
            }
        }
        for creature in dying {
            if let Some(at) = self.at(creature) {
                self.kill(at);
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
        let damage = modified::<V>(self.walk(), dealer, Some(target), None, amount, false);
        self.hit(dealer, target, damage, false, unblockable, None);
    }

    /// `HittableEnemies`: the opponents alive, less any a listener refuses a
    /// hit on.
    pub(super) fn hittable_opponents(&self) -> SmallVec<[CombatId; 8]> {
        self.hittable_among(sts2_core::Fight::opponents(self))
    }

    /// `HittableEnemies` for the side `of` fights on, whoever is acting.
    fn hittable_opponents_of(&self, of: CombatId) -> SmallVec<[CombatId; 8]> {
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
    fn apply_fresh(
        &mut self,
        at: usize,
        power: V::Power,
        amount: PowerAmount<V::Number>,
        applier: Option<CombatId>,
    ) {
        // `BeforeApplied`, which runs before the power attaches. What it asks
        // for lands first, so a creature carries those powers ahead of this
        // one.
        //
        // Not a `Stack`, because nothing carries it yet. The side and the
        // holder are the creature it is landing on, which is the owner it is
        // about to have.
        let mut state = <V::PowerState as sts2_core::PowerModel<V>>::new(power);
        let carrier = self.live.creatures[at].carrier();
        let fight = super::live::Onlookers {
            creatures: &self.live.creatures,
            powers: &self.live.powers,
            kept: &self.live.kept,
            relics: Some(&*self.relics),
        };
        let listener = sts2_core::EventPower {
            power,
            amount,
            owner: carrier.side,
            holder: carrier.id,
            applier,
            monster: carrier.monster,
            on_turn_start: sts2_core::PowerAmount::Counted(num_traits::zero()),
            state: &mut state,
            history: self.live.history.view(),
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
                    self.land(self.live.creatures[at].id, power, amount, applier);
                }
                other => unimplemented!("a power asking for {other:?} as it lands"),
            }
        }
        let Some(at) = self.at(self.live.creatures[at].id) else {
            return;
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
    }

    /// `AfterCardEnteredCombat`, for a card that reached a combat pile from no
    /// pile at all.
    ///
    /// A card moving between piles is `AfterCardChangedPiles` and does not
    /// come here.
    /// `AddGeneratedCardToCombat` of one new copy into the draw pile at
    /// `CardPilePosition.Random`. Nothing once the fight is over.
    fn add_to_draw_at_random(&mut self, card: V::Card) {
        self.add_at_random_here(card, Pile::Draw);
    }

    /// `AddGeneratedCardToCombat` of one new copy into `pile` at
    /// `CardPilePosition.Random`: the place `Rng.Shuffle.NextInt(count + 1)`
    /// names, from the top. Nothing once the fight is over.
    fn add_at_random_here(&mut self, card: V::Card, pile: Pile) -> Option<CardAt<V>> {
        use sts2_core::rng::GameRng as _;

        if self.stopped() {
            return None;
        }
        let places: i32 = num_traits::cast(self.live.cards.len(pile) + 1)
            .expect("a pile's places fit the RNG integer");
        let bound = sts2_core::rng::Bound::try_new(places).expect("a pile has a place");
        let at: usize = num_traits::cast(self.rngs.shuffle.next_int(bound))
            .expect("a draw below the bound is a place");
        let id = self.live.card_ids.mint();
        self.live
            .cards
            .insert(pile, at, CardInstance::new(CardIdent::new(card), id));
        let index = num_traits::NumCast::from(at)
            .unwrap_or_else(|| unimplemented!("a pile deeper than a fight can number"));
        let at = CardAt::new(id, pile, index);
        self.card_entered_combat(at, card, false);
        self.card_generated(at, card, self.creator());
        Some(at)
    }

    fn card_entered_combat(&mut self, at: CardAt<V>, card: V::Card, clone: bool) {
        let asked = super::listen::notify::<V>(
            self.live,
            self.relics,
            sts2_core::EventHook::CardEnteredCombat { at, card, clone },
        );
        let mut rest: SmallVec<[HookEffect<V>; 4]> = SmallVec::new();
        for (listener, effect) in asked {
            match (listener, effect) {
                // Written on the card that asked, wherever it stands.
                (Listener::Card { id }, HookEffect::ModifySelf { change, lasting }) => {
                    self.live.modify_self(id, change, lasting)
                }
                (_, effect) => rest.push(effect),
            }
        }
        self.write_cards(rest);
    }

    /// `Hook.AfterCardGeneratedForCombat`, once a card made in the fight has
    /// landed at `at`, and what it asked for. `CardPileCmd.Add` has raised
    /// `AfterCardEnteredCombat` for it first.
    fn card_generated(&mut self, at: CardAt<V>, card: V::Card, creator: Option<CombatId>) {
        let by_player = creator == Some(self.live.player().id);
        self.live.history.generated(by_player);
        let asked = super::listen::notify::<V>(
            self.live,
            self.relics,
            sts2_core::EventHook::CardGeneratedForCombat { at, card, creator },
        );
        let mut rest: SmallVec<[HookEffect<V>; 4]> = SmallVec::new();
        for (listener, effect) in asked {
            match (listener, effect) {
                // Written on the card that asked, wherever it stands.
                (Listener::Card { id }, HookEffect::ModifySelf { change, lasting }) => {
                    self.live.modify_self(id, change, lasting)
                }
                // Written on the cards it names.
                (_, effect @ HookEffect::ModifyCards { .. }) => self.write_cards([effect]),
                (_, effect) => rest.push(effect),
            }
        }
        // A power answers a card its own holder made, and that holder is the
        // creature acting.
        super::hook::apply::<V, _>(self, rest);
    }

    /// Who a card the acting creature makes is made by: the player, where
    /// the player acts. A monster's cards are made by nobody.
    fn creator(&self) -> Option<CombatId> {
        let player = self.live.player().id;
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

    /// `CardPileCmd.Add` of a new copy of `card` into `pile`, and the
    /// `AfterCardEnteredCombat` it raises as it lands.
    fn put(&mut self, card: CardIdent<V>, pile: Pile) -> CardAt<V> {
        let id = self.live.card_ids.mint();
        // A full hand sends the card to the discard pile instead.
        let full =
            pile == Pile::Hand && self.live.cards.len(Pile::Hand) >= super::pile::MAX_CARDS_IN_HAND;
        let pile = if full { Pile::Discard } else { pile };
        let index = num_traits::NumCast::from(self.live.cards.len(pile))
            .unwrap_or_else(|| unimplemented!("a pile deeper than a fight can number"));
        self.live.cards.push(pile, CardInstance::new(card, id));
        let at = CardAt::new(id, pile, index);
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
                        self.live.history.afflicted(affliction);
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
        match to {
            sts2_core::Writing::One(at) => {
                let index = self.standing(at);
                self.live.cards.write(at.pile(), index, |held| {
                    write(held);
                    true
                });
            }
            // `PlayerCombatState.AllCards`, which the exhaust and play piles
            // are among. A blade taken out of the fight is written on and
            // never played again.
            sts2_core::Writing::EveryCopyOf(card) => self
                .live
                .cards
                .write_all_cards(|held| held.ident.card == card, write),
            sts2_core::Writing::EveryTagged(tag) => self
                .live
                .cards
                .write_all_cards(|held| held.ident.card.tagged(tag), write),
            sts2_core::Writing::Every => self.live.cards.write_all_cards(|_| true, write),
            sts2_core::Writing::EveryOfKind(kind) => self
                .live
                .cards
                .write_all_cards(|held| V::card_kind(held.ident.card) == Some(kind), write),
            sts2_core::Writing::EveryAfflictedWith(affliction) => self
                .live
                .cards
                .write_all_cards(|held| held.ident.affliction == Some(affliction), write),
            sts2_core::Writing::Named(id) => self.live.cards.write_named(id, |held| {
                write(held);
                true
            }),
        }
    }

    /// Resolve a death, then end the fight if it is ending.
    ///
    /// A creature a power keeps in the fight is still counted, and still
    /// dead, so keeping one does not hold the fight open.
    #[tracing::instrument(level = "trace", skip_all)]
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
        self.live.creatures[at].hp =
            (hp - V::Health::from(amount.min(V::Number::from(hp)))).max(health_zero);
        let lost = V::Number::from(hp - self.live.creatures[at].hp);
        let overkill = match killed {
            true => (amount - V::Number::from(hp)).max(zero),
            false => zero,
        };
        (lost, overkill, killed)
    }

    fn kill(&mut self, at: usize) {
        self.die(at);
        self.end_if_ending();
    }

    /// `CheckWinCondition` once a creature leaves: a fight with no primary
    /// enemy left ends, won.
    fn end_if_ending(&mut self) {
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
                self.die(at);
            }
        }
        self.ended = Some(super::Ended::Won);
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
        self.ended == Some(super::Ended::Lost) || self.ending()
    }

    /// A command stops on: a loss waits, or the win this effect saw still
    /// holds. Cheaper than [`Board::over`] where nothing has ended.
    fn stopped(&self) -> bool {
        match self.ended {
            None => false,
            Some(super::Ended::Lost) => true,
            Some(super::Ended::Won) => self.ending(),
        }
    }

    /// `CombatManager.IsEnding`: no enemy stands that is both alive and
    /// primary, and no listener holds the fight open.
    #[tracing::instrument(level = "trace", skip_all)]
    fn ending(&self) -> bool {
        let primary_enemy_remains = self.live.creatures.iter().any(|creature| {
            creature.side == <V::CombatSide as SideKinds>::ENEMY
                && creature.alive
                && !self.live.powers.of(creature.id).any(|held| {
                    <V as sts2_core::Hooks>::decide_hook(
                        held.listener(
                            creature.carrier(),
                            self.live.history(),
                            &super::live::Onlookers {
                                creatures: &self.live.creatures,
                                powers: &self.live.powers,
                                kept: &self.live.kept,
                                relics: Some(&*self.relics),
                            },
                        ),
                        sts2_core::DecisionHook::ShouldCountForCombatEnd,
                    )
                    .is_decision_and(|counts| !counts)
                })
        });
        !primary_enemy_remains
            && !super::listen::any_decides::<V>(
                self.walk(),
                sts2_core::DecisionHook::ShouldStopCombatFromEnding,
                true,
            )
    }

    /// The fight and the belt, as a walk reads them.
    pub(super) fn walk(&self) -> Walk<'_, V> {
        Walk {
            live: self.live,
            relics: self.relics,
        }
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
    fn die(&mut self, at: usize) {
        let owner = self.live.creatures[at].side;
        let id = self.live.creatures[at].id;
        // `Hook.BeforeDeath`, once its hit points are gone and before
        // anything decides whether it dies.
        let asked = super::listen::notify::<V>(
            self.live,
            self.relics,
            sts2_core::EventHook::BeforeDeath { creature: id },
        );
        for (listener, effect) in asked {
            match effect {
                HookEffect::ReturnCard(card) => self.live.extra_cards.push(card),
                effect => unimplemented!("{listener:?} asking for {effect:?} before a death"),
            }
        }
        // `Hook.ShouldDieLate`, and the model that kept the creature alive.
        let player = self.live.player().id;
        if let Some(preventer) = super::listen::first_deciding::<V>(
            self.walk(),
            sts2_core::DecisionHook::ShouldDieLate {
                creature: id,
                player,
            },
            false,
        ) {
            self.death_prevented(at, preventer);
            return;
        }
        self.live.creatures[at].alive = false;
        // A pet dies the way a monster does, and whether it stays is what its
        // powers say. Only a player's death can lose the fight.
        let pet = self.live.creatures[at].owner.is_some();
        if owner != <V::CombatSide as SideKinds>::ENEMY && !pet {
            if self.live.creatures.iter().any(|creature| {
                creature.side == <V::CombatSide as SideKinds>::PLAYER
                    && creature.owner.is_none()
                    && creature.alive
            }) {
                unimplemented!("a player dying beside a player still standing");
            }
            if let Some(effect) = self.after_death(id, owner, false).first() {
                unimplemented!("a power asking for {effect:?} over a player's death")
            }
            self.strip_powers(at);
            self.ended = Some(super::Ended::Lost);
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
        if kept {
            let ascension = self.live.ascension;
            for effect in after_death {
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
                    sts2_core::HookEffect::EnterMove(move_id) => {
                        let monster = held.monster;
                        // A move the graph cannot leave yet is left standing.
                        // That is the command's own guard, and not a failure
                        // this has to answer for.
                        if let Ok(next) =
                            V::set_move_immediate(monster, ascension, move_id, &mut held.machine)
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
                        let id = super::monster::summon(
                            self.live,
                            self.relics,
                            monster,
                            position + offset,
                            None,
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
                        self.relics,
                        sts2_core::Spawned {
                            monster,
                            opens_on,
                            slot,
                        },
                        sts2_core::MonsterModel::written(monster, written),
                        position,
                        self.rngs.niche,
                    );
                    self.arrived(id, monster);
                    self.roll_joined(id);
                }
                // `PowerCmd.Apply` from nobody, which the Possess powers
                // hand back with.
                sts2_core::HookEffect::ApplyPower {
                    target,
                    power,
                    amount,
                } => self.land(target, power, amount, None),
                other => unimplemented!("a dead power asking for {other:?}"),
            }
        }
        // Nothing that reads the fight after the death commands can see the
        // dead monster. The observation therefore omits it rather than
        // reporting it dead, and the numbers its powers counted go with it.
        self.live.creatures.retain(|creature| creature.id != id);
        self.live.powers.clear(id);
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
                        self.live.history(),
                        &super::live::Onlookers {
                            creatures: &self.live.creatures,
                            powers: &self.live.powers,
                            kept: &self.live.kept,
                            relics: Some(&*self.relics),
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
    /// A model other than a relic keeping a creature alive, or anything
    /// asked for that nothing here carries out.
    fn death_prevented(&mut self, at: usize, preventer: sts2_core::ModelId<V>) {
        let creature = &self.live.creatures[at];
        let (id, side) = (creature.id, creature.side);
        let max_hp: V::Number =
            num_traits::cast(creature.max_hp).expect("maximum health fits a game number");
        if let Some(effect) = self.after_death(id, side, true).first() {
            unimplemented!("a power asking for {effect:?} over a death kept from happening")
        }
        let sts2_core::ModelId::Relic(relic) = preventer else {
            unimplemented!("{preventer:?} keeping a creature alive")
        };
        let hook = sts2_core::EventHook::DeathPrevented { max_hp };
        for effect in super::listen::ask_relic(self.relics, relic, hook) {
            match effect {
                HookEffect::Heal(amount) => sts2_core::Fight::heal(self, id, amount),
                effect => {
                    unimplemented!("{relic:?} asking for {effect:?} as it keeps {id:?} alive")
                }
            }
        }
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
        let asked = super::listen::notify::<V>(
            self.live,
            self.relics,
            sts2_core::EventHook::AfterDeath {
                creature,
                side,
                monster,
                removal_prevented,
            },
        );
        let mut own = sts2_core::HookEffects::new();
        for (listener, effect) in asked {
            match (listener, effect) {
                (
                    Listener::Relic { .. },
                    sts2_core::HookEffect::GainEnergy(_) | sts2_core::HookEffect::Draw(_),
                ) if self.ending() => {}
                (Listener::Relic { .. }, effect) => super::hook::apply::<V, _>(self, [effect]),
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
                // `CreatureCmd.GainBlock`, unpowered, on the power's owner.
                (Listener::Power { owner, .. }, HookEffect::GainBlock(amount)) => {
                    let block = self.resolve(sts2_core::Effect::GainBlock {
                        recipient: owner,
                        amount: sts2_core::Amount::Number(amount),
                        scaling: Scaling::Static,
                    });
                    sts2_core::Fight::apply(self, block);
                }
                (Listener::Card { id }, HookEffect::ModifySelf { change, lasting }) => {
                    self.live.modify_self(id, change, lasting)
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
        if let Ok(next) = V::set_move_immediate(held.monster, ascension, to, &mut held.machine) {
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
    fn hp_lost_after_osty(&mut self, target: CombatId, amount: V::Number) -> V::Number {
        let lost = after_osty::<V>(self.walk(), target, amount);
        if !super::listen::heard::<V>(
            self.live,
            self.relics,
            sts2_core::ModelHook::AfterModifyingHpLostAfterOsty,
        ) {
            return lost;
        }
        let float = |number: V::Number| -> f64 {
            num_traits::cast(number).expect("a number a fight works out fits a float")
        };
        let mut number = float(amount);
        let mut modifiers: SmallVec<[(CombatId, V::Power); 2]> = SmallVec::new();
        let player = self.live.player().id;
        for phase in [
            ModifyHook::HpLostAfterOsty {
                target,
                player,
                in_fight: true,
            },
            ModifyHook::HpLostAfterOstyLate { target, player },
        ] {
            super::listen::each::<V>(
                self.walk(),
                sts2_core::Overrides::of(phase.model_hook()),
                |listener, _| {
                    let before = number;
                    number = match <V as sts2_core::Hooks>::modify_hook(listener, phase) {
                        Modify::Nothing => number,
                        Modify::Added(more) => number + float(more),
                        Modify::Scaled(by) => number * by,
                        Modify::AtMost(most) => number.min(float(most)),
                        Modify::Replaced(with) => float(with),
                    };
                    if before.trunc() != number.trunc() {
                        match listener {
                            sts2_core::Listener::Power { holder, power, .. } => {
                                modifiers.push((holder, power))
                            }
                            // Every relic's `AfterModifyingHpLostAfterOsty`
                            // only flashes.
                            sts2_core::Listener::Relic(_) => {}
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
                Some(self.relics),
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
    /// Whether each death triggers Fatal is read before the first. The
    /// powers that answer it outlive their owner, so the answer is the same.
    ///
    /// # Panics
    ///
    /// A listener other than a relic to `AfterDiedToDoom`.
    pub(super) fn doom_kill(&mut self, doomed: &[CombatId]) {
        let player = self.live.player().id;
        let fatal = doomed
            .iter()
            .filter(|&&creature| {
                creature != player && sts2_core::Fight::death_triggers_fatal(self, creature)
            })
            .count();
        for &creature in doomed {
            self.kill_outright(creature);
        }
        let asked = super::listen::notify::<V>(
            self.live,
            self.relics,
            sts2_core::EventHook::DiedToDoom { fatal },
        );
        for (listener, effect) in asked {
            match listener {
                Listener::Relic { .. } => super::hook::apply::<V, _>(self, [effect]),
                listener => unimplemented!("{listener:?} asking for {effect:?} after Doom kills"),
            }
        }
    }

    /// `CreatureCmd.Kill`: the creature loses what hit points it has, and
    /// dies.
    fn kill_outright(&mut self, creature: CombatId) {
        let Some(at) = self.at(creature) else {
            return;
        };
        let hp = V::Number::from(self.live.creatures[at].hp);
        self.lose_hp(creature, hp);
        if hp > V::Number::zero() {
            self.current_hp_changed(creature, -hp);
        }
        if let Some(at) = self.at(creature) {
            self.kill(at);
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
        let player = self.live.player();
        let hook = sts2_core::EventHook::CurrentHpChanged {
            creature,
            monster,
            pet_of,
            delta,
            in_fight: true,
            player: sts2_core::PlayerHealth {
                hp: player.hp,
                max_hp: player.max_hp,
            },
        };
        let asked = super::listen::notify::<V>(self.live, self.relics, hook);
        self.after_hit(asked);
    }

    /// Carry out what the powers asked for after a hit or a broken block,
    /// each on the creature carrying it.
    fn after_hit(&mut self, asked: impl IntoIterator<Item = (Listener<V>, HookEffect<V>)>) {
        for (listener, effect) in asked {
            // `CardPileCmd.Draw` for the relic's owner, the player.
            if let (Listener::Relic { .. }, HookEffect::Draw(count)) = (listener, effect) {
                sts2_core::Fight::draw(self, count);
                continue;
            }
            // `PowerCmd.Apply` on the relic's owner, the player.
            if let (Listener::Relic { .. }, HookEffect::ApplyToOwner(power, amount)) =
                (listener, effect)
            {
                let player = self.live.player().id;
                self.land(player, power, amount, Some(player));
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
                HookEffect::AddToDrawAtRandom { card, count } => {
                    let count: usize = num_traits::cast(count)
                        .unwrap_or_else(|| unimplemented!("{count:?} cards into the draw pile"));
                    for _ in 0..count {
                        self.add_to_draw_at_random(card);
                    }
                }
                HookEffect::KillOwner => self.kill_outright(owner),
                // `OstyCmd.Summon` for the player.
                HookEffect::Summon(amount) => sts2_core::Fight::summon_osty(self, amount),
                // `CreatureCmd.Damage` over `HittableEnemies`, from the
                // power's owner, unblockable and unpowered.
                HookEffect::DamageOpponentsUnblockable(amount) => {
                    for target in self.hittable_opponents_of(owner) {
                        self.hit_unpowered(Some(owner), target, amount, true);
                    }
                }
                // The same, and blockable.
                HookEffect::DamageOpponents(amount) => {
                    for target in self.hittable_opponents_of(owner) {
                        self.hit_unpowered(Some(owner), target, amount, false);
                    }
                }
                HookEffect::ApplyPower {
                    target,
                    power,
                    amount,
                } => self.land(target, power, amount, Some(owner)),
                HookEffect::LoseMaxHpOf { target, amount } => {
                    sts2_core::Fight::lose_max_hp(self, target, amount)
                }
                // `CreatureCmd.Damage`, unpowered, from the power's owner.
                HookEffect::Damage { target, amount } => {
                    let hit = resolve::<V>(
                        self.walk(),
                        None,
                        Effect::Attack {
                            dealer: owner,
                            target,
                            damage: Amount::Number(amount),
                            scaling: Scaling::Static,
                        },
                    );
                    self.apply(hit);
                }
                HookEffect::StunOwner { stun, then } => self.stun_monster(owner, stun, then),
                effect => unimplemented!("a power asking for {effect:?} after a hit"),
            }
        }
    }
}

/// `amount` run through `phases` in order, which is how `Hook` modifies a
/// number. Each listener in a phase is handed what the one before it left. The
/// result is floored at zero.
///
/// One walk asks every phase a listener overrides. An answer never reads the
/// number it is handed, so the phases need no walk each.
///
/// A run of shares multiplies together before it lands.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn modified_by<V: Rules>(
    fight: Walk<'_, V>,
    amount: V::Number,
    phases: &[ModifyHook<V>],
) -> V::Number {
    scaled_by::<V>(fight, amount, None, phases)
}

fn float<V: Rules>(number: V::Number) -> f64 {
    num_traits::cast(number).expect("a number a fight works out fits a float")
}

/// [`modified_by`] over a number an enchantment scaled first, where `scale`
/// says so. The game holds the scaled number as a decimal until the
/// listeners are done.
fn scaled_by<V: Rules>(
    fight: Walk<'_, V>,
    amount: V::Number,
    scale: Option<f64>,
    phases: &[ModifyHook<V>],
) -> V::Number {
    num_traits::cast(scaled_exactly::<V>(fight, amount, scale, phases).trunc())
        .expect("a modified number is a game number")
}

/// [`scaled_by`] before the floor, which is the decimal the game's hook
/// hands back. Never below nothing.
fn scaled_exactly<V: Rules>(
    fight: Walk<'_, V>,
    amount: V::Number,
    scale: Option<f64>,
    phases: &[ModifyHook<V>],
) -> f64 {
    let hooks = phases
        .iter()
        .fold(sts2_core::Overrides::default(), |hooks, phase| {
            hooks.with(phase.model_hook())
        });
    let mut answers: SmallVec<[(usize, Modify<V::Number>); 8]> = SmallVec::new();
    super::listen::each::<V>(fight, hooks, |listener, overrides| {
        for (at, phase) in phases.iter().enumerate() {
            if overrides.contains(phase.model_hook()) {
                match <V as sts2_core::Hooks>::modify_hook(listener, *phase) {
                    Modify::Nothing => {}
                    answer => answers.push((at, answer)),
                }
            }
        }
    });
    // A number nothing changes is only floored.
    if answers.is_empty() {
        return (float::<V>(amount) * scale.unwrap_or(1.0)).max(0.0);
    }

    let float = float::<V>;
    // Phase by phase, and within a phase in the order the walk asked. The
    // sort is stable, so it keeps that order.
    answers.sort_by_key(|&(at, _)| at);
    let mut number = float(amount) * scale.unwrap_or(1.0);
    let mut share = 1.0f64;
    let mut phase = answers[0].0;
    for &(at, answer) in &answers {
        if at != phase {
            number *= share;
            share = 1.0;
            phase = at;
        }
        match answer {
            Modify::Nothing => {}
            Modify::Scaled(by) => share *= by,
            Modify::Added(more) => {
                number = number * share + float(more);
                share = 1.0;
            }
            Modify::AtMost(most) => {
                number = (number * share).min(float(most));
                share = 1.0;
            }
            Modify::Replaced(with) => {
                number = float(with);
                share = 1.0;
            }
        }
    }
    number *= share;
    number.max(0.0)
}

/// The most orb slots `OrbCmd.AddSlots` gives a player.
const MAX_ORB_SLOTS: u8 = 10;

/// `ModifyHpLost` in the phases after a hit found who takes it.
fn after_osty<V: Rules>(fight: Walk<'_, V>, target: CombatId, amount: V::Number) -> V::Number {
    let player = fight.live.player().id;
    modified_by::<V>(
        fight,
        amount,
        &[
            ModifyHook::HpLostAfterOsty {
                target,
                player,
                in_fight: true,
            },
            ModifyHook::HpLostAfterOstyLate { target, player },
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
    fight: Walk<'_, V>,
    dealer: Option<CombatId>,
    target: Option<CombatId>,
    card: Option<&CardInstance<V>>,
    damage: V::Number,
    powered: bool,
) -> V::Number {
    num_traits::cast(modified_exactly::<V>(fight, dealer, target, card, damage, powered).trunc())
        .expect("a modified number is a game number")
}

/// [`modified`] before the floor, which is the decimal `Hook.ModifyDamage`
/// hands back.
fn modified_exactly<V: Rules>(
    fight: Walk<'_, V>,
    dealer: Option<CombatId>,
    target: Option<CombatId>,
    card: Option<&CardInstance<V>>,
    damage: V::Number,
    powered: bool,
) -> f64 {
    // The card's enchantment has its say first, and only when a card deals
    // the hit. Its scale stays a decimal until the listeners are done.
    let enchanted = card.and_then(|held| {
        held.face
            .ident
            .enchantment
            .map(|enchanted| (enchanted, held.face.enchantment_status()))
    });
    let (damage, scale) = match enchanted {
        Some((enchanted, status)) => {
            let added = enchanted_by::<V>(
                enchanted,
                status,
                EnchantHook::DamageAdditive { powered },
                damage,
            );
            match <V as sts2_core::Hooks>::enchant_modify(
                enchanted,
                status,
                EnchantHook::DamageMultiplicative { powered },
            ) {
                Modify::Nothing => (added, None),
                Modify::Scaled(by) => (added, Some(by)),
                answer => unimplemented!("an enchantment scaling a hit with {answer:?}"),
            }
        }
        None => (damage, None),
    };
    let card = card.map(|held| sts2_core::CardSource {
        id: held.id,
        card: held.face.ident.card,
        playing: fight.live.cards.in_play(held.id),
        upgraded: held.face.ident.upgrade > 0,
    });
    let phases = [
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
    ];
    scaled_exactly::<V>(fight, damage, scale, &phases)
}

/// `amount` after an enchantment's `hook`, which the game asks before any
/// listener.
///
/// # Panics
///
/// An enchantment that does more to the number than add to it.
pub(super) fn enchanted_by<V: Rules>(
    enchanted: sts2_core::Enchanted<V>,
    status: sts2_core::EnchantmentStatus,
    hook: EnchantHook,
    amount: V::Number,
) -> V::Number {
    match <V as sts2_core::Hooks>::enchant_modify(enchanted, status, hook) {
        Modify::Nothing => amount,
        Modify::Added(more) => amount + more,
        answer => unimplemented!("an enchantment answering {hook:?} with {answer:?}"),
    }
}

/// `PlayerCombatState.MaxEnergy`: the player's own, after every listener
/// has had its say.
pub(crate) fn max_energy<V: Rules>(fight: Walk<'_, V>, base: V::Count) -> V::Count {
    let player = player::<V>(&fight.live.creatures);
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
pub(crate) fn orb_value<V: Rules>(fight: Walk<'_, V>, orb: V::Orb, base: V::Number) -> V::Number {
    let player = player::<V>(&fight.live.creatures);
    modified_by::<V>(fight, base, &[ModifyHook::OrbValue { player, orb }])
}

/// One block gain after every listener in the fight has had its say, which
/// is `Hook.ModifyBlock`.
///
/// The same hook whether the block is previewed or gained, as damage's is.
/// `powered` is `IsPoweredCardOrMonsterMoveBlock`, and `card` is the card
/// whose own play gives it. `play` is that play, while one is under way.
pub(super) fn blocked<V: Rules>(
    fight: Walk<'_, V>,
    recipient: CombatId,
    amount: V::Number,
    powered: bool,
    card: Option<V::Card>,
    play: Option<sts2_core::PlayAt>,
    source: Option<CardId<V>>,
) -> (V::Number, SmallVec<[V::Relic; 1]>) {
    let phases = [
        ModifyHook::BlockAdditive {
            target: recipient,
            powered,
            card,
        },
        ModifyHook::BlockMultiplicative {
            target: recipient,
            powered,
            card,
            play,
            source,
        },
    ];
    let amount = modified_by::<V>(fight, amount, &phases);
    // The `modifiers` `Hook.ModifyBlock` hands `AfterModifyingBlockAmount`:
    // the listeners whose answer moved the number. Only a relic acts on it.
    let mut modifying = SmallVec::new();
    let after = sts2_core::ModelHook::AfterModifyingBlockAmount;
    if super::listen::heard::<V>(fight.live, fight.relics, after) {
        for phase in phases {
            let hook = sts2_core::Overrides::of(phase.model_hook());
            super::listen::each::<V>(fight, hook, |listener, _| {
                let sts2_core::Listener::Relic(state) = listener else {
                    return;
                };
                let moved = match <V as sts2_core::Hooks>::modify_hook(listener, phase) {
                    Modify::Nothing => false,
                    Modify::Added(added) => added != V::Number::zero(),
                    Modify::Scaled(share) => share != 1.0,
                    answer => unimplemented!("a relic answering {answer:?} for block"),
                };
                let held = fight
                    .relics
                    .iter()
                    .find(|held| std::ptr::eq(&held.state, state))
                    .expect("a relic asked is on the belt");
                if moved && !modifying.contains(&held.relic) {
                    modifying.push(held.relic);
                }
            });
        }
    }
    (amount, modifying)
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
        Some(already) => already == affliction,
    }
}

/// A listener whose `TryModifyPowerAmountReceived` changed an amount.
#[derive(Debug, Clone, Copy)]
pub(super) enum Receiver<V: Rules> {
    /// A power, by the creature carrying it and the power it is.
    Power(CombatId, V::Power),
    Relic(V::Relic),
}

/// The listeners whose `TryModifyPowerAmountReceived` changed an amount.
pub(super) type Received<V> = SmallVec<[Receiver<V>; 1]>;

/// `Hook.ModifyPowerAmountGiven`, over the listeners in the fight: what
/// `amount` of `power` from `giver` to `target` becomes, and the relics that
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
    let player = fight.live.player().id;
    let mut amount = amount;
    let mut modified = |hook: ModelHook, asked: ModifyHook<V>, amount: &mut V::Number| {
        if !super::listen::heard::<V>(fight.live, fight.relics, hook) {
            return;
        }
        super::listen::each::<V>(fight, sts2_core::Overrides::of(hook), |listener, _| {
            let moved = match (
                <V as sts2_core::Hooks>::modify_hook(listener, asked),
                listener,
            ) {
                (Modify::Nothing, _) => false,
                (Modify::Added(added), sts2_core::Listener::Relic(_)) => {
                    *amount = *amount + added;
                    added != V::Number::zero()
                }
                (Modify::Scaled(share), sts2_core::Listener::Relic(_)) => {
                    let scaled = num_traits::ToPrimitive::to_f64(amount)
                        .expect("a power's amount is a finite number")
                        * share;
                    *amount = num_traits::cast(scaled)
                        .unwrap_or_else(|| unimplemented!("{power:?} given as {scaled}"));
                    share != 1.0
                }
                (answer, _) => {
                    unimplemented!(
                        "a listener answering {answer:?} for what {power:?} is given with"
                    )
                }
            };
            if moved && let sts2_core::Listener::Relic(state) = listener {
                let held = fight
                    .relics
                    .iter()
                    .find(|held| std::ptr::eq(&held.state, state))
                    .expect("a relic asked is on the belt");
                modifying.push(held.relic);
            }
        });
    };
    let additive = ModifyHook::PowerAmountGivenAdditive {
        power,
        giver,
        target,
        player,
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
        player,
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
    if !super::listen::heard::<V>(fight.live, fight.relics, hook) {
        return (amount, modifiers);
    }
    let mut amount = amount;
    super::listen::each::<V>(fight, sts2_core::Overrides::of(hook), |listener, _| {
        let asked = ModifyHook::PowerAmountReceived {
            power,
            target,
            amount,
            player: fight.live.player().id,
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
            (Modify::Replaced(with), sts2_core::Listener::Relic(state)) => {
                amount = with;
                let held = fight
                    .relics
                    .iter()
                    .find(|held| std::ptr::eq(&held.state, state))
                    .expect("a relic asked is on the belt");
                modifiers.push(Receiver::Relic(held.relic));
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
                let damage = carried::<V>(playing, fight.live, damage, Some(target));
                let powered = scaling == Scaling::Dynamic;
                modified::<V>(fight, Some(dealer), Some(target), playing, damage, powered)
            },
        },
        Effect::GainBlock {
            recipient,
            amount,
            scaling,
        } => {
            // A block effect names who gains it, and no creature the card
            // points at.
            let amount = carried::<V>(playing, fight.live, amount, None);
            // `Hook.ModifyBlock` asks the played card's enchantment whatever
            // the block's props, since `GainBlock` always runs it.
            let amount = match playing.and_then(|held| {
                held.face
                    .ident
                    .enchantment
                    .map(|e| (e, held.face.enchantment_status()))
            }) {
                Some((enchanted, status)) => {
                    enchanted_by::<V>(enchanted, status, EnchantHook::BlockAdditive, amount)
                }
                None => amount,
            };
            let powered = scaling == Scaling::Dynamic;
            let play = playing.and_then(|held| fight.live.history.play_at(held.id));
            let source = playing.map(|held| held.id);
            // `CreatureCmd.GainBlock` always runs `Hook.ModifyBlock`. A card's
            // block and a monster move's are the powered ones.
            let (amount, modifying) = blocked::<V>(
                fight,
                recipient,
                amount,
                powered,
                playing.map(|held| held.face.ident.card),
                play,
                source,
            );
            Resolved::GainBlock {
                recipient,
                amount,
                powered,
                play,
                source,
                modifying,
            }
        }
    };
    ResolvedEffect { effect }
}

/// The number an effect carries, before any hook of its own runs.
fn carried<V: Rules>(
    playing: Option<&CardInstance<V>>,
    live: &Live<V>,
    amount: Amount<V>,
    target: Option<CombatId>,
) -> V::Number {
    match amount {
        Amount::Number(number) => number,
        Amount::Var(key) => var::<V>(
            playing
                .unwrap_or_else(|| unimplemented!("a card's variable with no card being played")),
            key,
            live,
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
    held: &CardInstance<V>,
    key: V::CardVar,
    live: &Live<V>,
    target: Option<CombatId>,
) -> V::Number {
    let number = held.face.number(key);
    match number.calculated {
        false => number.base,
        true => {
            let standing = osty::<V>(&live.creatures).filter(|osty| osty.alive);
            let osty = standing.map(|osty| osty.hp);
            let osty_max_hp = standing.map(|osty| osty.max_hp);
            let targeted: Option<SmallVec<[Stack<V>; 8]>> =
                target.map(|target| live.powers.of(target).map(HeldPower::stack).collect());
            let exhausted: SmallVec<[V::Card; 16]> = live
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
            let hand: SmallVec<[V::Card; 12]> = live
                .cards
                .iter(Pile::Hand)
                .map(|held| held.face.ident.card)
                .collect();
            let owner = sts2_core::CardOwner {
                powers: &powers::<V>(live),
                osty,
                osty_max_hp,
                draw: live.cards.len(Pile::Draw),
                exhaust: &exhausted,
                hand_besides: live.cards.len(Pile::Hand)
                    - usize::from(live.cards.find(Pile::Hand, held.id).is_some()),
                enemy_powers: &enemy_powers,
                strikes: live.cards.count_all(|face| {
                    sts2_core::CardTable::tagged(
                        face.ident.card,
                        <<V::Card as sts2_core::CardTable>::Tag as sts2_core::CardTagKinds>::STRIKE,
                    )
                }),
                player: live.player().id,
                targeted: target,
                target: targeted.as_deref(),
                history: live.history(),
                block: live
                    .creatures
                    .iter()
                    .find(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER)
                    .map_or_else(num_traits::zero, |player| player.block),
                orbs: live.orbs.as_ref().map_or(&[], |orbs| orbs.queue.as_slice()),
                hand: &hand,
                star_cards: live.cards.count_all(|face| {
                    sts2_core::CardTable::star_cost(face.ident.card) != sts2_core::Cost::None
                }),
                osty_attacks_besides: {
                    let osty_attack = |card| {
                        sts2_core::CardTable::tagged(
                            card,
                            <<V::Card as sts2_core::CardTable>::Tag as sts2_core::CardTagKinds>::OSTY_ATTACK,
                        )
                    };
                    live.cards.count_all(|face| osty_attack(face.ident.card))
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
            // Every block a card prints on this build is a `BlockVar` with
            // `ValueProp.Move`.
            scaling: Scaling::Dynamic,
        }),
        // Not an effect a card resolves: the fight asks the hook itself.
        CardNumberModifier::PowerGiven => None,
    }
}

/// Who deals or gains a card's number: the player, or for an Osty's damage
/// `Owner.Osty`, which is Osty dead or alive.
///
/// # Panics
///
/// Osty's damage on a player who never summoned one, where the game hands the
/// hooks no dealer at all.
pub(crate) fn dealer_of<V: Rules>(
    modifier: CardNumberModifier,
    creatures: &[Fighter<V>],
) -> CombatId {
    match modifier {
        CardNumberModifier::OstyDamage => {
            osty::<V>(creatures)
                .unwrap_or_else(|| unimplemented!("Osty's damage with no Osty"))
                .id
        }
        CardNumberModifier::Damage
        | CardNumberModifier::UnpoweredDamage
        | CardNumberModifier::Block
        | CardNumberModifier::PowerGiven => player::<V>(creatures),
    }
}

/// `Owner.Osty`: the player's Osty, dead or alive, once one is summoned.
pub(super) fn osty<V: Rules>(creatures: &[Fighter<V>]) -> Option<&Fighter<V>> {
    let player = player::<V>(creatures);
    let osty = Some(sts2_core::ModelId::Monster(
        <V::Monster as sts2_core::MonsterKinds>::OSTY,
    ));
    creatures
        .iter()
        .find(|creature| creature.owner == Some(player) && creature.model == osty)
}

/// The player, which every fight has one of.
pub(super) fn player<V: Rules>(creatures: &[Fighter<V>]) -> CombatId {
    creatures
        .iter()
        .find(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER)
        .expect("a fight has the player in it")
        .id
}

impl<V: Rules> Board<'_, V> {
    /// `CreateClone` of the card `id` into `pile`, as
    /// `AddGeneratedCardToCombat` puts it there.
    pub(super) fn clone_named(
        &mut self,
        id: sts2_core::CardId<V>,
        pile: Pile,
    ) -> Option<CardAt<V>> {
        let face = self
            .live
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
    fn clone_face(&mut self, face: Face<V>, pile: Pile) -> Option<CardAt<V>> {
        let id = self.live.card_ids.mint();
        // A full hand sends the card to the discard pile instead.
        let full =
            pile == Pile::Hand && self.live.cards.len(Pile::Hand) >= super::pile::MAX_CARDS_IN_HAND;
        let pile = if full { Pile::Discard } else { pile };
        let index = num_traits::NumCast::from(self.live.cards.len(pile))
            .unwrap_or_else(|| unimplemented!("a pile deeper than a fight can number"));
        self.live.cards.push(pile, face.named(id));
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
        if self.over() {
            return;
        }
        let Some(orb) = self.live.orbs.as_mut().and_then(|orbs| {
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
        let targets = V::orb_evoke(&orb, self);
        let hook = sts2_core::EventHook::OrbEvoked { orb: orb.orb };
        for (listener, effect) in super::listen::notify::<V>(self.live, self.relics, hook) {
            match (listener, effect) {
                // `CreatureCmd.Damage` on the evoke's targets still alive.
                (Listener::Power { owner, .. }, HookEffect::DamageTargets(amount)) => {
                    for &target in &targets {
                        if !self
                            .at(target)
                            .is_some_and(|at| self.live.creatures[at].alive)
                        {
                            continue;
                        }
                        let hit = resolve::<V>(
                            self.walk(),
                            None,
                            Effect::Attack {
                                dealer: owner,
                                target,
                                damage: Amount::Number(amount),
                                scaling: Scaling::Static,
                            },
                        );
                        self.apply(hit);
                    }
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
        let id = self.live.next_creature();
        let at = self
            .live
            .creatures
            .iter()
            .rposition(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER)
            .map_or(0, |last| last + 1);
        let initial = V::monster_hp_bounds(monster, self.ascension).max();
        let player = self.live.player().id;
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
        };
        if let Some((listener, effect)) =
            super::listen::notify::<V>(self.live, self.relics, hook).first()
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
        if super::listen::heard::<V>(self.live, self.relics, hook) {
            unimplemented!("a listener to {}", hook.name());
        }
    }
}

impl<V: Rules> Fight<V> for Board<'_, V> {
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
        self.at(self.actor)
            .map(|at| self.live.creatures[at].side)
            .unwrap_or(<V::CombatSide as SideKinds>::PLAYER)
    }

    fn resolve(&self, effect: Effect<V>) -> Self::ResolvedEffect {
        if self.hits_wait() && matches!(effect, Effect::Attack { .. }) {
            return ResolvedEffect {
                effect: Resolved::Held(effect),
            };
        }
        resolve::<V>(self.walk(), self.acting().as_ref(), effect)
    }

    fn orb_value(&self, orb: V::Orb, base: V::Number) -> V::Number {
        orb_value::<V>(self.walk(), orb, base)
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
        let pausable = self.pausable;
        self.pausable |= self.moving;
        let results = self.attack_landing(attacker, scaling, hits);
        self.pausable = pausable;
        results
    }

    #[tracing::instrument(level = "trace", skip_all)]
    fn apply(&mut self, effect: Self::ResolvedEffect) {
        self.ran_on();
        match effect.effect {
            Resolved::Attack {
                dealer,
                target,
                damage,
                powered,
                card,
            } => self.hit(Some(dealer), target, damage, powered, false, card),
            Resolved::Held(effect) => self.held.push(effect),
            Resolved::GainBlock {
                recipient,
                amount,
                powered,
                play,
                source,
                modifying,
            } => {
                // `Hook.AfterModifyingBlockAmount`, asked of each relic that
                // changed the gain, before it lands.
                for relic in modifying {
                    let hook = sts2_core::EventHook::BlockAmountModified { amount, source };
                    if let Some(effect) = super::listen::ask_relic(self.relics, relic, hook).first()
                    {
                        unimplemented!("{relic:?} asking for {effect:?} once it changed block")
                    }
                }
                let Some(at) = self.at(recipient) else { return };
                if amount > V::Number::zero() {
                    self.live.creatures[at].block = self.live.creatures[at].block + amount;
                    self.live.history.block_gained(sts2_core::BlockGained {
                        receiver: recipient,
                        powered,
                        play,
                    });
                }
            }
        }
    }

    fn remove_power(&mut self, creature: CombatId, power: V::Power) {
        self.ran_on();
        self.remove_power_carried(creature, power);
    }

    fn add_at_random(&mut self, card: V::Card, pile: Pile) -> Option<CardAt<V>> {
        self.ran_on();
        self.add_at_random_here(card, pile)
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
        self.live.creatures[at].max_hp = hp;
        self.live.creatures[at].hp = hp;
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
        self.kill_outright(creature);
    }

    fn self_note(&self) -> V::Number {
        let playing = self.playing.expect("a card behavior has a card");
        self.live.noted(playing)
    }

    fn set_self_note(&mut self, note: V::Number) {
        let playing = self.playing.expect("a card behavior has a card");
        self.live.note(playing, note);
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
        num_traits::cast(self.live.energy).expect("energy fits a game number")
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
                    self.live.history(),
                    &super::live::Onlookers {
                        creatures: &self.live.creatures,
                        powers: &self.live.powers,
                        kept: &self.live.kept,
                        relics: Some(&*self.relics),
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
        let at = self.standing(card);
        let mut face = self
            .live
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
        self.ran_on();
        self.live
            .for_run
            .push(sts2_core::HookEffect::ProcureRandomPotions {
                count: 1,
                stream,
                in_combat: true,
                into_gained: false,
            });
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

    fn lose_max_hp(&mut self, creature: CombatId, amount: V::Number) {
        self.ran_on();
        let Some(at) = self.at(creature) else { return };
        let held = &self.live.creatures[at];
        let max_hp = V::Number::from(held.max_hp) - amount;
        let hp = V::Number::from(held.hp);
        if max_hp < hp {
            self.hit_unpowered(None, creature, hp - max_hp, true);
        }
        let Some(at) = self.at(creature) else { return };
        self.live.creatures[at].max_hp = num_traits::cast(max_hp.max(num_traits::one()))
            .expect("a maximum health fits the build's health");
    }

    fn lose_gold(&mut self, amount: V::Number) {
        self.ran_on();
        self.live.gold = (self.live.gold - amount).max(V::Number::zero());
    }

    fn gain_stars(&mut self, amount: V::Number) {
        self.ran_on();
        // The pool is a count, so a card printing a negative one is spending
        // rather than gaining. No card on this build does.
        if amount < V::Number::zero() {
            unimplemented!("a card spending stars as it plays");
        }
        self.live.modify_stars(amount);
    }

    fn modify_energy(&mut self, amount: V::Number) {
        self.ran_on();
        let size = |amount: V::Number| {
            num_traits::NumCast::from(amount)
                .unwrap_or_else(|| unimplemented!("an energy change of {amount:?}"))
        };
        // `PlayerCombatState` holds the pool as a count and clamps it at zero.
        // `PlayerCmd.GainEnergy` gains nothing once the fight is ending.
        match amount < V::Number::zero() {
            true => self.live.energy = self.live.energy.saturating_sub(size(-amount)),
            false if self.over() => {}
            false => self.live.energy = self.live.energy + size(amount),
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

    fn apply_to_opponents(&mut self, power: V::Power, amount: PowerAmount<V::Number>) {
        // The list is taken before the first lands, as an attack's is.
        for id in self.opponents() {
            self.land(id, power, amount, Some(self.actor));
        }
    }

    fn intent_damage(&self) -> V::Number {
        self.intent_damage
    }

    fn intent_repeats(&self) -> V::Count {
        self.intent_repeats
    }

    fn draw(&mut self, count: V::Number) {
        // `IsOverOrEnding` draws nothing.
        if self.stopped() {
            return;
        }
        // A negative count draws nothing: the game's own loop runs no times.
        let count = num_traits::NumCast::from(count).unwrap_or(0usize);
        if !self.pausable {
            self.ended = draw(
                self.live,
                self.rngs.reborrow(),
                self.relics,
                self.ascension,
                count,
                false,
            );
            return;
        }
        // A question a card this effect auto-played asked stands the draw
        // still, before it starts.
        if self.live.asking.is_some() {
            self.live.defer(super::live::Deferred::Draw {
                count,
                hand_draw: false,
                resumed: false,
            });
            return;
        }
        self.ended = draw_in_effect(
            self.live,
            self.rngs.reborrow(),
            self.relics,
            self.ascension,
            count,
            false,
            false,
        );
    }

    fn ascension(&self) -> V::AscensionLevel {
        self.ascension
    }

    fn var(&self, key: V::CardVar) -> V::Number {
        let held = self
            .acting()
            .unwrap_or_else(|| unimplemented!("a number printed by no card being played"));
        // A power's number runs its hook where the power lands, which knows
        // the applier and the target.
        if held
            .face
            .number(key)
            .modifier
            .is_some_and(|modifier| modifier != CardNumberModifier::PowerGiven)
        {
            // The hooks it runs through name creatures to run against, and a
            // bare read has none. `Fight::resolve` is where both arrive.
            unimplemented!(
                "the {} read without the creatures it runs against",
                key.name()
            );
        }
        var::<V>(&held, key, self.live, self.target)
    }

    fn history(&self) -> sts2_core::History<'_, V> {
        self.live.history()
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
        self.ran_on();
        // A new model for the replacement, so a new id. The original's own is
        // gone with it.
        let id = self.live.card_ids.mint();
        // `Transform` puts the replacement where the original stood, in the
        // pile it stood in. The place came with the name, so this looks once.
        let at = self.standing(card);
        self.live
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
            super::listen::notify::<V>(self.live, self.relics, hook).first()
        {
            unimplemented!("{listener:?} asking for {effect:?} as a card changes piles")
        }
        // `CardCmd.Transform` raises this once every replacement is in, made
        // by the card's owner.
        let player = self.live.player().id;
        self.card_generated(
            CardAt::new(id, card.pile(), card.index()),
            into.card,
            Some(player),
        );
    }

    fn forge(&mut self, card: V::Card, key: V::CardVar, amount: V::Number) {
        self.ran_on();
        // `ForgeCmd.Forge` creates one when the fight holds none it can reach.
        // The exhaust pile is not one it can reach, and is forged all the same.
        let held = self
            .live
            .cards
            .any_outside_exhaust(|held| held.ident.card == card);
        let created = (!held).then(|| {
            let id = self.live.card_ids.mint();
            self.live
                .cards
                .push(Pile::Hand, CardInstance::new(CardIdent::new(card), id));
            let index = num_traits::NumCast::from(self.live.cards.len(Pile::Hand) - 1)
                .unwrap_or_else(|| unimplemented!("a pile deeper than a fight can number"));
            CardAt::new(id, Pile::Hand, index)
        });
        // `IncreaseSovereignBladeDamage`, over `AllCards`: every copy, in
        // every pile, the one being played among them.
        self.live.cards.write_all_cards(
            |held| held.ident.card == card,
            |held| held.raise(key, amount),
        );
        // The blade goes through `AddGeneratedCardToCombat`, so it enters the
        // fight the way a generated card does. It is raised after the raise,
        // which is the order `ForgeCmd` adds and then increases in.
        if let Some(at) = created {
            self.card_entered_combat(at, card, false);
            let player = self.live.player().id;
            self.card_generated(at, card, Some(player));
        }
    }

    fn apply_to_actor(&mut self, power: V::Power, amount: PowerAmount<V::Number>) {
        self.land(self.actor, power, amount, Some(self.actor));
    }

    /// `OrbCmd.Channel`, which does nothing once the fight is ending.
    ///
    /// # Panics
    ///
    /// A listener to `AfterOrbChanneled` asking for anything but damage on
    /// every enemy.
    fn channel(&mut self, orb: V::Orb) {
        self.ran_on();
        if self.over() {
            return;
        }
        let base = V::orb_slots(self.live.character);
        let orbs = self.live.orbs.get_or_insert_with(|| sts2_core::Orbs {
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
        let Some(orbs) = self.live.orbs.as_mut() else {
            return;
        };
        if orbs.capacity == V::Count::zero() {
            return;
        }
        let Some(orbs) = self.live.orbs.as_mut() else {
            return;
        };
        orbs.queue.push(V::channelled(orb));
        let asked =
            super::listen::notify::<V>(self.live, self.relics, sts2_core::EventHook::OrbChanneled);
        let player = self.live.player().id;
        for (listener, effect) in asked {
            match (listener, effect) {
                // `CreatureCmd.Damage` over `HittableEnemies`, unpowered and
                // from the relic's owner.
                (Listener::Relic { .. }, HookEffect::DamageOpponents(amount)) => {
                    for target in self.hittable_opponents_of(player) {
                        self.hit_unpowered(Some(player), target, amount, false);
                    }
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
        self.ran_on();
        use sts2_core::ModelHook;

        self.unheard(ModelHook::ModifySummonAmount);
        if amount == V::Number::zero() {
            return;
        }
        let health: V::Health = num_traits::cast(amount)
            .unwrap_or_else(|| unimplemented!("summoning Osty for {amount:?}"));
        match osty::<V>(&self.live.creatures).map(|osty| (osty.id, osty.alive)) {
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
                let asked = super::listen::notify::<V>(
                    self.live,
                    self.relics,
                    sts2_core::EventHook::OstyRevived,
                );
                if let Some((listener, effect)) = asked.first() {
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
        osty::<V>(&self.live.creatures)
            .filter(|osty| osty.alive)
            .map(|osty| osty.id)
    }

    fn orbs_held(&self) -> usize {
        self.live.orbs.as_ref().map_or(0, |orbs| orbs.queue.len())
    }

    fn channelled(&self) -> Vec<sts2_core::Channelled<V>> {
        self.live
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
        self.ran_on();
        if cards.is_empty() || self.over() {
            return;
        }
        let (sly, ended) = super::turn::act_on::<V>(
            self.live,
            self.rngs.reborrow(),
            self.relics,
            sts2_core::PileAct::Discard,
            cards,
        );
        self.ended = self.ended.or(ended);
        if !self.stopped() && draw > V::Number::zero() {
            sts2_core::Fight::draw(self, draw);
        }
        if !self.stopped() {
            self.ended = super::turn::play_sly::<V>(
                self.live,
                self.rngs.reborrow(),
                self.relics,
                sly,
                self.pausable,
            );
        }
    }

    fn card_costs(&self, card: CardAt<V>, global: bool) -> (sts2_core::Cost, sts2_core::Cost) {
        let at = self.standing(card);
        let held = self
            .live
            .cards
            .get(card.pile(), at)
            .expect("a card stands where it was found");
        match global {
            false => held.face.costs(|cost| cost),
            true => super::legal::costs::<V>(
                &held,
                super::listen::Walk {
                    live: self.live,
                    relics: self.relics,
                },
            ),
        }
    }

    fn printed_costs(&self, card: CardAt<V>) -> (sts2_core::Cost, sts2_core::Cost) {
        let at = self.standing(card);
        let held = self
            .live
            .cards
            .get(card.pile(), at)
            .expect("a card stands where it was found");
        (held.face.cost, held.face.star_cost)
    }

    fn card_keyword(&self, card: CardAt<V>, keyword: V::CardKeyword) -> bool {
        let at = self.standing(card);
        self.live
            .cards
            .get(card.pile(), at)
            .expect("a card stands where it was found")
            .face
            .has_keyword(keyword)
    }

    fn local_cost(&self, card: CardAt<V>) -> sts2_core::Cost {
        let at = self.standing(card);
        self.live
            .cards
            .get(card.pile(), at)
            .expect("a card stands where it was found")
            .face
            .costs(|cost| cost)
            .0
    }

    fn random_card_in(&mut self, pile: Pile) -> Option<CardAt<V>> {
        use sts2_core::rng::GameRng as _;

        let cards = sts2_core::Fight::cards_in(self, pile);
        self.rngs.selection.choose(&cards).ok().copied()
    }

    fn auto_play_from_draw(&mut self, count: usize, force_exhaust: bool) {
        if self.over() {
            return;
        }
        let mut taken: SmallVec<[CardInstance<V>; 4]> = SmallVec::new();
        for _ in 0..count {
            // `ShuffleIfNecessary`.
            if self.live.cards.is_empty(Pile::Draw) {
                let ascension = self.live.ascension;
                shuffle::<V>(self.live, self.rngs.reborrow(), self.relics, ascension);
            }
            if self.live.cards.is_empty(Pile::Draw) {
                break;
            }
            taken.push(self.live.cards.play(Pile::Draw, 0));
        }
        for card in taken {
            if !self.live.player().alive {
                continue;
            }
            // `ExhaustOnNextPlay = forceExhaust`, which clears it as well.
            self.live.exhaust_next.retain(|id| *id != card.id);
            if force_exhaust {
                self.live.exhaust_next.push(card.id);
            }
            let ended = super::play::auto_play_in_play::<V>(
                self.live,
                self.rngs.reborrow(),
                self.relics,
                card,
                None,
                false,
            );
            if ended.is_some() {
                self.ended = ended;
                return;
            }
        }
    }

    fn add_orb_slots(&mut self, amount: V::Number) {
        self.ran_on();
        if self.over() {
            return;
        }
        let orbs = self.live.orbs.get_or_insert_with(|| sts2_core::Orbs {
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
        if self.over() {
            return;
        }
        let Some(orbs) = self.live.orbs.as_mut() else {
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
        self.ran_on();
        self.hit_unpowered(Some(self.actor), target, amount, true);
    }

    fn lose_block(&mut self, creature: CombatId, amount: V::Number) {
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
            let asked = super::listen::notify::<V>(
                self.live,
                self.relics,
                sts2_core::EventHook::BlockBroken { creature },
            );
            self.after_hit(asked);
        }
    }

    fn card_number(&self, card: CardAt<V>, key: V::CardVar) -> V::Number {
        let at = self.standing(card);
        let held = self
            .live
            .cards
            .get(card.pile(), at)
            .expect("a card stands where it was found");
        var::<V>(&held, key, self.live, None)
    }

    fn card_damage(&self, card: CardAt<V>, damage: V::Number) -> V::Number {
        let at = self.standing(card);
        let held = self
            .live
            .cards
            .get(card.pile(), at)
            .expect("a card stands where it was found");
        let player = self.live.player().id;
        let exactly =
            modified_exactly::<V>(self.walk(), Some(player), None, Some(&held), damage, true);
        if exactly.fract() != 0.0 {
            unimplemented!("a card's damage worked out to the fraction {exactly}");
        }
        num_traits::cast(exactly).expect("a modified number is a game number")
    }

    fn auto_play_card(&mut self, card: CardAt<V>) {
        if self.over() || !self.live.player().alive {
            return;
        }
        self.standing(card);
        self.ended = super::play::auto_play::<V>(
            self.live,
            self.rngs.reborrow(),
            self.relics,
            card.id(),
            card.pile(),
            None,
            self.pausable,
        );
    }

    fn auto_play_card_at(&mut self, card: CardAt<V>, target: CombatId) {
        if self.over() || !self.live.player().alive {
            return;
        }
        self.standing(card);
        self.ended = super::play::auto_play::<V>(
            self.live,
            self.rngs.reborrow(),
            self.relics,
            card.id(),
            card.pile(),
            Some(target),
            self.pausable,
        );
    }

    fn stable_shuffle_cards(&mut self, cards: &mut [CardAt<V>], stream: V::RunRngType) {
        use sts2_core::rng::GameRng as _;

        let rng = match stream {
            stream if stream == V::RunRngType::SHUFFLE => &mut self.rngs.shuffle,
            stream => unimplemented!("cards in the fight shuffled off {stream:?}"),
        };
        let cards_held = &self.live.cards;
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
        let held = self
            .live
            .cards
            .named(card)
            .unwrap_or_else(|| unimplemented!("a play count for a card that left the fight"));
        super::play::play_count::<V, _>(self.live, &mut self.rngs, self.relics, &held)
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
        self.unheard(sts2_core::ModelHook::ModifyXValue);
        let started = self
            .live
            .history()
            .started_this_turn()
            .last()
            .expect("an X-cost card resolving its X is a play started this turn");
        num_traits::cast(started.energy).expect("an energy fits a game number")
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
        if self.stopped() {
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

    fn write_power(
        &mut self,
        at: CombatId,
        power: V::Power,
        write: impl FnOnce(&mut V::PowerState),
    ) {
        let held = self
            .live
            .powers
            .of_mut(at)
            .find(|held| held.power() == power);
        if let Some(held) = held {
            write(&mut held.state);
        }
    }

    fn give_cards(&mut self, card: V::Card, count: usize) {
        self.ran_on();
        // Every opponent takes its own copies, into its own discard pile. A
        // fight here has one set of piles, so a second player would need the
        // observation to grow one.
        if self.opponents().len() > 1 {
            unimplemented!("cards given to more than one player");
        }
        self.add_generated_cards(CardIdent::new(card), Pile::Discard, count);
    }

    fn character(&self) -> V::Character {
        self.live.character
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
        // `IsOverOrEnding`, which a fight already over is.
        if self.stopped() {
            return;
        }
        let ascension = self.ascension;
        shuffle::<V>(self.live, self.rngs.reborrow(), self.relics, ascension);
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
        self.live.orbs.as_ref().map_or(0, |orbs| {
            num_traits::cast(orbs.capacity).expect("an orb capacity fits a length")
        })
    }

    fn orb_passive_at(&mut self, place: usize) {
        if self.over() {
            return;
        }
        let Some(mut orb) = self
            .live
            .orbs
            .as_ref()
            .and_then(|orbs| orbs.queue.get(place).copied())
        else {
            return;
        };
        V::orb_passive(&mut orb, self);
        if let Some(held) = self
            .live
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
        self.live
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
        let at = self.standing(card);
        self.live
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
        self.ran_on();
        let at = self.standing(card);
        let ident = self.live.cards.remove(card.pile(), at).face.ident;
        self.live.removed_from_deck.push(ident);
        ident
    }

    fn modify_card(
        &mut self,
        card: CardAt<V>,
        change: sts2_core::Change<V>,
        lasting: sts2_core::Lasting,
    ) {
        let at = self.standing(card);
        self.live.cards.write(card.pile(), at, |held| {
            held.modify(change, lasting);
            true
        });
    }

    fn act_on(&mut self, act: sts2_core::PileAct, cards: &[CardAt<V>]) {
        self.ran_on();
        let (sly, ended) =
            super::turn::act_on::<V>(self.live, self.rngs.reborrow(), self.relics, act, cards);
        self.ended = self.ended.or(ended);
        if !self.stopped() {
            self.ended = super::turn::play_sly::<V>(
                self.live,
                self.rngs.reborrow(),
                self.relics,
                sly,
                self.pausable,
            );
        }
    }

    fn modify_this(&mut self, change: sts2_core::Change<V>, lasting: sts2_core::Lasting) {
        self.ran_on();
        let playing = self
            .playing
            .unwrap_or_else(|| unimplemented!("a card writing on itself with none being played"));
        self.live.modify_self(playing, change, lasting);
    }

    fn hittable_opponents(&self) -> sts2_core::Opponents {
        Board::hittable_opponents(self).into_iter().collect()
    }

    fn add_this(&mut self, pile: Pile, position: sts2_core::PilePosition) {
        self.ran_on();
        let playing = self
            .playing
            .unwrap_or_else(|| unimplemented!("a card moving itself with none being played"));
        if !self.live.cards.in_play(playing) {
            unimplemented!("a card moving itself from outside the play pile");
        }
        if pile == Pile::Hand {
            unimplemented!("a card putting itself into the hand");
        }
        self.live.cards.leave_play(playing, Some(pile), position);
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

    fn discard_this(&mut self) {
        self.ran_on();
        let playing = self.playing.expect("a card behavior has a card");
        let Some(at) = self.live.cards.find(Pile::Hand, playing) else {
            return;
        };
        let card = self
            .live
            .cards
            .get(Pile::Hand, at)
            .expect("a card stands where it was found")
            .face
            .ident
            .card;
        // `CardCmd.Discard`.
        self.live.cards.move_to(Pile::Hand, at, Pile::Discard);
        self.live.history.discarded(card);
        let ended = super::turn::card_discarded::<V>(self.live, self.rngs.reborrow(), self.relics);
        self.ended = self.ended.or(ended);
    }

    fn summon(&mut self, monster: V::Monster, position: usize) -> CombatId {
        let id = super::monster::summon(
            self.live,
            self.relics,
            monster,
            position,
            None,
            self.rngs.niche,
        );
        self.arrived(id, monster);
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
        let id = super::monster::summon(
            self.live,
            self.relics,
            monster,
            position,
            Some(slot),
            self.rngs.niche,
        );
        self.arrived(id, monster);
        id
    }
}

/// A selection a listener asks for after a shuffle: from `pile`, `count`
/// cards, and what happens to them.
pub(super) type ShuffleAsks<V> = (
    Pile,
    <V as sts2_core::ContentVersion>::Number,
    sts2_core::PileAct,
);

/// `CardPileCmd.Shuffle`: the discard pile shuffled into the draw pile, then
/// `ModifyShuffleOrder` over the result.
///
/// # Panics
///
/// A listener asking the player something after the shuffle, which only a
/// draw that can pause carries out.
pub(super) fn shuffle<V: Rules>(
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    relics: &mut Belt<V>,
    ascension: V::AscensionLevel,
) {
    if let Some(asks) = shuffle_asking::<V>(live, rngs, relics, ascension) {
        unimplemented!("a shuffle asking for {asks:?} inside a draw that cannot pause");
    }
}

/// [`shuffle`], handing back the selection a listener asks for after it,
/// which is `CardSelectCmd.FromCombatPile` inside `AfterShuffle`.
pub(super) fn shuffle_asking<V: Rules>(
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    relics: &mut Belt<V>,
    ascension: V::AscensionLevel,
) -> Option<ShuffleAsks<V>> {
    live.cards.reshuffle(rngs.shuffle);
    let hook = sts2_core::EventHook::ShuffleOrder { initial: false };
    for (listener, effect) in super::listen::notify::<V>(live, relics, hook) {
        match (listener, effect) {
            // The shuffled list is the discard and draw piles, and a card
            // anywhere else stays put.
            (Listener::Card { id }, HookEffect::ToTopOfDrawPile) => {
                if let Some(at) = live.cards.find(Pile::Draw, id) {
                    live.cards.move_to_top(Pile::Draw, at);
                }
            }
            (listener, effect) => {
                unimplemented!("{listener:?} asking for {effect:?} over a shuffle")
            }
        }
    }
    // `Hook.AfterShuffle`.
    let asked = super::listen::notify::<V>(live, relics, sts2_core::EventHook::Shuffled);
    if asked.is_empty() {
        return None;
    }
    let mut asks = None;
    let mut board = Board::for_card(live, rngs, relics, ascension, None, None);
    for (listener, effect) in asked {
        match effect {
            // The listeners after one that asked wait for the pick, which
            // waits for the draw to pause.
            effect @ HookEffect::AddToDrawAtRandom { .. } if asks.is_some() => {
                board.live.after_pick.push(effect)
            }
            effect if asks.is_some() => {
                unimplemented!("{listener:?} asking for {effect:?} after a shuffle's selection")
            }
            HookEffect::SelectFromPile { pile, count, act } => {
                asks = Some((pile, count, act));
            }
            effect @ HookEffect::AddToDrawAtRandom { .. } => board.after_shuffle(effect),
            effect => unimplemented!("{listener:?} asking for {effect:?} after a shuffle"),
        }
    }
    asks
}

impl<V: Rules> Board<'_, V> {
    /// Stop a command that starts while a question waits, where nothing
    /// holds it for the answer.
    ///
    /// A move returns on an attack that asked, so its command here ran on
    /// past the attack. An effect stands still where its code hands back
    /// [`sts2_core::Played::Waiting`], so its command here needs a step cut.
    /// A draw or an auto-play waits behind the question on its own.
    fn ran_on(&self) {
        if self.live.asking.is_none() {
            return;
        }
        if self.moving && !self.pausable {
            panic!("a move ran on past its attack while the player's answer waits");
        }
        if !self.moving && self.pausable && self.landed.is_none() {
            let card = self
                .playing
                .and_then(|id| self.live.cards.named(id))
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
        let asked = super::listen::notify::<V>(
            self.live,
            self.relics,
            sts2_core::EventHook::BeforeAttack {
                command,
                attacker,
                source: self.playing.map(|id| {
                    let card = self
                        .live
                        .cards
                        .named(id)
                        .expect("the card making an attack is in the fight");
                    (id, card.face.ident.card)
                }),
                powered: scaling == Scaling::Dynamic,
            },
        );
        if let Some((listener, effect)) = asked.first() {
            unimplemented!("{listener:?} asking for {effect:?} before an attack")
        }
        // A command inside another's hits keeps its own results.
        let outer = self.landed.replace(sts2_core::AttackResults::new());
        hits(self);
        let results = std::mem::replace(&mut self.landed, outer).unwrap_or_default();
        let attack = super::live::HeldAttack {
            command,
            attacker,
            monster,
            powered: scaling == Scaling::Dynamic,
        };
        if !self.held.is_empty() {
            self.live.defer(super::live::Deferred::Hits {
                frame: self.frame(),
                attack,
                hits: std::mem::take(&mut self.held),
                landed: results.clone(),
            });
            return results;
        }
        self.attack_over(attack, results)
    }

    /// Whether a hit resolved now waits: inside an attack whose earlier hit
    /// raised a question, on a board that can stand still for it.
    fn hits_wait(&self) -> bool {
        self.live.asking.is_some() && self.pausable && self.landed.is_some()
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
        } = attack;
        // `History.CreatureAttacked`, before `AfterAttack` and whether or not
        // the fight is ending.
        self.live.history.attacked(monster);
        // `IterateCombatHookListeners` hands a fight that is ending no
        // listeners.
        if self.over() {
            return results;
        }
        let asked = super::listen::notify::<V>(
            self.live,
            self.relics,
            sts2_core::EventHook::AfterAttack {
                command,
                attacker,
                monster,
                powered,
            },
        );
        for (listener, effect) in asked {
            match (listener, effect) {
                (Listener::Power { owner, power, .. }, HookEffect::ModifyAmount(offset)) => {
                    self.modify_amount(owner, power, offset)
                }
                // Written on the card that asked, wherever it stands.
                (Listener::Card { id }, HookEffect::ModifySelf { change, lasting }) => {
                    self.live.modify_self(id, change, lasting)
                }
                // `CreatureCmd.GainBlock`, unpowered, on the relic's owner.
                (Listener::Relic { .. }, HookEffect::GainBlock(amount)) => {
                    let player = self.live.player().id;
                    self.live.gain_unpowered_block(player, amount);
                }
                // A fight here has one player, whose piles take them all.
                (Listener::Power { .. }, HookEffect::GiveCardsPerUnblockedHit { card, per }) => {
                    let player = self.live.player().id;
                    let hits = results
                        .iter()
                        .filter(|hit| hit.receiver == player && hit.unblocked > V::Number::zero())
                        .count();
                    let count = num_traits::cast::<_, usize>(per)
                        .unwrap_or_else(|| unimplemented!("{per:?} cards for each hit"))
                        * hits;
                    self.add_generated_cards(CardIdent::new(card), Pile::Discard, count);
                }
                (listener, effect) => {
                    unimplemented!("{listener:?} asking for {effect:?} after an attack")
                }
            }
        }
        results
    }

    /// What an `AfterShuffle` listener asked for, carried out.
    fn after_shuffle(&mut self, effect: HookEffect<V>) {
        match effect {
            HookEffect::AddToDrawAtRandom { card, count } => {
                let count: usize = num_traits::cast(count)
                    .unwrap_or_else(|| unimplemented!("{count:?} cards into the draw pile"));
                for _ in 0..count {
                    self.add_to_draw_at_random(card);
                }
            }
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
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    relics: &mut Belt<V>,
    ascension: V::AscensionLevel,
    count: usize,
    hand_draw: bool,
) -> Option<super::Ended> {
    match draw_pausing::<V>(live, rngs, relics, ascension, count, hand_draw, false) {
        Drawn::Done(ended) => ended,
        Drawn::Paused { asks, .. } => {
            unimplemented!("a shuffle asking for {asks:?} inside a draw that cannot pause")
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
    moving: bool,
}

impl<'a, V: Rules> Board<'a, V> {
    /// Take `id`'s move graph out of the fight. The model of the monster
    /// whose move this is stays on the board until the move is over.
    fn leave_machine(&mut self, id: CombatId) {
        if self.moving && id == self.actor {
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
            moving: self.moving,
        }
    }

    /// A board as `frame` was, inside the attack whose hits it held.
    fn in_frame(
        live: &'a mut Live<V>,
        rngs: CombatRngs<'a>,
        relics: &'a mut Belt<V>,
        ascension: V::AscensionLevel,
        frame: Frame<V>,
    ) -> Self {
        let Frame {
            actor,
            target,
            playing,
            intent_damage,
            intent_repeats,
            moving,
        } = frame;
        Board {
            live,
            rngs,
            relics,
            actor,
            target,
            playing,
            intent_damage,
            intent_repeats,
            ascension,
            skipped: Vec::new(),
            ended: None,
            pausable: true,
            moving,
            held: Vec::new(),
            left: None,
            landed: None,
        }
    }
}

/// Land the hits an attack held while a question waited, each resolved now,
/// then end the attack. A question one of them raises holds the rest again.
pub(super) fn land_held<V: Rules>(
    live: &mut Live<V>,
    rngs: CombatRngs<'_>,
    relics: &mut Belt<V>,
    ascension: V::AscensionLevel,
    held: super::live::Deferred<V>,
) -> Option<super::Ended> {
    let super::live::Deferred::Hits {
        frame,
        attack,
        hits,
        landed,
    } = held
    else {
        unreachable!("only held hits land as them")
    };
    let mut board = Board::in_frame(live, rngs, relics, ascension, frame);
    board.landed = Some(landed);
    for effect in hits {
        let hit = Fight::resolve(&board, effect);
        Fight::apply(&mut board, hit);
    }
    let results = board.landed.take().unwrap_or_default();
    if board.held.is_empty() {
        board.attack_over(attack, results);
    } else {
        let hits = std::mem::take(&mut board.held);
        board.live.defer(super::live::Deferred::Hits {
            frame,
            attack,
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
/// [`Live::deferred`] with the effect behind it.
pub(super) fn draw_in_effect<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut Belt<V>,
    ascension: V::AscensionLevel,
    count: usize,
    hand_draw: bool,
    resumed: bool,
) -> Option<super::Ended> {
    let (mut count, mut resumed) = (count, resumed);
    loop {
        match draw_pausing::<V>(
            live,
            rngs.reborrow(),
            relics,
            ascension,
            count,
            hand_draw,
            resumed,
        ) {
            Drawn::Done(ended) => return ended,
            Drawn::Paused {
                left,
                asks: (pile, picks, act),
            } => {
                match super::turn::select::<V>(live, rngs.reborrow(), relics, pile, picks, act) {
                    Ok(Some(ended)) => return Some(ended),
                    Ok(None) => {}
                    Err(question) => {
                        live.asking = Some(super::live::Asking {
                            then: super::live::Then::Pick { act },
                            question,
                        });
                        live.defer(super::live::Deferred::Draw {
                            count: left,
                            hand_draw,
                            resumed: true,
                        });
                        return None;
                    }
                }
                (count, resumed) = (left, true);
            }
        }
    }
}

/// How a draw that can pause stopped.
pub(super) enum Drawn<V: Rules> {
    /// As many cards as there were to draw, and how the fight ended if it did.
    Done(Option<super::Ended>),
    /// A shuffle inside it asked for a selection, with `left` cards still to
    /// draw once it is answered.
    Paused { left: usize, asks: ShuffleAsks<V> },
}

/// [`draw`], stopping where a shuffle inside it asks the player something.
/// `resumed` is a draw that carries on from such a stop, which has asked
/// `ShouldDraw` already.
pub(super) fn draw_pausing<V: Rules>(
    live: &mut Live<V>,
    mut rngs: CombatRngs<'_>,
    relics: &mut Belt<V>,
    ascension: V::AscensionLevel,
    count: usize,
    hand_draw: bool,
    resumed: bool,
) -> Drawn<V> {
    // `Hook.ShouldDraw`, once for the whole draw, and `AfterPreventingDraw`
    // when a listener stops it.
    let player = live.player().id;
    let hook = sts2_core::DecisionHook::ShouldDraw {
        player,
        hand_draw,
        side: live.side,
    };
    // The `AfterShuffle` listeners that waited for the pick this resumes
    // from.
    if resumed && !live.after_pick.is_empty() {
        let waiting = std::mem::take(&mut live.after_pick);
        let mut board = Board::for_card(live, rngs.reborrow(), relics, ascension, None, None);
        for effect in waiting {
            board.after_shuffle(effect);
        }
    }
    if !resumed && super::listen::any_decides::<V>(Walk { live, relics }, hook, false) {
        let asked = super::listen::notify::<V>(live, relics, sts2_core::EventHook::DrawPrevented);
        if let Some((listener, effect)) = asked.first() {
            unimplemented!("{listener:?} asking for {effect:?} after preventing a draw");
        }
        return Drawn::Done(None);
    }
    let mut ended = None;
    for drawing in 0..count {
        // `IsOverOrEnding`, and `CardPile.MaxCardsInHand`: a full hand stops
        // the draw.
        if ended.is_some() || live.cards.len(Pile::Hand) >= super::pile::MAX_CARDS_IN_HAND {
            return Drawn::Done(ended);
        }
        if live.cards.is_empty(Pile::Draw) {
            if live.cards.is_empty(Pile::Discard) {
                return Drawn::Done(ended);
            }
            if let Some(asks) = shuffle_asking::<V>(live, rngs.reborrow(), relics, ascension) {
                return Drawn::Paused {
                    left: count - drawing,
                    asks,
                };
            }
        }
        let drawn = live
            .cards
            .draw()
            .expect("a draw pile just checked for cards");
        // `History.CardDrawn`, before the hook, so a listener counts this
        // draw among the turn's.
        live.history.draw(drawn.face.ident.card, hand_draw);
        // `Hook.AfterCardDrawn` makes its early pass over every listener first.
        let hook = sts2_core::EventHook::CardDrawnEarly {
            card: drawn.face.ident.card,
            id: drawn.id,
            hand_draw,
        };
        for (listener, effect) in super::listen::notify::<V>(live, relics, hook) {
            match effect {
                // `IsOverOrEnding` stops an auto-play before it starts.
                HookEffect::AutoPlay(card) => {
                    if ended.is_none() {
                        ended = super::play::auto_play::<V>(
                            live,
                            rngs.reborrow(),
                            relics,
                            card,
                            Pile::Hand,
                            None,
                            false,
                        );
                    }
                }
                effect => unimplemented!("{listener:?} asking for {effect:?} as a card is drawn"),
            }
        }
        // `Hook.AfterCardDrawn`, raised for each card inside the loop rather
        // than once for the whole draw. So a card that spends the pool as it
        // arrives has spent it before the next card is taken.
        let asked = super::listen::notify::<V>(
            live,
            relics,
            sts2_core::EventHook::CardDrawn {
                card: drawn.face.ident.card,
                id: drawn.id,
                ethereal: drawn.face.ethereal(),
                hand_draw,
                side: live.side,
            },
        );
        if asked.is_empty() {
            continue;
        }
        let mut rest: SmallVec<[HookEffect<V>; 4]> = SmallVec::new();
        for (listener, effect) in asked {
            match (listener, effect) {
                // Written on the card that asked, wherever it stands.
                (Listener::Card { id }, HookEffect::ModifySelf { change, lasting }) => {
                    live.modify_self(id, change, lasting)
                }
                // Only while the card is still in hand, which the
                // enchantment checks before it draws.
                (Listener::Card { id }, HookEffect::RandomSelfCost { below }) => {
                    if live.cards.find(Pile::Hand, id).is_some() {
                        let cost = random_cost::<V>(&mut rngs, below);
                        live.modify_self(
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
                        card,
                        sts2_core::Change::SetCost(sts2_core::CostPool::Energy, cost),
                        sts2_core::Lasting::ThisFight,
                    );
                }
                (_, effect) => rest.push(effect),
            }
        }
        let mut board = Board::for_card(
            live,
            rngs.reborrow(),
            relics,
            ascension,
            None,
            Some(drawn.id),
        );
        super::hook::apply::<V, _>(&mut board, rest);
        ended = ended.or(board.ended());
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
            ),
        );
        resolve::<V>(
            Walk {
                live: &fight,
                relics: &Belt::new(),
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
