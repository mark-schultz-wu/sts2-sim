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

use bitfield_struct::bitfield;
use nonmax::{NonMaxI16, NonMaxU8};
use num_traits::CheckedAdd;
use sts2_core::{
    CardId, CardIdent, CardNumber, CardTable, CardVars, Combat, CombatId, Creature, Orbs, Pile,
    Rules, SideKinds,
};

use super::monster::Machine;
use super::powers::{PowerStore, Powers as _};
use crate::bounded::Bounded;

/// The powers that skip their next tick, by the creature holding each.
pub(super) type Skips<V> = Bounded<(CombatId, <V as sts2_core::ContentVersion>::Power), SKIPS>;

/// How a copy holds one of the numbers it prints.
///
/// Narrower than the build's own game number, which is an `int`. Both ported
/// builds print inside `-1..=600`, and a fight writes on top of that:
/// `ForgeCmd` raises `SovereignBlade`'s damage each time a smith is played.
/// So the width outlasts a run rather than fitting a table.
pub(super) type Amount = i16;

/// What one copy of a card prints, as the difference from its own table row.
///
/// A fresh copy is zeroes, whatever the card prints. An upgrade moves one and
/// `ForgeCmd` moves one again.
///
/// `None` for a card printing more than [`VARS`] numbers. Nothing raises one
/// of those, and [`CardInstance::raise`] says so if something tries.
pub(super) type CardVarList = Option<[NonMaxI16; VARS]>;

/// The powers one creature holds.
pub(crate) type PowerList<V> = Bounded<HeldPower<V>, POWERS>;

/// How many powers one creature holds.
const POWERS: usize = 16;

/// How many creatures a fight holds, summons included.
pub(crate) const CREATURES: usize = 8;

/// How many powers a fight holds that skip their next tick.
const SKIPS: usize = 32;

/// How many differences one copy of a card holds.
///
/// Five, which covers 577 of the 578 cards this build prints. The one that
/// prints more is `Event(MadScience)`, at eleven, and no event is ported.
/// `only_one_card_prints_more_than_a_copy_holds` is what keeps that true.
const VARS: usize = 5;

/// How many local price changes one copy of a card carries.
///
/// Four, from how they arrive rather than from a guess. A card collects one
/// from its own upgrade, and two from a power potion, which sets both pools at
/// once. So three is what the ported content reaches. A second potion
/// generates a second card, which starts empty.
const MODIFIERS: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Asker<V: Rules> {
    /// The card being played, which stands in the play pile.
    Card(CardPlay<V>),
    Potion(V::Potion),
}

/// A card's play: which copy, what it cost, and where it goes once the play
/// is over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CardPlay<V: Rules> {
    pub(super) id: CardId<V>,
    /// `CardPlay.Resources.EnergyValue`.
    pub(super) energy: V::Count,
    /// `ModifyCardPlayResultPileTypeAndPosition`'s answer, worked out before
    /// the first play. Nothing for a card that leaves the fight.
    pub(super) pile: Option<sts2_core::Pile>,
    pub(super) position: sts2_core::PilePosition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Resuming<V: Rules> {
    pub(super) asker: Asker<V>,
    pub(super) target: Option<CombatId>,
    pub(super) ended: Option<super::Ended>,
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

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Asking<V: Rules> {
    pub(super) then: Then<V>,
    pub(super) question: Asked<V>,
}

/// What an answer resumes.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Then<V: Rules> {
    /// A card's or a potion's own effect, which `V::resume_play` picks up.
    Play { effect: Resuming<V>, at: V::Resume },
    /// `SetupPlayerTurn`, which a hook parked on its selection. `act` is what
    /// happens to the cards picked, and `stage` is the rest of the setup.
    Setup {
        act: sts2_core::PileAct,
        stage: Setup<V>,
    },
}

/// Where `SetupPlayerTurn` stands, with what the hook it stopped in has still
/// to carry out.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Setup<V: Rules> {
    /// Inside `BeforeHandDraw`, before the hand is drawn.
    BeforeHandDraw(super::listen::Answers<V>),
    /// Inside `AfterPlayerTurnStart`, the last of the setup.
    PlayerTurnStart(super::listen::Answers<V>),
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

/// One entry of a card's cost modifier list, in a byte.
///
/// `LocalCostModifier`, minus `reduceOnly`: no ported effect passes it.
///
/// Eight bits say all of it: which pool, which expiration, whether it sets the
/// cost or shifts it, and by how much.
#[bitfield(u8, new = false, default = false, debug = false, order = Lsb)]
#[derive(PartialEq, Eq)]
pub(super) struct CostModifier {
    /// On the star pool rather than the energy one.
    star: bool,
    /// Sets the cost rather than shifting it.
    absolute: bool,
    /// `LocalCostModifierExpiration`'s two flags, which is what it is.
    turn_end: bool,
    when_played: bool,
    /// How much, as [`CostChange`] reads it.
    ///
    /// Raw, because the same four bits carry a set and a shift. A set is a
    /// price and reads unsigned. A shift is signed and carries
    /// [`SHIFT_BIAS`], because four bits cannot be both.
    #[bits(4)]
    amount: u8,
}

/// What a shift carries so that it shares the bits a set uses.
///
/// A set is a price, which is never negative, so its four bits run `0..=15`. A
/// shift runs `-8..=7` over the same four, and this is the offset between
/// them.
const SHIFT_BIAS: i8 = 8;

/// The most a modifier sets a cost to, which is four bits of price.
pub(super) const SET_AT_MOST: i8 = 15;
/// How far a modifier shifts a cost, which is the same four bits, biased.
pub(super) const SHIFT_WITHIN: std::ops::RangeInclusive<i8> = -SHIFT_BIAS..=SHIFT_BIAS - 1;

impl CostModifier {
    /// One modifier, as the change a rule asked for.
    ///
    /// The one place that knows what a modifier holds. A rule names a game
    /// number, and this is where it has to fit four bits.
    ///
    /// # Panics
    ///
    /// A cost set higher than [`SET_AT_MOST`] or shifted outside
    /// [`SHIFT_WITHIN`].
    fn held<N: std::fmt::Debug + Copy + num_traits::ToPrimitive>(
        pool: sts2_core::CostPool,
        change: CostChange<N>,
        lasting: sts2_core::Lasting,
    ) -> Self {
        let fits = |asked: N, within: std::ops::RangeInclusive<i8>| {
            num_traits::NumCast::from(asked)
                .filter(|amount| within.contains(amount))
                .unwrap_or_else(|| {
                    unimplemented!("a card cost of {asked:?}, and a modifier holds {within:?}")
                })
        };
        let (absolute, amount) = match change {
            CostChange::Relative(shift) => (false, fits(shift, SHIFT_WITHIN) + SHIFT_BIAS),
            CostChange::Absolute(set) => (true, fits(set, 0..=SET_AT_MOST)),
        };
        Self::from_bits(0)
            .with_star(pool == sts2_core::CostPool::Star)
            .with_absolute(absolute)
            .with_turn_end(lasting.clears_when_turn_ends())
            .with_when_played(lasting.clears_when_played())
            .with_amount(amount as u8)
    }

    fn pool(self) -> sts2_core::CostPool {
        match self.star() {
            false => sts2_core::CostPool::Energy,
            true => sts2_core::CostPool::Star,
        }
    }

    fn change(self) -> CostChange<i8> {
        let amount = self.amount() as i8;
        match self.absolute() {
            true => CostChange::Absolute(amount),
            false => CostChange::Relative(amount - SHIFT_BIAS),
        }
    }

    fn clears_when_turn_ends(self) -> bool {
        self.turn_end()
    }

    fn clears_when_played(self) -> bool {
        self.when_played()
    }
}

impl std::fmt::Debug for CostModifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CostModifier")
            .field("pool", &self.pool())
            .field("change", &self.change())
            .field("turn_end", &self.clears_when_turn_ends())
            .field("when_played", &self.clears_when_played())
            .finish()
    }
}

/// A listener of this fight's, which is a hook's audience of one.
///
/// The card a hook reaches is one of this fight's copies, so these name that
/// type once rather than at every call. See [`sts2_core::Printed`].
pub(crate) type Heard<'a, V> = sts2_core::Listener<'a, V, CardInstance<V>>;

/// One copy of a card: which copy, and the state the fight has left it in.
///
/// Two copies of one card are two objects. A rule that raises the damage on
/// one leaves the other where it was. A pile stores the id and the state
/// apart, so copies in one state share it. See [`super::cards::Cards`].
#[derive(Debug)]
pub(crate) struct CardInstance<V: Rules> {
    /// Which copy this is, for as long as the fight holds it.
    pub(super) id: CardId<V>,
    pub(super) face: Face<V>,
}

impl<V: Rules> Clone for CardInstance<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: Rules> Copy for CardInstance<V> {}

impl<V: Rules> PartialEq for CardInstance<V> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.face == other.face
    }
}

impl<V: Rules> Eq for CardInstance<V> {}

impl<V: Rules> CardInstance<V> {
    /// A fresh copy, printing what the table prints.
    pub(super) fn new(ident: CardIdent<V>, id: CardId<V>) -> Self {
        Self {
            id,
            face: Face::fresh(ident),
        }
    }
}

/// A card's state, less the id that names a copy in it.
///
/// What a fight stores once however many copies share it. See
/// [`super::cards::Cards`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Face<V: Rules> {
    pub(super) ident: CardIdent<V>,
    /// What this copy prints, which starts as the table's and moves.
    ///
    /// Private, because how a copy keeps them is nobody else's business. A
    /// rule asks [`sts2_core::Printed::face`] for one by name, and the fight
    /// asks [`Face::numbers`] for all of them.
    vars: CardVarList,
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
    /// Empty for almost every card, and inline for the rest.
    cost_modifiers: [CostModifier; MODIFIERS],
    /// How many of [`Face::cost_modifiers`] a card holds.
    modifiers: u8,
    /// `CardModel.AddKeyword`, which writes words the table did not print.
    ///
    /// Beside the printed set rather than folded into it. The table's answer
    /// is the card's, and this is the copy's. Only the copy is written on.
    ///
    /// A set, which is what the build spells it as: one bit per word, in a
    /// byte.
    added_keywords: V::CardKeywordSet,
    /// `CardModel.BaseReplayCount`, the extra times this copy's play runs.
    ///
    /// Zero for every card the table prints. Only a power hands these out, and
    /// what it writes stays on the copy for the rest of the fight.
    ///
    /// A byte, because a card played more times than that is a fight nobody
    /// finishes. [`Face::replay`] says so rather than wrapping.
    replays: i8,
    /// `EnchantmentModel.Status`, which the enchantment's own `OnPlay` reads
    /// and sets. A fight starts every copy's at `Normal`.
    enchantment_status: sts2_core::EnchantmentStatus,
}

impl<V: Rules> Clone for Face<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: Rules> Copy for Face<V> {}

impl<V: Rules> Face<V> {
    /// The copy `id` names, in this state.
    pub(super) fn named(self, id: CardId<V>) -> CardInstance<V> {
        CardInstance { id, face: self }
    }
}

impl<V: Rules> sts2_core::Printed<V> for CardInstance<V> {
    fn face(&self, key: V::CardVar) -> V::Number {
        self.face.number(key).base
    }
}

impl<V: Rules> Face<V> {
    /// The entry this copy holds for `key`.
    ///
    /// A key the card does not print is a mistake in the port, the way a card
    /// asking for a number no card prints is.
    pub(super) fn number(&self, key: V::CardVar) -> CardNumber<V::CardVar, V::Number> {
        let vars = self.ident.card.vars();
        let at = Self::at(vars, key);
        self.printed(&vars[at], self.moved(at))
    }

    /// How far this copy has moved the number its row holds at `at`.
    fn moved(&self, at: usize) -> Amount {
        self.vars.map_or(0, |row| row[at].get())
    }

    /// Every number this copy prints, in the order its table prints them.
    ///
    /// The table names each one and says how it is worked out. This copy
    /// carries the amounts, which is the half a fight writes on.
    pub(super) fn numbers(&self) -> impl Iterator<Item = CardNumber<V::CardVar, V::Number>> + '_ {
        self.ident
            .card
            .vars()
            .iter()
            .enumerate()
            .map(|(at, named)| self.printed(named, self.moved(at)))
    }

    /// Where in the card's own row, `vars`, `key` sits.
    ///
    /// A key the card does not print is a mistake in the port, the way a card
    /// asking for a number no card prints is.
    fn at(
        vars: &[CardNumber<V::CardVar, <V::Card as sts2_core::CardTable>::Number>],
        key: V::CardVar,
    ) -> usize {
        vars.iter()
            .position(|named| named.id == key)
            .unwrap_or_else(|| unimplemented!("the {} this card does not print", key.name()))
    }

    /// One of the table's numbers, as this copy prints it.
    fn printed(
        &self,
        named: &CardNumber<V::CardVar, <V::Card as sts2_core::CardTable>::Number>,
        moved: Amount,
    ) -> CardNumber<V::CardVar, V::Number> {
        let printed: V::Number = num_traits::NumCast::from(named.base).unwrap_or_else(|| {
            unimplemented!("the fractional {} this card prints", named.id.name())
        });
        let moved: V::Number =
            num_traits::NumCast::from(moved).expect("a difference fits a game number");
        CardNumber {
            id: named.id,
            base: printed + moved,
            calculated: named.calculated,
            modifier: named.modifier,
        }
    }

    /// Add to what this copy prints for `key`.
    ///
    /// # Panics
    ///
    /// A card too wide to hold a difference. No fight raises one: the only
    /// card either build prints that wide belongs to an event, and no event
    /// is ported.
    ///
    /// A number raised past what an [`Amount`] holds. `ForgeCmd` is the only
    /// thing that raises one repeatedly, and arriving here takes two thousand
    /// smiths in one fight.
    pub(super) fn raise(&mut self, key: V::CardVar, by: V::Number) {
        let at = Self::at(self.ident.card.vars(), key);
        let by: Amount = num_traits::NumCast::from(by)
            .unwrap_or_else(|| unimplemented!("the {} raised by {by:?}", key.name()));
        let row = self.vars.as_mut().unwrap_or_else(|| {
            unimplemented!("the {} raised on a card too wide to hold one", key.name())
        });
        let moved = row[at]
            .get()
            .checked_add(by)
            .and_then(NonMaxI16::new)
            .unwrap_or_else(|| unimplemented!("the {} raised past what a card holds", key.name()));
        row[at] = moved;
    }

    /// A fresh copy's state, printing what the table prints.
    fn fresh(ident: CardIdent<V>) -> Self {
        // Zeroes, because a fresh copy prints what its row prints. A card too
        // wide for the row carries no differences at all, which is what its
        // `None` says.
        let vars = (ident.card.vars().len() <= VARS).then_some([NonMaxI16::ZERO; VARS]);
        let mut card = Self {
            ident,
            vars,
            cost: ident.card.cost(),
            star_cost: ident.card.star_cost(),
            cost_modifiers: [CostModifier::from_bits(0); MODIFIERS],
            modifiers: 0,
            added_keywords: sts2_core::FlagSet::from_members(&[]),
            replays: 0,
            enchantment_status: sts2_core::EnchantmentStatus::Normal,
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
                .take(usize::from(self.modifiers))
                .filter(|held| held.pool() == pool)
                .fold(printed, |cost, held| {
                    let (shift, amount) = match held.change() {
                        CostChange::Relative(amount) => (true, amount),
                        CostChange::Absolute(amount) => (false, amount),
                    };
                    let amount: V::Number = num_traits::NumCast::from(amount)
                        .expect("a cost change fits a game number");
                    match shift {
                        true => cost + amount,
                        false => amount,
                    }
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
            sts2_core::Change::Raise(key, amount) => self.raise(key, amount),
            sts2_core::Change::ChangeCost(pool, by) => {
                self.modify_cost(pool, CostChange::Relative(by), lasting)
            }
            sts2_core::Change::SetCost(pool, to) => {
                self.modify_cost(pool, CostChange::Absolute(to), lasting)
            }
            // `AddKeyword`, which is a set: a word already on the copy is not
            // added twice.
            sts2_core::Change::AddKeyword(keyword) => {
                self.added_keywords = sts2_core::FlagSet::with(self.added_keywords, keyword);
            }
        }
    }

    /// `Keywords.Contains(Innate)` for this copy.
    ///
    /// The union of what the table prints and what has been written on. Asked
    /// of Innate, Retain and Sly alone, because those are the keywords
    /// something in a fight both writes and reads. The rest are read off the table.
    pub(super) fn innate(&self) -> bool {
        use sts2_core::{CardTable as _, KeywordKinds};

        self.ident.card.innate()
            || sts2_core::FlagSet::contains(
                self.added_keywords,
                <V::CardKeyword as KeywordKinds<V>>::INNATE,
            )
    }

    /// `Keywords.Contains(Ethereal)` for this copy, the way [`Face::innate`]
    /// reads Innate.
    pub(super) fn ethereal(&self) -> bool {
        use sts2_core::{CardTable as _, KeywordKinds};

        self.ident.card.ethereal()
            || sts2_core::FlagSet::contains(
                self.added_keywords,
                <V::CardKeyword as KeywordKinds<V>>::ETHEREAL,
            )
    }

    /// `IsSlyThisTurn` for this copy, the keyword the way [`Face::innate`]
    /// reads Innate. Nothing ported makes a card Sly for one turn.
    pub(super) fn sly(&self) -> bool {
        use sts2_core::{CardTable as _, KeywordKinds};

        self.ident.card.sly()
            || sts2_core::FlagSet::contains(
                self.added_keywords,
                <V::CardKeyword as KeywordKinds<V>>::SLY,
            )
    }

    /// `Keywords.Contains(Retain)` for this copy, the way [`Face::innate`]
    /// reads Innate.
    pub(super) fn retains(&self) -> bool {
        use sts2_core::{CardTable as _, KeywordKinds};

        self.ident.card.retains()
            || sts2_core::FlagSet::contains(
                self.added_keywords,
                <V::CardKeyword as KeywordKinds<V>>::RETAIN,
            )
    }

    /// Append one cost modifier, which is what every `Set` and `Add` on
    /// `CardEnergyCost` does.
    ///
    /// A pool the card is not priced in takes none. The setters guard on
    /// `Canonical >= 0`, and `GetWithModifiers` returns early on the same
    /// test, so nothing reads a modifier there.
    fn modify_cost<N: std::fmt::Debug + Copy + num_traits::ToPrimitive>(
        &mut self,
        pool: sts2_core::CostPool,
        change: CostChange<N>,
        lasting: sts2_core::Lasting,
    ) {
        let printed = match pool {
            sts2_core::CostPool::Energy => self.cost,
            sts2_core::CostPool::Star => self.star_cost,
        };
        if printed == sts2_core::Cost::None {
            return;
        }
        let at = usize::from(self.modifiers);
        if at == MODIFIERS {
            unimplemented!("a card carrying more than {MODIFIERS} cost modifiers");
        }
        self.cost_modifiers[at] = CostModifier::held(pool, change, lasting);
        self.modifiers += 1;
    }

    /// Drop the cost modifiers that do not outlive this turn.
    pub(super) fn turn_ended(&mut self) {
        self.kept(CostModifier::clears_when_turn_ends);
    }

    /// Whether this copy holds a cost modifier that ends with the turn.
    pub(super) fn lasts_the_turn(&self) -> bool {
        self.cost_modifiers[..usize::from(self.modifiers)]
            .iter()
            .any(|held| held.clears_when_turn_ends())
    }

    /// `GetEnchantedReplayCount` and one: the replays this copy holds, through
    /// its enchantment's `EnchantPlayCount`, and the play itself.
    /// `Hook.ModifyCardPlayCount` runs over this where the play is made.
    pub(super) fn play_count(&self) -> V::Number {
        let replays: V::Number =
            num_traits::NumCast::from(self.replays).expect("a replay count fits a game number");
        let replays = match self.ident.enchantment {
            Some(enchanted) => super::board::enchanted_by::<V>(
                enchanted,
                sts2_core::EnchantHook::PlayCount,
                replays,
            ),
            None => replays,
        };
        replays + num_traits::one()
    }

    /// This copy's `EnchantmentModel.Status`.
    pub(super) fn enchantment_status(&self) -> sts2_core::EnchantmentStatus {
        self.enchantment_status
    }

    /// Set this copy's `EnchantmentModel.Status`, and say whether that changed
    /// it.
    pub(super) fn set_enchantment_status(&mut self, status: sts2_core::EnchantmentStatus) -> bool {
        std::mem::replace(&mut self.enchantment_status, status) != status
    }

    /// Drop the cost modifiers that do not outlive being played, and say
    /// whether there were any.
    pub(super) fn was_played(&mut self) -> bool {
        self.kept(CostModifier::clears_when_played)
    }

    /// Hand this copy one more play of its own, which is `BaseReplayCount`.
    ///
    /// # Panics
    ///
    /// More replays than a byte holds.
    pub(super) fn replay(&mut self, amount: V::Number) {
        let amount: i8 = num_traits::NumCast::from(amount)
            .unwrap_or_else(|| unimplemented!("a card replayed {amount:?} times"));
        self.replays = self
            .replays
            .checked_add(amount)
            .unwrap_or_else(|| unimplemented!("a card replayed past what a copy counts"));
    }

    /// Drop the cost modifiers `clears` names, and close the gaps they leave.
    ///
    /// In order, because a modifier that sets the cost overrides every one
    /// written before it. Compacted, because an empty slot ends the row. The
    /// answer is whether it dropped any.
    fn kept(&mut self, clears: fn(CostModifier) -> bool) -> bool {
        let mut to = 0;
        for at in 0..usize::from(self.modifiers) {
            let held = self.cost_modifiers[at];
            if !clears(held) {
                self.cost_modifiers[to] = held;
                to += 1;
            }
        }
        let dropped = usize::from(self.modifiers) != to;
        self.modifiers = to as u8;
        dropped
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
        // `RecalculateForUpgradeOrEnchant`, which gives a calculated number
        // the base its `CalculationBase` now prints.
        let base = <V::CardVar as sts2_core::CardVars>::CALCULATION_BASE;
        let calculated: smallvec::SmallVec<[_; 2]> =
            self.numbers().filter(|n| n.calculated).collect();
        for number in calculated {
            let by = self.number(base).base - number.base;
            if by != num_traits::zero() {
                self.raise(number.id, by);
            }
        }
        self.ident.upgrade += 1;
    }
}

/// One creature as content reads it: the creature, and the powers the fight
/// holds for it.
pub(crate) struct Seen<'a, V: Rules> {
    pub(crate) fighter: &'a Fighter<V>,
    pub(crate) powers: &'a PowerStore<V>,
}

impl<V: Rules> sts2_core::CreatureView<V> for Seen<'_, V> {
    fn id(&self) -> CombatId {
        self.fighter.id
    }

    fn side(&self) -> V::CombatSide {
        self.fighter.side
    }

    fn alive(&self) -> bool {
        self.fighter.alive
    }

    fn hp(&self) -> V::Health {
        self.fighter.hp
    }

    fn max_hp(&self) -> V::Health {
        self.fighter.max_hp
    }

    fn owner(&self) -> Option<CombatId> {
        self.fighter.owner
    }

    fn power(&self, power: V::Power) -> Option<sts2_core::PowerAmount<V::Number>> {
        self.powers
            .of(self.fighter.id)
            .find(|held| held.power() == power)
            .map(|held| held.amount.amount())
    }
}

/// How much of a power a creature carries, as the fight holds one.
///
/// [`sts2_core::PowerAmount`] narrowed. The count is an [`Amount`] and the
/// endless flag is its niche, not a tag beside it. So this is two bytes where
/// the report's is eight, and `PowerModel.Amount` is one number either way.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Stacked(Option<NonMaxI16>);

impl Stacked {
    /// One instance with no number to run out, which is `IsInfinite`.
    pub(crate) const ENDLESS: Self = Self(None);

    /// A number that stacks, and counts down where the power counts down.
    ///
    /// # Panics
    ///
    /// A stack no [`Amount`] holds.
    pub(crate) fn counted<N: std::fmt::Debug + Copy + num_traits::ToPrimitive>(amount: N) -> Self {
        Self(Some(
            num_traits::NumCast::from(amount)
                .and_then(NonMaxI16::new)
                .unwrap_or_else(|| unimplemented!("a power stacked to {amount:?}")),
        ))
    }

    /// The number, or nothing for a power with none to run out.
    pub(crate) fn count(self) -> Option<Amount> {
        self.0.map(|amount| amount.get())
    }

    /// Counted down to nothing, which is a power the fight drops.
    pub(crate) fn spent(self) -> bool {
        self.count() == Some(0)
    }

    /// One less of it. An endless power has no number to count.
    pub(crate) fn ticked(self) -> Self {
        match self.count() {
            Some(amount) => Self::counted(amount - 1),
            None => self,
        }
    }

    /// What a report shows of it, and what a rule names one with.
    pub(crate) fn amount<N: num_traits::NumCast>(self) -> sts2_core::PowerAmount<N> {
        match self.count() {
            Some(amount) => sts2_core::PowerAmount::Counted(
                num_traits::NumCast::from(amount).expect("a stack fits a game number"),
            ),
            None => sts2_core::PowerAmount::Endless,
        }
    }
}

impl<N: std::fmt::Debug + Copy + num_traits::ToPrimitive> From<sts2_core::PowerAmount<N>>
    for Stacked
{
    fn from(amount: sts2_core::PowerAmount<N>) -> Self {
        match amount {
            sts2_core::PowerAmount::Counted(count) => Self::counted(count),
            sts2_core::PowerAmount::Endless => Self::ENDLESS,
        }
    }
}

/// The move a monster has chosen, as the fight holds one.
///
/// [`sts2_core::NextMove`] narrowed. The report's is a name beside a
/// `SmallVec`. That pays a capacity word and a heap branch for a list the
/// same type's own comment says never exceeds two. This is a fixed pair, so
/// it is fourteen bytes where the report's is fifty-six, and `Option` takes
/// its niche from the move name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Telegraphed<V: Rules> {
    /// The name, at the width the build spells it. 275 moves outrun a byte.
    pub(crate) id: V::MoveId,
    /// Each slot in the order the game draws it, and a hole for a move that
    /// draws fewer. The kinds carry a niche, so the holes cost nothing.
    intents: [Option<Intended<V>>; INTENTS],
}

/// How many intents one move telegraphs.
///
/// An attack that also blocks draws two, and no ported move draws three.
/// [`sts2_core::NextMove`] inlines to the same number and says why.
const INTENTS: usize = 2;

impl<V: Rules> Telegraphed<V> {
    /// `UNSET_MOVE`, which a pet shows and never performs.
    pub(crate) fn unset() -> Self {
        Telegraphed {
            id: <V::MoveId as sts2_core::MoveKinds>::UNSET,
            intents: [None; INTENTS],
        }
    }

    /// What the move draws, in the order the game draws it.
    fn intents(&self) -> impl Iterator<Item = &Intended<V>> {
        self.intents.iter().flatten()
    }

    /// The numbers of the first intent that attacks, which is what the move
    /// performs with.
    pub(crate) fn attack(&self) -> Option<Landing> {
        self.intents().find_map(|intent| intent.attack)
    }

    /// What a report shows of it.
    pub(crate) fn telegraph(&self) -> sts2_core::NextMove<V> {
        sts2_core::NextMove {
            id: self.id,
            intents: self.intents().map(Intended::intent).collect(),
        }
    }
}

impl<V: Rules> From<&sts2_core::NextMove<V>> for Telegraphed<V> {
    /// # Panics
    ///
    /// A move drawing more intents than [`INTENTS`].
    fn from(next: &sts2_core::NextMove<V>) -> Self {
        let drawn = next.intents.len();
        if drawn > INTENTS {
            unimplemented!("a move drawing {drawn} intents");
        }
        let mut intents = [None; INTENTS];
        for (slot, intent) in intents.iter_mut().zip(&next.intents) {
            *slot = Some(Intended::from(*intent));
        }
        Self {
            id: next.id,
            intents,
        }
    }
}

/// One thing a move telegraphs, as the fight holds one.
///
/// [`sts2_core::Intent`] narrowed: six bytes rather than sixteen, because the
/// numbers beside the kind fit their own widths.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Intended<V: Rules> {
    kind: V::IntentKind,
    attack: Option<Landing>,
}

impl<V: Rules> Intended<V> {
    /// What a report shows of it.
    fn intent(&self) -> sts2_core::Intent<V> {
        sts2_core::Intent {
            kind: self.kind,
            attack: self.attack.map(Landing::attack),
        }
    }
}

impl<V: Rules> From<sts2_core::Intent<V>> for Intended<V> {
    fn from(intent: sts2_core::Intent<V>) -> Self {
        Self {
            kind: intent.kind,
            attack: intent.attack.map(Landing::from),
        }
    }
}

/// What an attack will land, as the fight holds one.
///
/// [`sts2_core::Attack`] narrowed. The content types the damage a
/// `Scaled<u16>` and the hit count a `Scaled<u8>`, and the report widens both
/// to a game number. The damage keeps a width that survives `ModifyDamage`,
/// which adds Strength with no ceiling. The hit count keeps the width the
/// content gave it. Both are `NonMax`, so `Option<Landing>` costs no tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Landing {
    damage: NonMaxI16,
    repeats: NonMaxU8,
}

impl Landing {
    /// What a report shows of it, and what a move performs with.
    pub(crate) fn attack<V: Rules>(self) -> sts2_core::Attack<V> {
        sts2_core::Attack {
            damage: num_traits::NumCast::from(self.damage.get())
                .expect("damage fits a game number"),
            repeats: num_traits::NumCast::from(self.repeats.get())
                .expect("a hit count fits a game count"),
        }
    }
}

impl<V: Rules> From<sts2_core::Attack<V>> for Landing {
    /// # Panics
    ///
    /// Damage or a hit count neither field holds.
    fn from(attack: sts2_core::Attack<V>) -> Self {
        Self {
            damage: num_traits::NumCast::from(attack.damage)
                .and_then(NonMaxI16::new)
                .unwrap_or_else(|| unimplemented!("an attack landing {:?}", attack.damage)),
            repeats: num_traits::NumCast::from(attack.repeats)
                .and_then(NonMaxU8::new)
                .unwrap_or_else(|| unimplemented!("an attack landing {:?} times", attack.repeats)),
        }
    }
}

/// The creature carrying a power, as a hook reaching the power reads it:
/// `Owner`, its side, and `Owner.Monster`.
pub(crate) struct Carrier<V: Rules> {
    pub(crate) side: V::CombatSide,
    pub(crate) id: CombatId,
    pub(crate) monster: Option<V::Monster>,
}

impl<V: Rules> Clone for Carrier<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: Rules> Copy for Carrier<V> {}

/// One power on a creature, with the fields its own model saves.
///
/// [`sts2_core::Stack`] is what a report shows of a power: which one, and the
/// amount. This is what the fight carries, which is that pair and the model
/// beside it. See [`PowerStore`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct HeldPower<V: Rules> {
    pub(crate) amount: Stacked,
    /// `PowerModel.AmountOnTurnStart`, which its holder's side writes as it
    /// begins a turn.
    pub(crate) on_turn_start: Stacked,
    /// `PowerModel.Applier`.
    pub(crate) applier: Option<CombatId>,
    pub(crate) state: V::PowerState,
}

impl<V: Rules> Clone for HeldPower<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: Rules> Copy for HeldPower<V> {}

impl<V: Rules> HeldPower<V> {
    /// Which power this is, which its model knows.
    pub(crate) fn power(&self) -> V::Power {
        sts2_core::PowerModel::power(&self.state)
    }

    /// A power as `PowerCmd.Apply` lands one, its model as new as it is.
    pub(crate) fn new(power: V::Power, amount: Stacked, applier: Option<CombatId>) -> Self {
        Self {
            amount,
            on_turn_start: Stacked::counted(0),
            applier,
            state: <V::PowerState as sts2_core::PowerModel<V>>::new(power),
        }
    }

    /// This power as a hook's listener, which is the instance the game hands
    /// the hook to.
    ///
    /// A power is held by a creature and does not know which, so the caller
    /// that found it says.
    pub(crate) fn listener<'a>(
        &'a self,
        carrier: Carrier<V>,
        history: sts2_core::History<'a, V>,
    ) -> Heard<'a, V> {
        sts2_core::Listener::Power {
            power: self.power(),
            amount: self.amount.amount(),
            owner: carrier.side,
            holder: carrier.id,
            applier: self.applier,
            monster: carrier.monster,
            on_turn_start: self.on_turn_start.amount(),
            state: &self.state,
            history,
        }
    }

    pub(crate) fn event_power<'a>(
        &'a mut self,
        carrier: Carrier<V>,
        history: sts2_core::History<'a, V>,
    ) -> sts2_core::EventPower<'a, V> {
        sts2_core::EventPower {
            power: self.power(),
            amount: self.amount.amount(),
            owner: carrier.side,
            holder: carrier.id,
            applier: self.applier,
            monster: carrier.monster,
            on_turn_start: self.on_turn_start.amount(),
            state: &mut self.state,
            history,
        }
    }

    /// What a report shows of it.
    pub(crate) fn stack(&self) -> sts2_core::Stack<V> {
        sts2_core::Stack {
            power: self.power(),
            amount: self.amount.amount(),
        }
    }
}

/// One creature in the fight.
///
/// [`sts2_core::Creature`] is what a report shows. This is what the fight
/// holds of it besides its powers. The fight keeps those in its [`PowerStore`],
/// with a model each rather than a bare stack.
#[derive(Debug)]
pub(crate) struct Fighter<V: Rules> {
    pub(crate) id: CombatId,
    pub(crate) model: Option<sts2_core::ModelId<V>>,
    pub(crate) side: V::CombatSide,
    pub(crate) hp: V::Health,
    pub(crate) max_hp: V::Health,
    pub(crate) block: V::Number,
    pub(crate) alive: bool,
    pub(crate) next_move: Option<Telegraphed<V>>,
    /// `PetOwner`: the player a pet stands beside. Nothing for a player or a
    /// monster.
    pub(crate) owner: Option<CombatId>,
}

impl<V: Rules> Clone for Fighter<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: Rules> Copy for Fighter<V> {}

impl<V: Rules> Fighter<V> {
    /// This creature, as the powers it carries are asked about it.
    pub(crate) fn carrier(&self) -> Carrier<V> {
        Carrier {
            side: self.side,
            id: self.id,
            monster: match self.model {
                Some(sts2_core::ModelId::Monster(monster)) => Some(monster),
                _ => None,
            },
        }
    }

    /// What a report shows of it, which is everything but the models.
    ///
    /// The powers and the move are the fight's to give. The fight holds the
    /// one and modifies the other's damage. See [`Live::telegraphed`].
    pub(crate) fn creature<'a>(
        &self,
        powers: impl Iterator<Item = &'a HeldPower<V>>,
        next_move: Option<sts2_core::NextMove<V>>,
    ) -> Creature<V> {
        Creature {
            id: self.id,
            model: self.model,
            side: self.side,
            hp: self.hp,
            max_hp: self.max_hp,
            block: self.block,
            alive: self.alive,
            powers: powers.map(HeldPower::stack).collect(),
            next_move,
        }
    }
}

/// The creatures in a fight, and how many of their monster models override
/// each hook.
///
/// A creature arrives or leaves only through here, which is what keeps the
/// counts true. Everything else reads and writes the creatures as a slice.
#[derive(Debug)]
pub(crate) struct Creatures<V: Rules> {
    list: Bounded<Fighter<V>, CREATURES>,
    counts: super::listen::HookCounts,
}

impl<V: Rules> Clone for Creatures<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: Rules> Copy for Creatures<V> {}

impl<V: Rules> std::ops::Deref for Creatures<V> {
    type Target = [Fighter<V>];

    fn deref(&self) -> &[Fighter<V>] {
        &self.list
    }
}

impl<V: Rules> std::ops::DerefMut for Creatures<V> {
    fn deref_mut(&mut self) -> &mut [Fighter<V>] {
        &mut self.list
    }
}

impl<V: Rules> FromIterator<Fighter<V>> for Creatures<V> {
    fn from_iter<I: IntoIterator<Item = Fighter<V>>>(fighters: I) -> Self {
        let mut creatures = Self::new();
        for fighter in fighters {
            creatures.push(fighter);
        }
        creatures
    }
}

impl<V: Rules> Creatures<V> {
    pub(crate) fn new() -> Self {
        Self {
            list: Bounded::new(),
            counts: super::listen::HookCounts::default(),
        }
    }

    /// The hooks `fighter`'s own model overrides, which is none for the
    /// player's: a character is not a listener.
    fn overrides(fighter: &Fighter<V>) -> sts2_core::Overrides {
        use sts2_core::MonsterTable as _;

        match fighter.model {
            Some(sts2_core::ModelId::Monster(monster)) => monster.overrides(),
            _ => sts2_core::Overrides::default(),
        }
    }

    pub(crate) fn push(&mut self, fighter: Fighter<V>) {
        self.counts.add(Self::overrides(&fighter));
        self.list.push(fighter);
    }

    pub(crate) fn insert(&mut self, at: usize, fighter: Fighter<V>) {
        self.counts.add(Self::overrides(&fighter));
        self.list.insert(at, fighter);
    }

    pub(crate) fn retain(&mut self, mut keep: impl FnMut(&Fighter<V>) -> bool) {
        let counts = &mut self.counts;
        self.list.retain(|fighter| {
            let kept = keep(fighter);
            if !kept {
                counts.remove(Self::overrides(fighter));
            }
            kept
        });
    }

    pub(crate) fn copy_from(&mut self, source: &Self) {
        self.list.copy_from(&source.list);
        self.counts = source.counts;
    }

    /// Whether any monster model in the fight overrides a hook in `hooks`.
    pub(crate) fn hear(&self, hooks: sts2_core::Overrides) -> bool {
        self.counts.hears(hooks)
    }
}

/// A fight in progress.
#[derive(Debug)]
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
    /// `ICombatState.CurrentSide`: whose turn it is.
    pub(super) side: V::CombatSide,
    /// `CombatManager.History`.
    pub(super) history: CombatHistory<V>,
    /// The encounter this active fight belongs to.
    pub(super) encounter: V::Encounter,
    pub(super) cards: super::cards::Cards<V>,
    /// Deck cards a combat command removed permanently while this fight ran.
    pub(super) removed_from_deck: Vec<CardIdent<V>>,
    /// Cards a listener handed back as it died, which a won fight offers as
    /// `SpecialCardReward`s.
    pub(super) returned: Vec<CardIdent<V>>,
    /// What `AfterCombatEnd` asked of the run, which carries it out as it
    /// takes the fight's rewards.
    pub(super) leaving: Vec<sts2_core::HookEffect<V>>,
    pub(super) creatures: Creatures<V>,
    /// What every creature carries, in the order it gained it.
    pub(super) powers: PowerStore<V>,
    pub(super) orbs: Option<Orbs<V>>,
    /// One per monster in the fight, in the order the encounter fields them.
    pub(super) machines: Bounded<Machine<V>, CREATURES>,
    /// `PowerModel.SkipNextDurationTick`, as the powers holding it.
    ///
    /// A debuff landing on a player-side creature does not count down at the
    /// end of the turn it landed in. The game keeps that on the power
    /// instance. The observation's [`sts2_core::Stack`] is the game's own wire
    /// shape and carries no such field, so the fight keeps it beside them.
    pub(super) skip_tick: Skips<V>,
    /// The ids this fight has given out, and the next one it will.
    pub(super) card_ids: CardIds<V>,
    /// How many attack commands this fight has started, which is what the
    /// next one is told apart by.
    pub(super) attacks: u32,
    /// A card play waiting on an answer, which is the only thing the fight
    /// will take while it is set.
    pub(super) asking: Option<Asking<V>>,
}

/// How many history entries one turn holds before its list reaches the heap:
/// a hand's draws, and a few plays each started and finished.
const ENTRIES_INLINE: usize = 24;

/// `CombatManager.History`: the turn the fight stands in, and the turns
/// before it.
///
/// A turn is frozen into the shared chain as the sides switch. Copying a
/// fight copies this turn's few entries and one pointer.
pub(crate) struct CombatHistory<V: Rules> {
    this_turn: smallvec::SmallVec<[sts2_core::HistoryEntry<V>; ENTRIES_INLINE]>,
    before: Option<std::sync::Arc<sts2_core::TurnHistory<V>>>,
}

impl<V: Rules> CombatHistory<V> {
    pub(super) fn new() -> Self {
        CombatHistory {
            this_turn: smallvec::SmallVec::new(),
            before: None,
        }
    }

    /// `History.CardPlayStarted`, in the turn the fight stands in.
    pub(super) fn start(&mut self, card: V::Card, energy: V::Count) {
        let started = sts2_core::PlayStarted { card, energy };
        self.this_turn
            .push(sts2_core::HistoryEntry::Started(started));
    }

    /// `History.CardPlayFinished`, in the turn the fight stands in.
    pub(super) fn finish(&mut self, card: V::Card) {
        let finished = sts2_core::PlayFinished { card };
        self.this_turn
            .push(sts2_core::HistoryEntry::Finished(finished));
    }

    /// `History.CardDrawn`, in the turn the fight stands in.
    pub(super) fn draw(&mut self, card: V::Card) {
        self.this_turn.push(sts2_core::HistoryEntry::Drawn(card));
    }

    /// The turn at `round` on `side` is over. Its entries join the chain,
    /// unless it had none.
    fn end_turn(&mut self, round: V::Count, side: V::CombatSide) {
        if self.this_turn.is_empty() {
            return;
        }
        let turn = sts2_core::TurnHistory {
            round,
            side,
            entries: self.this_turn.drain(..).collect(),
            before: self.before.take(),
        };
        self.before = Some(std::sync::Arc::new(turn));
    }

    pub(super) fn view(&self) -> sts2_core::History<'_, V> {
        sts2_core::History {
            this_turn: &self.this_turn,
            before: self.before.as_deref(),
        }
    }
}

impl<V: Rules> std::fmt::Debug for CombatHistory<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CombatHistory")
            .field("this_turn", &self.this_turn)
            .field("before", &self.before)
            .finish()
    }
}

impl<V: Rules> Clone for CombatHistory<V> {
    fn clone(&self) -> Self {
        CombatHistory {
            this_turn: self.this_turn.clone(),
            before: self.before.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.this_turn.clone_from(&source.this_turn);
        self.before.clone_from(&source.before);
    }
}

impl<V: Rules> Clone for Live<V> {
    fn clone(&self) -> Self {
        Self {
            cards: self.cards.clone(),
            removed_from_deck: self.removed_from_deck.clone(),
            returned: self.returned.clone(),
            history: self.history.clone(),
            leaving: self.leaving.clone(),
            powers: self.powers.clone(),
            orbs: self.orbs.clone(),
            card_ids: self.card_ids.clone(),
            asking: self.asking.clone(),
            ..*self
        }
    }

    /// Into the fight already here, field by field, so the lists keep what
    /// they allocated and copy only what `source` holds.
    fn clone_from(&mut self, source: &Self) {
        let Live {
            character,
            ascension,
            turn,
            phase,
            energy,
            stars,
            round,
            side,
            history,
            encounter,
            cards,
            removed_from_deck,
            returned,
            leaving,
            creatures,
            powers,
            orbs,
            machines,
            skip_tick,
            card_ids,
            attacks,
            asking,
        } = self;
        *character = source.character;
        *ascension = source.ascension;
        *turn = source.turn;
        *phase = source.phase;
        *energy = source.energy;
        *stars = source.stars;
        *round = source.round;
        *side = source.side;
        history.clone_from(&source.history);
        *encounter = source.encounter;
        cards.clone_from(&source.cards);
        removed_from_deck.clone_from(&source.removed_from_deck);
        returned.clone_from(&source.returned);
        leaving.clone_from(&source.leaving);
        creatures.copy_from(&source.creatures);
        powers.clone_from(&source.powers);
        orbs.clone_from(&source.orbs);
        machines.copy_from(&source.machines);
        skip_tick.copy_from(&source.skip_tick);
        card_ids.clone_from(&source.card_ids);
        *attacks = source.attacks;
        asking.clone_from(&source.asking);
    }
}

impl<V: Rules> Live<V> {
    pub(crate) fn removed_from_deck(&self) -> &[CardIdent<V>] {
        &self.removed_from_deck
    }

    /// The cards this fight hands back once it is won, in the order they came
    /// back.
    pub(crate) fn returned(&self) -> &[CardIdent<V>] {
        &self.returned
    }

    /// What the fight's end asked of the run, in the order it asked.
    pub(crate) fn leaving(&self) -> &[sts2_core::HookEffect<V>] {
        &self.leaving
    }

    /// Where each card a selection from `pile` offers stands in it, in the
    /// order the selector is handed them.
    ///
    /// `FromCombatPile` sorts the draw pile by rarity and then by id, a
    /// stable sort. Any other pile is handed over as it lies.
    pub(crate) fn offered(&self, pile: sts2_core::Pile) -> smallvec::SmallVec<[usize; 16]> {
        use sts2_core::CardTable as _;

        let mut offered: smallvec::SmallVec<[usize; 16]> = (0..self.cards.len(pile)).collect();
        if pile == sts2_core::Pile::Draw {
            offered.sort_by_key(|&at| {
                let card = self
                    .cards
                    .get(pile, at)
                    .expect("a card stands at each place in its pile")
                    .face
                    .ident
                    .card;
                (card.rarity(), card.order())
            });
        }
        offered
    }

    /// The card-selection question on which this fight is parked.
    pub(crate) fn question(&self) -> Option<sts2_core::Question<V>> {
        let asked = self.asking.as_ref()?;
        let (min, max, _) = self.selecting()?;
        let options: Vec<_> = match &asked.question {
            Asked::Pile { pile, .. } => self
                .offered(*pile)
                .into_iter()
                .map(|at| {
                    self.cards
                        .get(*pile, at)
                        .expect("an offered card stands in its pile")
                        .face
                        .ident
                })
                .collect(),
            Asked::Offered { offered, .. } => offered.to_vec(),
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

    /// How many cards the parked selection takes at least and at most, and
    /// how many it offers.
    ///
    /// The whole of what the legal answers read, so they are listed without
    /// building the question.
    pub(crate) fn selecting(&self) -> Option<(u32, u32, usize)> {
        let asked = self.asking.as_ref()?;
        Some(match &asked.question {
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
                (min, max, self.cards.len(*pile))
            }
            Asked::Offered { offered, skippable } => (u32::from(!skippable), 1, offered.len()),
        })
    }

    /// What a creature's next move shows, with each attack's damage as it
    /// will land on the player.
    ///
    /// `Attack::damage` is the number that will land, not the one the move
    /// prints. The game works it out on every read.
    pub(crate) fn telegraphed(
        &self,
        relics: &crate::run::Belt<V>,
        fighter: &Fighter<V>,
    ) -> Option<sts2_core::NextMove<V>> {
        let mut next = fighter.next_move.as_ref()?.telegraph();
        let player = self
            .creatures
            .iter()
            .find(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER);
        if let Some(player) = player {
            for intent in &mut next.intents {
                if let Some(attack) = &mut intent.attack {
                    attack.damage = super::board::modified::<V>(
                        super::listen::Walk { live: self, relics },
                        fighter.id,
                        Some(player.id),
                        None,
                        attack.damage,
                    );
                }
            }
        }
        Some(next)
    }

    /// The player, which every fight has one of.
    /// The orb queue as the game reports it, each number through
    /// `ModifyOrbValue`. The queue itself, while nothing listens.
    pub(crate) fn reported_orbs(
        &self,
        relics: &crate::run::Belt<V>,
    ) -> Option<std::borrow::Cow<'_, sts2_core::Orbs<V>>> {
        let orbs = self.orbs.as_ref()?;
        if !super::listen::heard::<V>(self, relics, sts2_core::ModelHook::ModifyOrbValue) {
            return Some(std::borrow::Cow::Borrowed(orbs));
        }
        let walk = || super::listen::Walk { live: self, relics };
        let mut reported = orbs.clone();
        for held in &mut reported.queue {
            held.passive = super::board::orb_value::<V>(walk(), held.orb, held.passive);
            held.evoke = super::board::orb_value::<V>(walk(), held.orb, held.evoke);
        }
        Some(std::borrow::Cow::Owned(reported))
    }

    /// A change a card writes on itself, wherever in the fight it stands.
    ///
    /// # Panics
    ///
    /// A card no combat pile holds.
    pub(super) fn modify_self(
        &mut self,
        id: CardId<V>,
        change: sts2_core::Change<V>,
        lasting: sts2_core::Lasting,
    ) {
        let (pile, at) = [
            sts2_core::Pile::Hand,
            sts2_core::Pile::Draw,
            sts2_core::Pile::Discard,
            sts2_core::Pile::Exhaust,
        ]
        .into_iter()
        .find_map(|pile| self.cards.find(pile, id).map(|at| (pile, at)))
        .unwrap_or_else(|| unimplemented!("a card writing on itself out of every pile"));
        self.cards.write(pile, at, |held| {
            held.modify(change, lasting);
            true
        });
    }

    /// `CombatManager.History`, as a listener reads it.
    pub(crate) fn history(&self) -> sts2_core::History<'_, V> {
        self.history.view()
    }

    /// `SwitchSides`: the turn now over joins the history, and `side` takes
    /// its turn.
    pub(super) fn switch_side(&mut self, side: V::CombatSide) {
        self.history.end_turn(self.round, self.side);
        self.side = side;
    }

    /// `CurrentRoom.RoomType`, which is the encounter's.
    pub(crate) fn room(&self) -> V::RoomType {
        <V::Encounter as sts2_core::EncounterTable>::room(self.encounter)
    }

    pub(crate) fn player(&self) -> &Fighter<V> {
        self.creatures
            .iter()
            .find(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER)
            .expect("a fight has the player in it")
    }

    /// `PowerCmd.Remove`: `power` off the creature `owner`, if it is still
    /// there to take it off, and then its `AfterRemoved`.
    ///
    /// # Panics
    ///
    /// A power asking for something as it comes off that nothing here carries
    /// out, or taking block that breaks as it goes.
    pub(crate) fn remove_power(&mut self, owner: CombatId, power: V::Power) {
        let Some(creature) = self.creatures.iter_mut().find(|c| c.id == owner) else {
            return;
        };
        let Some(at) = self.powers.of(owner).position(|held| held.power() == power) else {
            return;
        };
        let carrier = creature.carrier();
        let mut held = self.powers.remove(owner, at);
        let mut asked = sts2_core::HookEffects::new();
        V::power_event_hook(
            held.event_power(carrier, self.history.view()),
            sts2_core::EventHook::Removed,
            &mut asked,
        );
        for effect in asked {
            match effect {
                // `CreatureCmd.LoseBlock` for all of it. Block it takes raises
                // `AfterBlockBroken`, which a power coming off has no fight to
                // raise it on.
                sts2_core::HookEffect::LoseBlock if creature.block > num_traits::zero() => {
                    unimplemented!("{power:?} taking block as it comes off")
                }
                sts2_core::HookEffect::LoseBlock => {}
                other => unimplemented!("{power:?} asking for {other:?} as it comes off"),
            }
        }
    }

    /// This fight as a snapshot, and the only way out of here.
    ///
    /// The one place a `Combat` is made. The hand is held to the rules here
    /// and nowhere else, so a decision nobody observes costs none of that.
    ///
    /// `Attack::damage` comes out as the number that will land, not the one
    /// the move prints. The game works it out on every read.
    #[tracing::instrument(level = "trace", skip_all)]
    pub(crate) fn report(&self, relics: &crate::run::Belt<V>) -> Combat<V> {
        let creatures = self
            .creatures
            .iter()
            .map(|fighter| {
                fighter.creature(
                    self.powers.of(fighter.id),
                    self.telegraphed(relics, fighter),
                )
            })
            .collect();
        Combat {
            turn: self.turn,
            phase: self.phase,
            energy: self.energy,
            stars: self.stars,
            round: self.round,
            encounter: Some(self.encounter),
            hand: self.cards.idents(Pile::Hand).collect(),
            draw: self.cards.idents(Pile::Draw).collect(),
            discard: self.cards.idents(Pile::Discard).collect(),
            exhaust: self.cards.idents(Pile::Exhaust).collect(),
            creatures,
            legal: super::legal::hand::<V>(
                self.cards.iter(Pile::Hand),
                super::listen::Walk { live: self, relics },
                self.energy,
                self.stars,
            ),
            orbs: self.reported_orbs(relics).map(std::borrow::Cow::into_owned),
        }
    }

    /// Consume a won fight into the report that remains on its reward screen.
    pub(crate) fn finish(mut self, relics: &crate::run::Belt<V>) -> Combat<V> {
        self.phase = <V::PlayerTurnPhase as sts2_core::TurnPhaseKinds>::NONE;
        self.cards.clear();
        let player = self
            .creatures
            .iter_mut()
            .find(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER)
            .expect("a fight has the player in it");
        player.block = num_traits::zero();
        let player = player.id;
        self.powers.clear(player);
        self.report(relics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A packed modifier hands back every field it was given.
    ///
    /// Four expirations, two pools, and a shift or a set. The amount is signed
    /// for a shift and a plain price for a set, so both ends are checked.
    #[test]
    fn a_packed_cost_modifier_reads_back() {
        use sts2_core::{CostPool, Lasting};

        let expirations = [
            Lasting::ThisFight,
            Lasting::UntilPlayed,
            Lasting::ThisTurn,
            Lasting::ThisTurnOrUntilPlayed,
        ];
        for pool in [CostPool::Energy, CostPool::Star] {
            for lasting in expirations {
                for change in [CostChange::Relative(-3), CostChange::Absolute(0)] {
                    let packed = CostModifier::held(pool, change, lasting);
                    assert_eq!(packed.pool(), pool);
                    assert_eq!(packed.change(), change);
                    assert_eq!(
                        packed.clears_when_turn_ends(),
                        lasting.clears_when_turn_ends(),
                    );
                    assert_eq!(packed.clears_when_played(), lasting.clears_when_played());
                }
            }
        }
    }

    /// The widest amount a cost names survives the pack.
    #[test]
    fn a_cost_modifier_holds_the_widest_price() {
        use sts2_core::CostPool;

        for shift in [*SHIFT_WITHIN.start(), -1, 0, 1, *SHIFT_WITHIN.end()] {
            let change = CostChange::Relative(shift);
            let packed =
                CostModifier::held(CostPool::Energy, change, sts2_core::Lasting::ThisFight);
            assert_eq!(packed.change(), change, "a shift of {shift} survives");
        }
        for set in [0, 1, SET_AT_MOST] {
            let change = CostChange::Absolute(set);
            let packed =
                CostModifier::held(CostPool::Energy, change, sts2_core::Lasting::ThisFight);
            assert_eq!(packed.change(), change, "a cost set to {set} survives");
        }
    }

    /// Dropping the expired ones keeps the rest, in the order they arrived.
    ///
    /// Order is what decides the fold: a modifier that sets the cost overrides
    /// every one written before it. So a gap in the middle has to close rather
    /// than leave the row out of order.
    #[test]
    fn dropping_a_modifier_closes_the_gap() {
        use sts2_core::{CostPool, Lasting};
        type V = sts2_content::v0_107_1::V0_107_1;

        let mut card = CardInstance::<V>::new(
            CardIdent::new(sts2_content::v0_107_1::cards::Cards::Regent(
                sts2_content::v0_107_1::cards::RegentCard::Strike,
            )),
            CardId::new(0),
        );
        // A turn's worth in the middle of two that outlive it.
        for (change, lasting) in [
            (CostChange::Relative(-1), Lasting::ThisFight),
            (CostChange::Relative(-2), Lasting::ThisTurn),
            (CostChange::Absolute(3), Lasting::ThisFight),
        ] {
            card.face.modify_cost(CostPool::Energy, change, lasting);
        }

        card.face.turn_ended();

        let kept: Vec<CostChange<i8>> = card
            .face
            .cost_modifiers
            .iter()
            .take(usize::from(card.face.modifiers))
            .map(|held| held.change())
            .collect();
        assert_eq!(
            kept,
            [CostChange::Relative(-1), CostChange::Absolute(3)],
            "the turn's own goes, and the two around it close up",
        );
    }

    /// Every cost either build prints fits what a modifier sets.
    ///
    /// [`SET_AT_MOST`] is four bits, which is where the bytes went. The
    /// dearest card either build prints costs twelve. A build printing a
    /// dearer one moves the packing rather than finding out in a fight.
    #[test]
    fn every_cost_fits_a_modifier() {
        use sts2_core::CardTable as _;

        fn costs<V: sts2_core::ContentVersion>(build: &str) {
            for &card in <V::Card as sts2_core::Table>::ALL {
                for cost in [card.cost(), card.star_cost()] {
                    let sts2_core::Cost::Fixed(printed) = cost else {
                        continue;
                    };
                    assert!(
                        i16::from(printed.get()) <= i16::from(SET_AT_MOST),
                        "{build} prints {card:?} at {}, and a modifier sets at most \
                         {SET_AT_MOST}",
                        printed.get(),
                    );
                }
            }
        }
        costs::<sts2_content::v0_107_1::V0_107_1>("v0.107.1");
        costs::<sts2_content::v0_111_0::V0_111_0>("v0.111.0");
    }

    /// One card in each ported build prints more than a copy holds.
    ///
    /// [`VARS`] is a measured constant, so it goes stale the moment a build
    /// prints another wide card. The one that is already wide is an event's,
    /// and no event is ported, so no fight raises it. A second one is a
    /// decision somebody has to make rather than a panic in a fight.
    #[test]
    fn only_one_card_prints_more_than_a_copy_holds() {
        fn wide<V: sts2_core::ContentVersion>(build: &str) {
            let mut wide = 0;
            for &card in <V::Card as sts2_core::Table>::ALL {
                let printed = card.vars().len();
                if printed > VARS {
                    wide += 1;
                    assert!(
                        wide <= 1,
                        "{build} holds {VARS} a copy, and {card:?} prints {printed}",
                    );
                }
            }
            assert_eq!(wide, 1, "{build} prints one card wider than {VARS}");
        }
        wide::<sts2_content::v0_107_1::V0_107_1>("v0.107.1");
        wide::<sts2_content::v0_111_0::V0_111_0>("v0.111.0");
    }
}

#[cfg(test)]
impl Live<sts2_content::v0_107_1::V0_107_1> {
    /// `creatures` as a fight with nothing else in it.
    pub(crate) fn of(
        creatures: impl IntoIterator<Item = Fighter<sts2_content::v0_107_1::V0_107_1>>,
    ) -> Self {
        use sts2_content::v0_107_1::{
            ascension::AscensionLevel,
            characters::Character,
            encounters::Encounter,
            runstate::{CombatSide, PlayerTurnPhase},
        };

        Live {
            character: Character::Regent,
            ascension: AscensionLevel::None,
            turn: 1,
            phase: PlayerTurnPhase::Play,
            energy: 3,
            stars: 0,
            round: 1,
            side: CombatSide::Player,
            history: CombatHistory::new(),
            encounter: Encounter::SlimesWeak,
            cards: super::cards::Cards::with_cards(0),
            removed_from_deck: Vec::new(),
            returned: Vec::new(),
            leaving: Vec::new(),
            creatures: creatures.into_iter().collect(),
            powers: PowerStore::new(),
            orbs: None,
            machines: Bounded::new(),
            skip_tick: Bounded::new(),
            card_ids: CardIds::new(),
            attacks: 0,
            asking: None,
        }
    }
}

#[cfg(test)]
mod sizes {
    use super::*;

    /// The widths the comments above claim, so that a change has to restate
    /// them rather than quietly widen a fight.
    ///
    /// One build's content types, because a size needs a concrete `V`. The
    /// numbers move when a build's own enums do, which is the point.
    #[test]
    fn a_fight_holds_what_it_says_it_holds() {
        use sts2_content::v0_107_1::V0_107_1 as V;
        assert_eq!(size_of::<Landing>(), 4);
        assert_eq!(size_of::<Option<Landing>>(), 4, "the niche carries the tag");
        assert_eq!(size_of::<Intended<V>>(), 6);
        assert_eq!(size_of::<Telegraphed<V>>(), 14);
        assert_eq!(
            size_of::<Option<Telegraphed<V>>>(),
            14,
            "the move name carries the tag"
        );
        assert_eq!(size_of::<Stacked>(), 2);
        assert_eq!(size_of::<Fighter<V>>(), 36);
    }
}
