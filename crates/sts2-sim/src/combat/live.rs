//! The fight the simulator owns.
//!
//! [`sts2_core::Combat`] is a snapshot: what a fight looks like from outside.
//! This is the fight itself. Rules mutate it, nothing clones it to hand out,
//! and it carries no report.
//!
//! What it holds beyond the snapshot is the move graphs and the skipped ticks.
//! An observation says what a monster does next, not how its graph got there.
//!
//! What it does not hold is [`sts2_core::Playable`]. Every card in hand with
//! its previews is derived from the fields below, so [`Live::report`] builds
//! it and nothing else does.

use num_traits::CheckedAdd;
use sts2_core::{
    CardId, CardIdent, CardNumber, CardTable, CardVars, Combat, CombatId, Creature, Orbs, Rules,
    SideKinds,
};

use super::monster::Machine;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Asker<V: Rules> {
    Card(CardInstance<V>),
    Potion(V::Potion),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Resuming<V: Rules> {
    pub(super) asker: Asker<V>,
    pub(super) target: Option<CombatId>,
    pub(super) won: bool,
    /// Where this effect is in `GeneratePlayCount`'s loop.
    pub(super) plays: Plays<V>,
}

/// `GeneratePlayCount`'s count, and how much of its loop has run.
///
/// The count is worked out once, before the first play. A replay the effect
/// itself hands out does not lengthen the loop it is inside. A potion has no
/// count, and runs once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Plays<V: Rules> {
    pub(super) done: V::Number,
    pub(super) count: V::Number,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Asked<V: Rules> {
    Pile {
        pile: sts2_core::Pile,
        selection: sts2_core::Selection,
    },
    Offered {
        offered: [CardIdent<V>; sts2_core::CHOICES],
        skippable: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Asking<V: Rules> {
    pub(super) effect: Resuming<V>,
    pub(super) at: V::Resume,
    pub(super) question: Asked<V>,
}

/// Mints the ids a fight names its cards by.
///
/// One counter, and nothing reuses a number it gave out. A fight that runs out
/// stops the run rather than name two cards the same.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CardIds<V: Rules>(V::CardSerial);

impl<V: Rules> CardIds<V> {
    pub(super) fn new() -> Self {
        Self(num_traits::zero())
    }

    pub(super) fn mint(&mut self) -> CardId<V> {
        let next = self.0;
        self.0 = next
            .checked_add(&num_traits::one())
            .unwrap_or_else(|| unimplemented!("a fight holding more cards than it can name"));
        CardId::new(next)
    }
}

/// `LocalCostType`: what a modifier does to the cost so far.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CostChange<N> {
    Relative(N),
    Absolute(N),
}

/// One entry of a card's cost modifier list.
///
/// `LocalCostModifier`, minus `reduceOnly`: no ported effect passes it, and an
/// effect that does adds it here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CostModifier<V: Rules> {
    pub(super) pool: sts2_core::CostPool,
    pub(super) change: CostChange<V::Number>,
    pub(super) lasting: sts2_core::Lasting,
}

/// A pile holds copies rather than ids, because a fight writes on a card. Two
/// copies of one card are two objects. A rule that raises the damage on one
/// leaves the other where it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CardInstance<V: Rules> {
    /// Which copy this is, for as long as the fight holds it.
    pub(super) id: CardId<V>,
    pub(super) ident: CardIdent<V>,
    /// The numbers this copy prints, which start as the table's and move.
    pub(super) vars: Vec<CardNumber<V::CardVar, V::Number>>,
    /// `CardModel.EnergyCost` and `StarCost` for this copy.
    ///
    /// Per copy, because an upgrade writes on them. The table prices a fresh
    /// one, and `UpgradeBy` moves the copy alone.
    pub(super) cost: sts2_core::Cost,
    pub(super) star_cost: sts2_core::Cost,
    /// `CardEnergyCost._localModifiers`, and the star pool's beside it.
    ///
    /// Held rather than folded into the printed cost, because each wears off
    /// on its own event and the rest keep applying. Applied in the order they
    /// were written, which is what decides what a stack of them comes to.
    ///
    /// Empty for almost every card, and an empty `Vec` allocates nothing.
    pub(super) cost_modifiers: Vec<CostModifier<V>>,
    /// `CardModel.AddKeyword`, which writes words the table did not print.
    ///
    /// Beside the printed set rather than folded into it. The table's answer
    /// is the card's, and this is the copy's. Only the copy is written on.
    pub(super) added_keywords: Vec<V::CardKeyword>,
    /// `CardModel.BaseReplayCount`, the extra times this copy's play runs.
    ///
    /// Zero for every card the table prints. Only a power hands these out, and
    /// what it writes stays on the copy for the rest of the fight.
    pub(super) replays: V::Number,
}

impl<V: Rules> CardInstance<V> {
    /// A fresh copy, printing what the table prints.
    pub(super) fn new(ident: CardIdent<V>, id: CardId<V>) -> Self {
        let vars = ident
            .card
            .vars()
            .iter()
            .map(|var| CardNumber {
                id: var.id,
                // A build can store exact fractions where this slice only
                // evaluates whole-number effects.
                base: num_traits::NumCast::from(var.base).unwrap_or_else(|| {
                    unimplemented!("the fractional {} this card prints", var.id.name())
                }),
                calculated: var.calculated,
                modifier: var.modifier,
            })
            .collect();
        let mut card = Self {
            id,
            ident,
            vars,
            cost: ident.card.cost(),
            star_cost: ident.card.star_cost(),
            cost_modifiers: Vec::new(),
            added_keywords: Vec::new(),
            replays: num_traits::zero(),
        };
        if card.ident.upgrade != 0 {
            assert_eq!(card.ident.upgrade, 1, "only first upgrades are ported");
            // The level is already on the identity, so this writes what the
            // upgrade writes without counting it again.
            for &change in V::card_upgrade(card.ident.card) {
                card.modify(change, sts2_core::Lasting::ThisFight);
            }
        }
        card
    }

    /// `EnergyCost.GetWithModifiers` and its star twin, as the pair a play
    /// reads.
    ///
    /// The modifiers fold over the printed cost in the order they were
    /// written. `GetWithModifiers` returns the base untouched for a cost that
    /// is not a number, so a pool the card is not priced in folds nothing.
    pub(super) fn costs(&self) -> (sts2_core::Cost, sts2_core::Cost) {
        let fold = |pool, printed| {
            let sts2_core::Cost::Fixed(printed) = printed else {
                return printed;
            };
            let printed: V::Number =
                num_traits::NumCast::from(printed.get()).expect("a fixed cost fits a game number");
            let folded = self
                .cost_modifiers
                .iter()
                .filter(|held| held.pool == pool)
                .fold(printed, |cost, held| match held.change {
                    CostChange::Relative(amount) => cost + amount,
                    CostChange::Absolute(amount) => amount,
                });
            // `Math.Max(0, num)`, which every read clamps at.
            let largest: V::Number =
                num_traits::NumCast::from(u8::MAX - 1).expect("a fixed cost fits a game number");
            let folded = folded.clamp(num_traits::zero(), largest);
            let folded = num_traits::NumCast::from(folded)
                .expect("a clamped fixed cost fits its representation");
            sts2_core::Cost::fixed(folded)
        };
        (
            fold(sts2_core::CostPool::Energy, self.cost),
            fold(sts2_core::CostPool::Star, self.star_cost),
        )
    }

    /// Write one change on this copy.
    ///
    /// A number and a replay count go into what the copy prints, because
    /// `DynamicVars.BaseValue` and `BaseReplayCount` are written that way and
    /// neither wears off. A cost is kept beside the printed one, because the
    /// game keeps a list of those and folds it on every read.
    pub(super) fn modify(&mut self, change: sts2_core::Change<V>, lasting: sts2_core::Lasting) {
        match change {
            sts2_core::Change::Upgrade => self.upgrade(),
            sts2_core::Change::Raise(key, amount) => {
                let printed = self
                    .vars
                    .iter_mut()
                    .find(|var| var.id == key)
                    .unwrap_or_else(|| {
                        unimplemented!("the {} this card does not print", key.name())
                    });
                printed.base = printed.base + amount;
            }
            sts2_core::Change::ChangeCost(pool, by) => {
                self.modify_cost(pool, CostChange::Relative(by), lasting)
            }
            sts2_core::Change::SetCost(pool, to) => {
                self.modify_cost(pool, CostChange::Absolute(to), lasting)
            }
            // `AddKeyword`, which is a set: a word already on the copy is not
            // added twice.
            sts2_core::Change::AddKeyword(keyword) => {
                if !self.added_keywords.contains(&keyword) {
                    self.added_keywords.push(keyword);
                }
            }
        }
    }

    /// `Keywords.Contains(Innate)` for this copy.
    ///
    /// The union of what the table prints and what has been written on. Asked
    /// of Innate alone, because that is the one keyword something in a fight
    /// both writes and reads. The rest are read off the table.
    pub(super) fn innate(&self) -> bool {
        use sts2_core::{CardTable as _, KeywordKinds};

        self.ident.card.innate()
            || self
                .added_keywords
                .contains(&<V::CardKeyword as KeywordKinds<V>>::INNATE)
    }

    /// Append one cost modifier, which is what every `Set` and `Add` on
    /// `CardEnergyCost` does.
    ///
    /// A pool the card is not priced in takes none. The setters guard on
    /// `Canonical >= 0`, and `GetWithModifiers` returns early on the same
    /// test, so nothing reads a modifier there.
    fn modify_cost(
        &mut self,
        pool: sts2_core::CostPool,
        change: CostChange<V::Number>,
        lasting: sts2_core::Lasting,
    ) {
        let printed = match pool {
            sts2_core::CostPool::Energy => self.cost,
            sts2_core::CostPool::Star => self.star_cost,
        };
        if printed == sts2_core::Cost::None {
            return;
        }
        self.cost_modifiers.push(CostModifier {
            pool,
            change,
            lasting,
        });
    }

    /// Drop the cost modifiers that do not outlive this turn.
    pub(super) fn turn_ended(&mut self) {
        self.cost_modifiers
            .retain(|held| !held.lasting.clears_when_turn_ends());
    }

    /// `GeneratePlayCount`: one play, and one more per replay this copy holds.
    ///
    /// `Hook.ModifyCardPlayCount` and an enchantment's `EnchantPlayCount` both
    /// move this in the game. Neither is ported, and each is a hook this reads
    /// rather than a number to store.
    pub(super) fn play_count(&self) -> V::Number {
        self.replays + num_traits::one()
    }

    /// Drop the cost modifiers that do not outlive being played.
    pub(super) fn was_played(&mut self) {
        self.cost_modifiers
            .retain(|held| !held.lasting.clears_when_played());
    }

    fn upgrade(&mut self) {
        // `CardModel.IsUpgradable`, which a card already at its own maximum is
        // not. A status card prints no upgrade at all.
        if self.ident.upgrade >= self.ident.card.max_upgrade() {
            return;
        }
        for &change in V::card_upgrade(self.ident.card) {
            self.modify(change, sts2_core::Lasting::ThisFight);
        }
        self.ident.upgrade += 1;
    }
}

impl<V: Rules> sts2_core::CreatureView<V> for Fighter<V> {
    fn id(&self) -> CombatId {
        self.id
    }

    fn side(&self) -> V::CombatSide {
        self.side
    }

    fn alive(&self) -> bool {
        self.alive
    }

    fn hp(&self) -> V::Health {
        self.hp
    }

    fn max_hp(&self) -> V::Health {
        self.max_hp
    }

    fn power(&self, power: V::Power) -> Option<sts2_core::PowerAmount<V::Number>> {
        self.powers
            .iter()
            .find(|held| held.power == power)
            .map(|held| held.amount)
    }
}

/// One power on a creature, with the fields its own model saves.
///
/// [`sts2_core::Stack`] is what a report shows of a power: which one, and the
/// amount. This is what the fight carries, which is that pair and the model
/// beside it. See [`Fighter`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeldPower<V: Rules> {
    pub(crate) power: V::Power,
    pub(crate) amount: sts2_core::PowerAmount<V::Number>,
    pub(crate) state: V::PowerState,
}

impl<V: Rules> HeldPower<V> {
    /// A power as `PowerCmd.Apply` lands one, its model as new as it is.
    pub(crate) fn new(power: V::Power, amount: sts2_core::PowerAmount<V::Number>) -> Self {
        Self {
            power,
            amount,
            state: <V::PowerState as sts2_core::PowerModel<V>>::new(power),
        }
    }

    /// This power as a hook's listener, which is the instance the game hands
    /// the hook to.
    ///
    /// `owner` is the side of the creature carrying it. A power is held by a
    /// creature and does not know which, so the caller that found it says.
    pub(crate) fn listener(&self, owner: V::CombatSide) -> sts2_core::Listener<'_, V> {
        sts2_core::Listener::Power {
            power: self.power,
            amount: self.amount,
            owner,
            state: &self.state,
        }
    }

    pub(crate) fn event_listener(
        &mut self,
        owner: V::CombatSide,
    ) -> sts2_core::EventListener<'_, V> {
        sts2_core::EventListener::Power {
            power: self.power,
            amount: self.amount,
            owner,
            state: &mut self.state,
        }
    }

    /// What a report shows of it.
    pub(crate) fn stack(&self) -> sts2_core::Stack<V> {
        sts2_core::Stack {
            power: self.power,
            amount: self.amount,
        }
    }
}

/// One creature in the fight, and what its powers save.
///
/// [`sts2_core::Creature`] is what a report shows. This is what the fight
/// holds: the same facts, and a model per power rather than a bare stack.
#[derive(Debug, Clone)]
pub(crate) struct Fighter<V: Rules> {
    pub(crate) id: CombatId,
    pub(crate) model: Option<sts2_core::ModelId<V>>,
    pub(crate) side: V::CombatSide,
    pub(crate) hp: V::Health,
    pub(crate) max_hp: V::Health,
    pub(crate) block: V::Number,
    pub(crate) alive: bool,
    pub(crate) powers: Vec<HeldPower<V>>,
    pub(crate) next_move: Option<sts2_core::NextMove<V>>,
}

impl<V: Rules> Fighter<V> {
    /// What a report shows of it, which is everything but the models.
    pub(crate) fn creature(&self) -> Creature<V> {
        Creature {
            id: self.id,
            model: self.model,
            side: self.side,
            hp: self.hp,
            max_hp: self.max_hp,
            block: self.block,
            alive: self.alive,
            powers: self.powers.iter().map(HeldPower::stack).collect(),
            next_move: self.next_move.clone(),
        }
    }
}

/// A fight in progress.
#[derive(Debug, Clone)]
pub(crate) struct Live<V: Rules> {
    pub(super) character: V::Character,
    /// What the run is being played at, which a card or a move reads for the
    /// numbers the level moves.
    pub(super) ascension: V::AscensionLevel,
    pub(super) turn: V::Count,
    pub(super) phase: V::PlayerTurnPhase,
    pub(super) energy: V::Count,
    pub(super) stars: V::Count,
    pub(super) round: V::Count,
    /// The encounter this active fight belongs to.
    pub(super) encounter: V::Encounter,
    pub(super) hand: Vec<CardInstance<V>>,
    pub(super) draw: Vec<CardInstance<V>>,
    pub(super) discard: Vec<CardInstance<V>>,
    pub(super) exhaust: Vec<CardInstance<V>>,
    /// Deck cards a combat command removed permanently while this fight ran.
    pub(super) removed_from_deck: Vec<CardIdent<V>>,
    pub(super) creatures: Vec<Fighter<V>>,
    pub(super) orbs: Option<Orbs<V>>,
    /// One per monster in the fight, in the order the encounter fields them.
    pub(super) machines: Vec<Machine<V>>,
    /// `PowerModel.SkipNextDurationTick`, as the powers holding it.
    ///
    /// A debuff landing on a player-side creature does not count down at the
    /// end of the turn it landed in. The game keeps that on the power
    /// instance. The observation's [`sts2_core::Stack`] is the game's own wire
    /// shape and carries no such field, so the fight keeps it beside them.
    pub(super) skip_tick: Vec<(CombatId, V::Power)>,
    /// The ids this fight has given out, and the next one it will.
    pub(super) card_ids: CardIds<V>,
    /// A card play waiting on an answer, which is the only thing the fight
    /// will take while it is set.
    pub(super) asking: Option<Asking<V>>,
}

impl<V: Rules> Live<V> {
    pub(crate) fn removed_from_deck(&self) -> &[CardIdent<V>] {
        &self.removed_from_deck
    }

    /// The card-selection question on which this fight is parked.
    pub(crate) fn question(&self) -> Option<sts2_core::Question<V>> {
        let asked = self.asking.as_ref()?;
        let (min, max, options) = match &asked.question {
            Asked::Pile { pile, selection } => {
                let (min, max) = match *selection {
                    sts2_core::Selection::Exactly(count) => {
                        let count =
                            u32::try_from(count.get()).expect("a selection count fits the wire");
                        (count, count)
                    }
                    sts2_core::Selection::UpTo(count) => (
                        0,
                        u32::try_from(count.get()).expect("a selection count fits the wire"),
                    ),
                };
                let options = super::play::pile(self, *pile)
                    .iter()
                    .map(|card| card.ident)
                    .collect();
                (min, max, options)
            }
            Asked::Offered { offered, skippable } => (u32::from(!skippable), 1, offered.to_vec()),
        };
        Some(sts2_core::Question::SelectCards {
            min,
            max,
            // The content id does not carry display text. The selector has no
            // prompt either, so this stays absent as it does for other
            // unattributed selections.
            asker: None,
            options: options
                .into_iter()
                .enumerate()
                .map(|(index, card)| sts2_core::CardOption {
                    index,
                    card,
                    text: None,
                })
                .collect(),
        })
    }

    /// The player, which every fight has one of.
    pub(crate) fn player(&self) -> &Fighter<V> {
        self.creatures
            .iter()
            .find(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER)
            .expect("a fight has the player in it")
    }

    /// This fight as a snapshot, and the only way out of here.
    ///
    /// The one place a `Combat` is made. The hand is held to the rules here
    /// and nowhere else, so a decision nobody observes costs none of that.
    ///
    /// `Attack::damage` comes out as the number that will land, not the one
    /// the move prints. The game works it out on every read.
    pub(crate) fn report(&self) -> Combat<V> {
        let mut creatures: Vec<_> = self.creatures.iter().map(Fighter::creature).collect();
        let player = self
            .creatures
            .iter()
            .find(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER)
            .map(|creature| creature.id);
        for creature in &mut creatures {
            let (Some(player), Some(next)) = (player, &mut creature.next_move) else {
                continue;
            };
            for intent in &mut next.intents {
                if let Some(attack) = &mut intent.attack {
                    attack.damage = super::board::modified::<V>(
                        &self.creatures,
                        creature.id,
                        Some(player),
                        attack.damage,
                    );
                }
            }
        }
        Combat {
            turn: self.turn,
            phase: self.phase,
            energy: self.energy,
            stars: self.stars,
            round: self.round,
            encounter: Some(self.encounter),
            hand: self.hand.iter().map(|card| card.ident).collect(),
            draw: self.draw.iter().map(|card| card.ident).collect(),
            discard: self.discard.iter().map(|card| card.ident).collect(),
            exhaust: self.exhaust.iter().map(|card| card.ident).collect(),
            creatures,
            legal: super::legal::hand::<V>(&self.hand, &self.creatures, self.energy, self.stars),
            orbs: self.orbs.clone(),
        }
    }

    /// Consume a won fight into the report that remains on its reward screen.
    pub(crate) fn finish(mut self) -> Combat<V> {
        self.phase = <V::PlayerTurnPhase as sts2_core::TurnPhaseKinds>::NONE;
        self.hand.clear();
        self.draw.clear();
        self.discard.clear();
        self.exhaust.clear();
        let player = self
            .creatures
            .iter_mut()
            .find(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER)
            .expect("a fight has the player in it");
        player.powers.clear();
        player.block = num_traits::zero();
        self.report()
    }
}
