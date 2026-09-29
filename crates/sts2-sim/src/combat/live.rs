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
const POWERS: usize = 24;

/// How many creatures a fight holds, summons included.
///
/// The most slots an encounter has is six. Beside them stand the player,
/// Osty and a Byrdpip.
pub(crate) const CREATURES: usize = 10;

/// How many powers a fight holds that skip their next tick.
const SKIPS: usize = 32;

/// How many differences one copy of a card holds.
///
/// Five, which covers every card this build prints but the nine Mad
/// Sciences, at eleven each. Only an upgrade writes on one, and it writes a
/// keyword. `only_mad_science_prints_more_than_a_copy_holds` is what keeps
/// that true.
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
    /// The cards of one pile a selection's filter lets through.
    Among {
        cards: smallvec::SmallVec<[sts2_core::CardAt<V>; sts2_core::MAX_CARDS_IN_HAND]>,
        selection: sts2_core::Selection,
    },
    Offered {
        offered: sts2_core::Offered<V>,
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
    /// `outermost` is a play no other is running inside.
    Play {
        effect: Resuming<V>,
        at: V::Resume,
        outermost: bool,
    },
    /// `SetupPlayerTurn`, which a hook parked on its selection. `act` is what
    /// happens to the cards picked, and `stage` is the rest of the setup.
    Setup {
        act: sts2_core::PileAct,
        stage: Setup<V>,
    },
    /// The enemy turn, which a monster's move parked on a choice.
    EnemyTurn(EnemyTurn<V>),
    /// A selection a shuffle asked for inside an effect's draw. `act` is
    /// what happens to the cards picked, and the rest of the draw waits in
    /// [`Live::deferred`].
    Pick { act: sts2_core::PileAct },
}

/// What waits behind a question raised inside an effect, a move or an
/// attack's hit. The game's commands are coroutines, so what surrounds the
/// question stands still until it is answered. These run once it is.
///
/// An effect that does not wait runs on to its end first. So one whose code
/// goes on past the question does that part early.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Deferred<V: Rules> {
    /// The rest of an effect, from where its own code handed back `played`.
    Resume {
        effect: Resuming<V>,
        played: sts2_core::Played<V>,
        outermost: bool,
    },
    /// The end of an effect whose last play is over, which puts the card
    /// where its play sends it.
    Finish {
        effect: Resuming<V>,
        outermost: bool,
    },
    /// `CardCmd.AutoPlay` of a copy, which came up while the question
    /// waited. `named` is the target it was given.
    AutoPlay {
        id: CardId<V>,
        named: Option<CombatId>,
    },
    /// The move `move_id` of `actor`, which stood still on a command the
    /// player had to answer. It goes on from `at`, where it handed a resume
    /// point back, and is over otherwise. Then `OnMovePerformed`, the enemies
    /// in `acting`, and the end of the turn. `damage` and `repeats` are what
    /// the move advertised, which its attacks read.
    Move {
        actor: CombatId,
        monster: V::Monster,
        move_id: V::MoveId,
        at: Option<V::Resume>,
        damage: V::Number,
        repeats: V::Count,
        acting: smallvec::SmallVec<[CombatId; CREATURES]>,
        max_energy: V::Count,
    },
    /// `CardPileCmd.Draw` of `count` cards. `resumed` for the rest of a draw
    /// a shuffle's question stopped, which asked `ShouldDraw` already.
    Draw {
        count: usize,
        hand_draw: bool,
        resumed: bool,
    },
    /// The hits of an attack that came up while a question one of its
    /// earlier hits raised waited, each resolved as it lands. Then the end of
    /// the attack, over every hit in `landed` and the rest.
    ///
    /// `AttackCommand`'s hit loop awaits each hit, so the later ones stand
    /// still until the answer comes.
    Hits {
        frame: super::board::Frame<V>,
        attack: HeldAttack<V>,
        hits: Vec<sts2_core::Effect<V>>,
        landed: sts2_core::AttackResults<V>,
    },
}

/// The attack command whose hits wait, which `AfterAttack` names once they
/// have landed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct HeldAttack<V: Rules> {
    pub(super) command: sts2_core::AttackId,
    pub(super) attacker: CombatId,
    /// The monster swinging, which `History.CreatureAttacked` counts.
    pub(super) monster: Option<V::Monster>,
    /// `IsPoweredAttack`.
    pub(super) powered: bool,
}

/// Where the enemy turn stands while a monster's move waits on the player.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct EnemyTurn<V: Rules> {
    /// The monster whose move asked.
    pub(super) actor: CombatId,
    pub(super) move_id: V::MoveId,
    /// Where the move goes on from, which `V::resume_play` picks up.
    pub(super) at: V::Resume,
    /// The enemies still to act after it, in turn order.
    pub(super) acting: smallvec::SmallVec<[CombatId; CREATURES]>,
    /// The energy the player's next turn resets to.
    pub(super) max_energy: V::Count,
}

/// Where `SetupPlayerTurn` stands, with what the hook it stopped in has still
/// to carry out.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Setup<V: Rules> {
    /// Inside `BeforeHandDraw`, before the hand is drawn.
    BeforeHandDraw(super::listen::Answers<V>),
    /// Drawing the hand, with this many cards still to draw. `resumed` for a
    /// draw that a shuffle's question paused.
    Drawing { left: usize, resumed: bool },
    /// Inside `AfterPlayerTurnStart`, the last of the setup.
    PlayerTurnStart(super::listen::Answers<V>),
    /// Drawing for a listener inside `AfterPlayerTurnStart`, with this many
    /// cards still to draw and the listeners after it still to run.
    /// `resumed` for a draw that a shuffle's question paused.
    TurnStartDrawing {
        left: usize,
        resumed: bool,
        rest: super::listen::Answers<V>,
    },
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
    /// `Absolute` with `IsReduceOnly`: the lower of the cost so far and this.
    Lower(N),
}

/// One entry of a card's cost modifier list, in two bytes.
///
/// `LocalCostModifier`.
///
/// Sixteen bits say all of it: which pool, which expiration, whether it sets
/// the cost or shifts it, and by how much.
#[bitfield(u16, new = false, default = false, debug = false, order = Lsb)]
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
    /// Raw, because the same eight bits carry a set and a shift. A set is a
    /// price and reads unsigned. A shift is signed and carries
    /// [`SHIFT_BIAS`], because eight bits cannot be both.
    #[bits(8)]
    amount: u8,
    /// `IsReduceOnly`, which only a set here carries.
    reduce_only: bool,
    #[bits(3)]
    __: u8,
}

/// What a shift carries so that it shares the bits a set uses.
///
/// A set is a price, which is never negative, so its eight bits run
/// `0..=255`. A shift runs `-128..=127` over the same eight, and this is the
/// offset between them. `BansheesCry` shifts its cost down by its `Energy`
/// for each Ethereal play of the fight, which runs past four bits.
const SHIFT_BIAS: i16 = 128;

/// The most a modifier sets a cost to, which is eight bits of price.
pub(super) const SET_AT_MOST: i16 = 255;
/// How far a modifier shifts a cost, which is the same eight bits, biased.
pub(super) const SHIFT_WITHIN: std::ops::RangeInclusive<i16> = -SHIFT_BIAS..=SHIFT_BIAS - 1;

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
        let fits = |asked: N, within: std::ops::RangeInclusive<i16>| {
            num_traits::NumCast::from(asked)
                .filter(|amount| within.contains(amount))
                .unwrap_or_else(|| {
                    unimplemented!("a card cost of {asked:?}, and a modifier holds {within:?}")
                })
        };
        let (absolute, amount) = match change {
            CostChange::Relative(shift) => (false, fits(shift, SHIFT_WITHIN) + SHIFT_BIAS),
            CostChange::Absolute(set) | CostChange::Lower(set) => {
                (true, fits(set, 0..=SET_AT_MOST))
            }
        };
        Self::from_bits(0)
            .with_star(pool == sts2_core::CostPool::Star)
            .with_absolute(absolute)
            .with_reduce_only(matches!(change, CostChange::Lower(_)))
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

    fn change(self) -> CostChange<i16> {
        let amount = i16::from(self.amount());
        match (self.absolute(), self.reduce_only()) {
            (true, false) => CostChange::Absolute(amount),
            (true, true) => CostChange::Lower(amount),
            (false, _) => CostChange::Relative(amount - SHIFT_BIAS),
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
    /// Keywords `RemoveKeyword` took off, whether the table prints them or
    /// they were written on.
    removed_keywords: V::CardKeywordSet,
    /// `HasSingleTurnSly`: words the copy carries until the turn ends,
    /// whatever `RemoveKeyword` took off.
    turn_keywords: V::CardKeywordSet,
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
            removed_keywords: sts2_core::FlagSet::from_members(&[]),
            turn_keywords: sts2_core::FlagSet::from_members(&[]),
            replays: 0,
            enchantment_status: sts2_core::EnchantmentStatus::Normal,
        };
        if card.ident.upgrade != 0 {
            assert_eq!(card.ident.upgrade, 1, "only first upgrades are ported");
            // The level is already on the identity, so this writes what the
            // upgrade writes without counting it again.
            for &change in V::card_upgrade(card.ident.card) {
                card.upgrade_change(change);
            }
        }
        if let Some(enchanted) = card.ident.enchantment {
            for &change in V::enchanted_changes(enchanted.enchantment) {
                card.modify(change, sts2_core::Lasting::ThisFight);
            }
        }
        card
    }

    /// `EnergyCost.GetWithModifiers` and its star twin, as the pair a play
    /// reads.
    ///
    /// The modifiers fold over the printed cost in the order they were
    /// written, and a cost that is not a number folds nothing. `global` is
    /// `Hook.ModifyEnergyCostInCombat`, run over the energy cost after those
    /// and before the clamp. It leaves a cost below zero alone, so this does
    /// not ask it one.
    pub(super) fn costs(
        &self,
        global: impl FnOnce(V::Number) -> V::Number,
    ) -> (sts2_core::Cost, sts2_core::Cost) {
        let mut global = Some(global);
        let mut fold = |pool, printed| {
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
                    let number = |amount: i16| -> V::Number {
                        num_traits::NumCast::from(amount).expect("a cost change fits a game number")
                    };
                    match held.change() {
                        CostChange::Relative(amount) => cost + number(amount),
                        CostChange::Absolute(amount) => number(amount),
                        CostChange::Lower(amount) => cost.min(number(amount)),
                    }
                });
            let folded = match (pool, global.take()) {
                (sts2_core::CostPool::Energy, Some(global)) if folded >= num_traits::zero() => {
                    global(folded)
                }
                _ => folded,
            };
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
            sts2_core::Change::LowerCost(pool, to) => {
                self.modify_cost(pool, CostChange::Lower(to), lasting)
            }
            // `Keywords` is one set the copy owns, which both of these write.
            // A word already on the copy is not added twice.
            sts2_core::Change::AddKeyword(keyword) if lasting == sts2_core::Lasting::ThisTurn => {
                self.turn_keywords = sts2_core::FlagSet::with(self.turn_keywords, keyword);
            }
            sts2_core::Change::AddKeyword(keyword) => {
                assert!(
                    lasting == sts2_core::Lasting::ThisFight,
                    "a keyword written until played"
                );
                self.added_keywords = sts2_core::FlagSet::with(self.added_keywords, keyword);
                self.removed_keywords = sts2_core::FlagSet::without(self.removed_keywords, keyword);
            }
            sts2_core::Change::RemoveKeyword(keyword) => {
                self.added_keywords = sts2_core::FlagSet::without(self.added_keywords, keyword);
                self.removed_keywords = sts2_core::FlagSet::with(self.removed_keywords, keyword);
            }
        }
    }

    /// `Keywords.Contains(keyword)` for this copy: what the table prints, as
    /// `printed` says, and what has been written on, less what was taken off.
    /// Then what a power gives it for the affliction it carries, which
    /// `TryModifyKeywordsInCombat` adds after the rest.
    fn keyword(&self, printed: bool, keyword: V::CardKeyword) -> bool {
        (printed || sts2_core::FlagSet::contains(self.added_keywords, keyword))
            && !sts2_core::FlagSet::contains(self.removed_keywords, keyword)
            || sts2_core::FlagSet::contains(self.turn_keywords, keyword)
            || self
                .ident
                .affliction
                .and_then(V::keyword_while_afflicted)
                .is_some_and(|granted| granted == keyword)
    }

    /// `Keywords.Contains(keyword)` for this copy, for the words the table
    /// answers for.
    ///
    /// # Panics
    ///
    /// A word the table does not answer for.
    pub(super) fn has_keyword(&self, keyword: V::CardKeyword) -> bool {
        use sts2_core::{CardTable as _, KeywordKinds};

        let card = self.ident.card;
        let printed = match keyword {
            k if k == <V::CardKeyword as KeywordKinds<V>>::INNATE => card.innate(),
            k if k == <V::CardKeyword as KeywordKinds<V>>::RETAIN => card.retains(),
            k if k == <V::CardKeyword as KeywordKinds<V>>::SLY => card.sly(),
            k if k == <V::CardKeyword as KeywordKinds<V>>::ETHEREAL => card.ethereal(),
            k if k == <V::CardKeyword as KeywordKinds<V>>::EXHAUST => card.exhausts(),
            k if k == <V::CardKeyword as KeywordKinds<V>>::UNPLAYABLE => card.unplayable(),
            keyword => unimplemented!("whether {card:?} prints {keyword:?}"),
        };
        self.keyword(printed, keyword)
    }

    /// `Keywords.Contains(Exhaust)` for this copy, which an upgrade can take
    /// off.
    pub(super) fn exhausts(&self) -> bool {
        use sts2_core::{CardTable as _, KeywordKinds};

        self.keyword(
            self.ident.card.exhausts(),
            <V::CardKeyword as KeywordKinds<V>>::EXHAUST,
        )
    }

    /// `Keywords.Contains(Innate)` for this copy.
    ///
    /// Asked of the keywords something in a fight writes or takes off, and
    /// reads. The rest are read off the table.
    pub(super) fn innate(&self) -> bool {
        use sts2_core::{CardTable as _, KeywordKinds};

        self.keyword(
            self.ident.card.innate(),
            <V::CardKeyword as KeywordKinds<V>>::INNATE,
        )
    }

    /// `Keywords.Contains(Ethereal)` for this copy, the way [`Face::innate`]
    /// reads Innate.
    pub(super) fn ethereal(&self) -> bool {
        use sts2_core::{CardTable as _, KeywordKinds};

        self.keyword(
            self.ident.card.ethereal(),
            <V::CardKeyword as KeywordKinds<V>>::ETHEREAL,
        )
    }

    /// `IsSlyThisTurn` for this copy, the keyword the way [`Face::innate`]
    /// reads Innate, or Sly written on for the turn.
    pub(super) fn sly(&self) -> bool {
        use sts2_core::{CardTable as _, KeywordKinds};

        self.keyword(
            self.ident.card.sly(),
            <V::CardKeyword as KeywordKinds<V>>::SLY,
        )
    }

    /// `Keywords.Contains(Retain)` for this copy, the way [`Face::innate`]
    /// reads Innate.
    pub(super) fn retains(&self) -> bool {
        use sts2_core::{CardTable as _, KeywordKinds};

        self.keyword(
            self.ident.card.retains(),
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
        let held = CostModifier::held(pool, change, lasting);
        // `LocalCostModifier.Modify` returns a set's amount whatever the cost
        // before it, and one that lasts the fight never leaves the list. So
        // nothing reads an energy modifier ahead of it again. Stars differ:
        // `GetStarCostThisCombat` reads the first such cost, not the last.
        if pool == sts2_core::CostPool::Energy
            && matches!(held.change(), CostChange::Absolute(_))
            && !lasting.clears_when_turn_ends()
            && !lasting.clears_when_played()
        {
            self.kept(|modifier| modifier.pool() == sts2_core::CostPool::Energy);
        }
        // A set on the energy pool the copy already carries, alike in every
        // part, adds nothing the fold reads: the fold lands on the last set,
        // and the two wear off together. So the older goes, and the newer
        // lands last. Rocket Punch writes one for each Status its owner makes.
        if pool == sts2_core::CostPool::Energy && matches!(held.change(), CostChange::Absolute(_)) {
            let count = usize::from(self.modifiers);
            if let Some(found) = self.cost_modifiers[..count]
                .iter()
                .position(|&modifier| modifier == held)
            {
                self.cost_modifiers.copy_within(found + 1..count, found);
                self.modifiers -= 1;
            }
        }
        let at = usize::from(self.modifiers);
        // A shift right after another on the same pool, which wears off when
        // it does, is one shift of their sum. The fold clamps nothing between
        // them, and `UpgradeBy` lowers sets alone.
        if let Some(&last) = at.checked_sub(1).map(|last| &self.cost_modifiers[last])
            && let (CostChange::Relative(before), CostChange::Relative(more)) =
                (last.change(), held.change())
            && last.pool() == held.pool()
            && last.clears_when_turn_ends() == held.clears_when_turn_ends()
            && last.clears_when_played() == held.clears_when_played()
            && SHIFT_WITHIN.contains(&(before + more))
        {
            self.cost_modifiers[at - 1] =
                CostModifier::held(pool, CostChange::Relative(before + more), lasting);
            return;
        }
        if at == MODIFIERS {
            unimplemented!("a card carrying more than {MODIFIERS} cost modifiers");
        }
        self.cost_modifiers[at] = held;
        self.modifiers += 1;
    }

    /// Drop the cost modifiers and keywords that do not outlive this turn.
    pub(super) fn turn_ended(&mut self) {
        self.kept(CostModifier::clears_when_turn_ends);
        self.turn_keywords = sts2_core::FlagSet::from_members(&[]);
    }

    /// Whether this copy holds a cost modifier or a keyword that ends with
    /// the turn.
    pub(super) fn lasts_the_turn(&self) -> bool {
        self.turn_keywords != sts2_core::FlagSet::from_members(&[])
            || self.cost_modifiers[..usize::from(self.modifiers)]
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
                self.enchantment_status,
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

    /// One of what an upgrade writes. A price change moves the printed cost,
    /// the rest are written for the fight.
    fn upgrade_change(&mut self, change: sts2_core::Change<V>) {
        match change {
            sts2_core::Change::ChangeCost(pool, by) => self.upgrade_cost(pool, by),
            change => self.modify(change, sts2_core::Lasting::ThisFight),
        }
    }

    /// `CardEnergyCost.UpgradeBy` and `CardModel.UpgradeStarCostBy`: the
    /// printed cost itself moves, which a downgrade puts back.
    ///
    /// An energy cost stops at nothing. A lower one also lowers every set
    /// above it, and a lower star cost drops the star sets above it.
    fn upgrade_cost(&mut self, pool: sts2_core::CostPool, by: V::Number) {
        let printed = match pool {
            sts2_core::CostPool::Energy => &mut self.cost,
            sts2_core::CostPool::Star => &mut self.star_cost,
        };
        let sts2_core::Cost::Fixed(was) = *printed else {
            return;
        };
        let by: i16 = num_traits::cast(by)
            .unwrap_or_else(|| unimplemented!("an upgrade moving a cost by {by:?}"));
        let was = i16::from(was.get());
        let now = match pool {
            sts2_core::CostPool::Energy => (was + by).max(0),
            sts2_core::CostPool::Star if was + by < 0 => {
                unimplemented!("an upgrade taking a star cost below nothing")
            }
            sts2_core::CostPool::Star => was + by,
        };
        *printed = sts2_core::Cost::Fixed(
            u8::try_from(now)
                .ok()
                .and_then(nonmax::NonMaxU8::new)
                .unwrap_or_else(|| unimplemented!("an upgraded cost of {now}")),
        );
        if now >= was {
            return;
        }
        let above = |held: CostModifier| {
            held.pool() == pool && matches!(held.change(), CostChange::Absolute(set) if set > now)
        };
        let mut to = 0;
        for at in 0..usize::from(self.modifiers) {
            let held = self.cost_modifiers[at];
            match pool {
                sts2_core::CostPool::Energy if above(held) => {
                    self.cost_modifiers[to] = held.with_amount(now as u8);
                    to += 1;
                }
                sts2_core::CostPool::Star if above(held) => {}
                _ => {
                    self.cost_modifiers[to] = held;
                    to += 1;
                }
            }
        }
        self.modifiers = to as u8;
    }

    /// `CardModel.DowngradeInternal`: the level to nothing, and the printed
    /// numbers, prices and keywords back, which drops what the fight wrote on
    /// them. The cost modifiers stay, and the enchantment writes again.
    pub(super) fn downgrade(&mut self) {
        let mut fresh = Self::fresh(CardIdent {
            upgrade: 0,
            ..self.ident
        });
        fresh.cost_modifiers = self.cost_modifiers;
        fresh.modifiers = self.modifiers;
        fresh.replays = self.replays;
        fresh.enchantment_status = self.enchantment_status;
        *self = fresh;
    }

    pub(super) fn upgrade(&mut self) {
        // `CardModel.IsUpgradable`, which a card already at its own maximum is
        // not. A status card prints no upgrade at all.
        if self.ident.upgrade >= self.ident.card.max_upgrade() {
            return;
        }
        for &change in V::card_upgrade(self.ident.card) {
            self.upgrade_change(change);
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

    fn block(&self) -> V::Number {
        self.fighter.block
    }

    fn owner(&self) -> Option<CombatId> {
        self.fighter.owner
    }

    fn monster(&self) -> Option<V::Monster> {
        match self.fighter.model {
            Some(sts2_core::ModelId::Monster(monster)) => Some(monster),
            _ => None,
        }
    }

    fn power(&self, power: V::Power) -> Option<sts2_core::PowerAmount<V::Number>> {
        self.powers
            .of(self.fighter.id)
            .find(|held| held.power() == power)
            .map(|held| held.amount.amount())
    }

    fn intents(&self) -> impl Iterator<Item = V::IntentKind> + '_ {
        self.fighter
            .next_move
            .iter()
            .flat_map(|next| next.intents().map(|intent| intent.kind))
    }
}

/// The fight's creatures and their powers, as a listener reads ones it does
/// not own. Borrowed field by field, so a caller holding another part of the
/// fight mutably can still lend it.
pub(crate) struct Onlookers<'a, V: Rules> {
    pub(crate) creatures: &'a Creatures<V>,
    pub(crate) powers: &'a PowerStore<V>,
    pub(crate) kept: &'a [KeptBeside<V>],
    /// The belt, where the caller has one to lend. A relic asked about with
    /// none lent panics by name.
    pub(crate) relics: Option<&'a crate::run::Belt<V>>,
}

impl<V: Rules> sts2_core::FightView<V> for Onlookers<'_, V> {
    fn kept(&self, holder: CombatId, power: V::Power) -> Option<&sts2_core::Kept<V>> {
        self.kept
            .iter()
            .find(|(at, kept, _)| *at == holder && *kept == power)
            .map(|(_, _, kept)| &**kept)
    }

    fn power(
        &self,
        creature: CombatId,
        power: V::Power,
    ) -> Option<sts2_core::PowerAmount<V::Number>> {
        self.powers
            .of(creature)
            .find(|held| held.power() == power)
            .map(|held| held.amount.amount())
    }

    fn pet_owner(&self, creature: CombatId) -> Option<CombatId> {
        self.creatures
            .iter()
            .find(|fighter| fighter.id == creature)
            .and_then(|fighter| fighter.owner)
    }

    fn monster(&self, creature: CombatId) -> Option<V::Monster> {
        self.creatures
            .iter()
            .find(|fighter| fighter.id == creature)
            .and_then(|fighter| fighter.carrier().monster)
    }

    fn current_hp(&self, creature: CombatId) -> Option<V::Health> {
        self.creatures
            .iter()
            .find(|fighter| fighter.id == creature)
            .map(|fighter| fighter.hp)
    }

    /// The other side's creatures, a pet among them.
    fn player(&self) -> CombatId {
        super::board::player::<V>(self.creatures)
    }

    fn living_opponents(&self, creature: CombatId) -> smallvec::SmallVec<[CombatId; 8]> {
        let Some(side) = self
            .creatures
            .iter()
            .find(|fighter| fighter.id == creature)
            .map(|fighter| fighter.side)
        else {
            return smallvec::SmallVec::new();
        };
        self.creatures
            .iter()
            .filter(|fighter| fighter.side != side && fighter.alive)
            .map(|fighter| fighter.id)
            .collect()
    }

    /// The belt is the one player's, and every creature that is no monster
    /// is that player.
    fn relic(&self, creature: CombatId, relic: V::Relic) -> bool {
        let player = self
            .creatures
            .iter()
            .any(|fighter| fighter.id == creature && fighter.carrier().monster.is_none());
        let relics = self
            .relics
            .unwrap_or_else(|| unimplemented!("{relic:?} asked about where no belt is lent"));
        player && relics.iter().any(|held| held.relic == relic)
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
/// The Infested Prism's Pulsate draws three: an attack, a buff and a block.
/// That is the most the table holds. [`sts2_core::NextMove`] inlines to the
/// same number.
const INTENTS: usize = 3;

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
        fight: &'a Onlookers<'a, V>,
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
            fight,
        }
    }

    ///
    /// `state` is a copy of this power's own, so the fight stays readable
    /// while the hook writes on it. [`Live::ask_power`] keeps what it wrote.
    pub(crate) fn event_power<'a>(
        &self,
        state: &'a mut V::PowerState,
        carrier: Carrier<V>,
        history: sts2_core::History<'a, V>,
        fight: &'a dyn sts2_core::FightView<V>,
    ) -> sts2_core::EventPower<'a, V> {
        sts2_core::EventPower {
            power: self.power(),
            amount: self.amount.amount(),
            owner: carrier.side,
            holder: carrier.id,
            applier: self.applier,
            monster: carrier.monster,
            on_turn_start: self.on_turn_start.amount(),
            state,
            history,
            fight,
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
    /// `EncounterModel.SpawnedEnemies`, as how many: every enemy that joined
    /// the fight.
    spawned_enemies: u8,
    /// `CombatState.EscapedCreatures`, as how many.
    escaped: u8,
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
            spawned_enemies: 0,
            escaped: 0,
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
        self.joined(&fighter);
        self.list.push(fighter);
    }

    pub(crate) fn insert(&mut self, at: usize, fighter: Fighter<V>) {
        self.joined(&fighter);
        self.list.insert(at, fighter);
    }

    /// The counts a creature joining the fight moves.
    fn joined(&mut self, fighter: &Fighter<V>) {
        self.counts.add(Self::overrides(fighter));
        if fighter.side == <V::CombatSide as SideKinds>::ENEMY {
            self.spawned_enemies += 1;
        }
    }

    /// `CombatState.CreatureEscaped`: the creature leaves, and counts as
    /// escaped.
    pub(crate) fn escape(&mut self, id: CombatId) {
        self.retain(|creature| creature.id != id);
        self.escaped += 1;
    }

    /// `EncounterModel.CalculateGoldProportion`: the share of the enemies
    /// that joined the fight and did not escape.
    pub(crate) fn gold_proportion(&self) -> f32 {
        1.0 - f32::from(self.escaped) / f32::from(self.spawned_enemies)
    }

    /// Whether any enemy escaped the fight.
    pub(crate) fn any_escaped(&self) -> bool {
        self.escaped > 0
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
        self.spawned_enemies = source.spawned_enemies;
        self.escaped = source.escaped;
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
    /// The player's `Gold`, which the fight holds while it runs. The run
    /// takes it back after each step.
    pub(super) gold: V::Number,
    pub(super) round: V::Count,
    /// `ICombatState.CurrentSide`: whose turn it is.
    pub(super) side: V::CombatSide,
    /// `CombatManager.History`.
    pub(super) history: CombatHistory<V>,
    /// What each power that keeps anything beside its model keeps, by the
    /// creature carrying it. Empty in nearly every fight.
    pub(super) kept: Vec<KeptBeside<V>>,
    /// The encounter this active fight belongs to.
    pub(super) encounter: V::Encounter,
    pub(super) cards: super::cards::Cards<V>,
    /// Deck cards a combat command removed permanently while this fight ran.
    pub(super) removed_from_deck: Vec<CardIdent<V>>,
    /// `CombatRoom.ExtraRewards`' cards, which a won fight offers as
    /// `SpecialCardReward`s. An event adds its own as the fight opens, and a
    /// listener hands one back as it dies.
    pub(super) extra_cards: Vec<CardIdent<V>>,
    /// The copies whose `ExhaustOnNextPlay` is set, which their next play
    /// clears and so does the end of the turn.
    pub(super) exhaust_next: smallvec::SmallVec<[CardId<V>; 2]>,
    /// What `AfterCombatEnd` asked of the run, which carries it out as it
    /// takes the fight's rewards.
    pub(super) leaving: Vec<sts2_core::HookEffect<V>>,
    /// What the fight asked of the run in the middle of a decision, which the
    /// run carries out as the decision settles.
    pub(super) for_run: Vec<sts2_core::HookEffect<V>>,
    /// What `AfterShuffle` listeners asked for behind one that asked the
    /// player to pick, which waits for the pick.
    pub(super) after_pick: Vec<sts2_core::HookEffect<V>>,
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
    /// Clones of cards the fight keeps outside every pile, by the id they
    /// were minted under, which a power's model names.
    pub(super) kept_faces: Vec<(CardId<V>, Face<V>)>,
    /// How many attack commands this fight has started, which is what the
    /// next one is told apart by.
    pub(super) attacks: u32,
    /// The last creature id this fight gave out. A creature that leaves keeps
    /// its id, and the next one to arrive takes the id after this.
    pub(super) last_creature: CombatId,
    /// Numbers a card keeps on its own model and prints nowhere, by copy,
    /// such as Regret's `CardsInHand`.
    pub(super) notes: smallvec::SmallVec<[(CardId<V>, V::Number); 2]>,
    /// A card play waiting on an answer, which is the only thing the fight
    /// will take while it is set.
    pub(super) asking: Option<Asking<V>>,
    /// What waits behind that answer, the next to run last.
    pub(super) deferred: Vec<Deferred<V>>,
    /// Where what an effect defers goes in [`Live::deferred`], once the
    /// question it waits behind is asked. Each one goes under the last, so
    /// the first deferred runs first.
    pub(super) deferring_at: Option<usize>,
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
    pub(super) fn start(&mut self, card: V::Card, id: CardId<V>, energy: V::Count, first: bool) {
        let started = sts2_core::PlayStarted {
            card,
            id,
            energy,
            first,
        };
        self.this_turn
            .push(sts2_core::HistoryEntry::Started(started));
    }

    /// `History.CardPlayFinished`, in the turn the fight stands in.
    pub(super) fn finish(&mut self, card: V::Card, id: CardId<V>, ethereal: bool) {
        let finished = sts2_core::PlayFinished { card, id, ethereal };
        self.this_turn
            .push(sts2_core::HistoryEntry::Finished(finished));
    }

    /// `History.CreatureAttacked`, in the turn the fight stands in.
    pub(super) fn attacked(&mut self, monster: Option<V::Monster>) {
        self.this_turn
            .push(sts2_core::HistoryEntry::Attacked(monster));
    }

    /// `History.CardAfflicted`, in the turn the fight stands in.
    pub(super) fn afflicted(&mut self, affliction: V::Affliction) {
        self.this_turn
            .push(sts2_core::HistoryEntry::Afflicted(affliction));
    }

    /// `History.CardDrawn`, in the turn the fight stands in. `hand_draw` is
    /// `fromHandDraw`.
    pub(super) fn draw(&mut self, card: V::Card, hand_draw: bool) {
        self.this_turn
            .push(sts2_core::HistoryEntry::Drawn { card, hand_draw });
    }

    /// `History.CardDiscarded`, in the turn the fight stands in.
    pub(super) fn discarded(&mut self, card: V::Card) {
        self.this_turn
            .push(sts2_core::HistoryEntry::Discarded(card));
    }

    /// `History.CardExhausted`, in the turn the fight stands in.
    pub(super) fn exhausted(&mut self, card: V::Card) {
        self.this_turn
            .push(sts2_core::HistoryEntry::Exhausted(card));
    }

    /// `History.CardGenerated`, in the turn the fight stands in, by whether
    /// the player made it.
    pub(super) fn generated(&mut self, by_player: bool) {
        self.this_turn
            .push(sts2_core::HistoryEntry::Generated { by_player });
    }

    /// `History.DamageReceived`, in the turn the fight stands in.
    pub(super) fn damage_received(&mut self, received: sts2_core::DamageReceived<V>) {
        self.this_turn
            .push(sts2_core::HistoryEntry::DamageReceived(received));
    }

    /// `History.BlockGained`, in the turn the fight stands in.
    pub(super) fn block_gained(&mut self, gained: sts2_core::BlockGained) {
        self.this_turn
            .push(sts2_core::HistoryEntry::BlockGained(gained));
    }

    /// `History.StarsModified`, in the turn the fight stands in.
    pub(super) fn stars_modified(&mut self, amount: V::Number) {
        self.this_turn
            .push(sts2_core::HistoryEntry::StarsModified(amount));
    }

    /// The play of `id` under way: the latest `CardPlayStartedEntry` of that
    /// copy this turn. A play runs inside one turn.
    pub(super) fn play_at(&self, id: CardId<V>) -> Option<sts2_core::PlayAt> {
        let at = self.this_turn.iter().rposition(
            |entry| matches!(entry, sts2_core::HistoryEntry::Started(started) if started.id == id),
        )?;
        Some(sts2_core::PlayAt(
            at.try_into().expect("a turn's entries fit a play's place"),
        ))
    }

    /// The turn at `round` on `side`, in the player's turn `player_turn`, is
    /// over. Its entries join the chain, unless it had none.
    fn end_turn(&mut self, round: V::Count, side: V::CombatSide, player_turn: V::Count) {
        if self.this_turn.is_empty() {
            return;
        }
        let turn = sts2_core::TurnHistory {
            round,
            side,
            player_turn,
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

impl<V: Rules> Live<V> {
    /// The id the next creature to arrive takes.
    ///
    /// # Panics
    ///
    /// A fight that has given out every id a creature can hold.
    pub(super) fn next_creature(&mut self) -> CombatId {
        let Some(next) = self
            .last_creature
            .get()
            .checked_add(1)
            .and_then(nonmax::NonMaxU8::new)
            .map(CombatId::new)
        else {
            panic!("combat has too many creatures to assign an id");
        };
        self.last_creature = next;
        next
    }
}

impl<V: Rules> Clone for Live<V> {
    fn clone(&self) -> Self {
        Self {
            cards: self.cards.clone(),
            removed_from_deck: self.removed_from_deck.clone(),
            extra_cards: self.extra_cards.clone(),
            exhaust_next: self.exhaust_next.clone(),
            history: self.history.clone(),
            kept: self.kept.clone(),
            leaving: self.leaving.clone(),
            for_run: self.for_run.clone(),
            after_pick: self.after_pick.clone(),
            powers: self.powers.clone(),
            orbs: self.orbs.clone(),
            card_ids: self.card_ids.clone(),
            kept_faces: self.kept_faces.clone(),
            notes: self.notes.clone(),
            asking: self.asking.clone(),
            deferred: self.deferred.clone(),
            deferring_at: self.deferring_at,
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
            gold,
            round,
            side,
            history,
            kept,
            encounter,
            cards,
            removed_from_deck,
            extra_cards,
            exhaust_next,
            leaving,
            for_run,
            after_pick,
            creatures,
            powers,
            orbs,
            machines,
            skip_tick,
            card_ids,
            kept_faces,
            attacks,
            last_creature,
            notes,
            asking,
            deferred,
            deferring_at,
        } = self;
        *character = source.character;
        *ascension = source.ascension;
        *turn = source.turn;
        *phase = source.phase;
        *energy = source.energy;
        *stars = source.stars;
        *gold = source.gold;
        *round = source.round;
        *side = source.side;
        history.clone_from(&source.history);
        kept.clone_from(&source.kept);
        *encounter = source.encounter;
        cards.clone_from(&source.cards);
        removed_from_deck.clone_from(&source.removed_from_deck);
        extra_cards.clone_from(&source.extra_cards);
        exhaust_next.clone_from(&source.exhaust_next);
        leaving.clone_from(&source.leaving);
        for_run.clone_from(&source.for_run);
        after_pick.clone_from(&source.after_pick);
        creatures.copy_from(&source.creatures);
        powers.clone_from(&source.powers);
        orbs.clone_from(&source.orbs);
        machines.copy_from(&source.machines);
        skip_tick.copy_from(&source.skip_tick);
        card_ids.clone_from(&source.card_ids);
        kept_faces.clone_from(&source.kept_faces);
        *attacks = source.attacks;
        *last_creature = source.last_creature;
        notes.clone_from(&source.notes);
        asking.clone_from(&source.asking);
        deferred.clone_from(&source.deferred);
        *deferring_at = source.deferring_at;
    }
}

/// What one power keeps beside its model: the creature carrying it, which
/// power, and the record, shared between the fights a search copies.
pub(crate) type KeptBeside<V> = (
    CombatId,
    <V as sts2_core::ContentVersion>::Power,
    std::sync::Arc<sts2_core::Kept<V>>,
);

impl<V: Rules> Live<V> {
    /// What the fight keeps beside the instance of `power` that `holder`
    /// carries, to write on, begun empty where it keeps nothing yet.
    pub(super) fn kept_mut(
        &mut self,
        holder: CombatId,
        power: V::Power,
    ) -> &mut sts2_core::Kept<V> {
        let at = match self
            .kept
            .iter()
            .position(|(at, kept, _)| *at == holder && *kept == power)
        {
            Some(at) => at,
            None => {
                self.kept.push((holder, power, std::sync::Arc::default()));
                self.kept.len() - 1
            }
        };
        std::sync::Arc::make_mut(&mut self.kept[at].2)
    }

    /// Keep `note` on the copy `id`'s own model, over what it kept before.
    pub(super) fn note(&mut self, id: CardId<V>, note: V::Number) {
        match self.notes.iter_mut().find(|(noted, _)| *noted == id) {
            Some((_, kept)) => *kept = note,
            None => self.notes.push((id, note)),
        }
    }

    /// What the copy `id` keeps on its own model, zero for nothing kept.
    pub(super) fn noted(&self, id: CardId<V>) -> V::Number {
        self.notes
            .iter()
            .find(|(noted, _)| *noted == id)
            .map_or_else(num_traits::zero, |&(_, kept)| kept)
    }
}

impl<V: Rules> Live<V> {
    /// `GainBlockInternal` and `History.BlockGained` for `amount` of
    /// unpowered block on `owner`, which no play gives. Nothing for a
    /// creature gone from the fight, or for no block.
    pub(super) fn gain_unpowered_block(&mut self, owner: CombatId, amount: V::Number) {
        let Some(creature) = self.creatures.iter_mut().find(|c| c.id == owner) else {
            return;
        };
        if amount > num_traits::zero() {
            creature.block = creature.block + amount;
            self.history.block_gained(sts2_core::BlockGained {
                receiver: owner,
                powered: false,
                play: None,
            });
        }
    }

    /// `PlayerCombatState.Stars` moved by `amount`. The setter writes a
    /// `StarsModifiedEntry` for any change.
    pub(super) fn modify_stars(&mut self, amount: V::Number) {
        let count = |amount: V::Number| -> V::Count {
            num_traits::cast(amount).unwrap_or_else(|| unimplemented!("{amount:?} stars"))
        };
        match amount < num_traits::zero() {
            true => self.stars = self.stars - count(-amount),
            false => self.stars = self.stars + count(amount),
        }
        if amount != num_traits::zero() {
            self.history.stars_modified(amount);
        }
    }

    /// The player's `Gold` as the fight left it.
    pub(crate) fn gold(&self) -> V::Number {
        self.gold
    }

    /// `CombatRoom.GoldProportion`, which `CalculateGoldProportion` sets as
    /// the fight ends.
    pub(crate) fn gold_proportion(&self) -> f32 {
        self.creatures.gold_proportion()
    }

    /// Whether any enemy escaped, which is what the dummy's
    /// `RanOutOfTime` records.
    pub(crate) fn any_escaped(&self) -> bool {
        self.creatures.any_escaped()
    }

    pub(crate) fn removed_from_deck(&self) -> &[CardIdent<V>] {
        &self.removed_from_deck
    }

    /// `CombatRoom.AddExtraReward` of a special card, which an event does as
    /// it builds its fight's room.
    pub(crate) fn add_extra_card(&mut self, card: CardIdent<V>) {
        self.extra_cards.push(card);
    }

    /// The special cards this fight offers once it is won, in the order they
    /// were added.
    pub(crate) fn extra_cards(&self) -> &[CardIdent<V>] {
        &self.extra_cards
    }

    /// What the fight's end asked of the run, in the order it asked.
    pub(crate) fn leaving(&self) -> &[sts2_core::HookEffect<V>] {
        &self.leaving
    }

    /// Put `deferred` behind the question asked, under what else waits
    /// behind it.
    pub(super) fn defer(&mut self, deferred: Deferred<V>) {
        let at = *self.deferring_at.get_or_insert(self.deferred.len());
        self.deferred.insert(at, deferred);
    }

    /// Whether the fight asked anything of the run since the last decision
    /// settled.
    pub(crate) fn asked_for_run(&self) -> bool {
        !self.for_run.is_empty()
    }

    /// What the fight asked of the run since the last decision settled.
    pub(crate) fn take_for_run(&mut self) -> Vec<sts2_core::HookEffect<V>> {
        std::mem::take(&mut self.for_run)
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
            Asked::Among { cards, .. } => cards
                .iter()
                .map(|card| {
                    self.cards
                        .named(card.id())
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
        let bounds = |selection: sts2_core::Selection| match selection {
            sts2_core::Selection::Exactly(count) => {
                let count = u32::try_from(count.get()).expect("a selection count fits the wire");
                (count, count)
            }
            sts2_core::Selection::UpTo(count) => (
                0,
                u32::try_from(count.get()).expect("a selection count fits the wire"),
            ),
        };
        Some(match &asked.question {
            Asked::Pile { pile, selection } => {
                let (min, max) = bounds(*selection);
                (min, max, self.cards.len(*pile))
            }
            Asked::Among { cards, selection } => {
                let (min, max) = bounds(*selection);
                (min, max, cards.len())
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
        let Some(rolled) = fighter.next_move.as_ref() else {
            // `new MoveState()`: a monster that has not rolled yet shows
            // `UNSET_MOVE`.
            return matches!(fighter.model, Some(sts2_core::ModelId::Monster(_)))
                .then(|| Telegraphed::<V>::unset().telegraph());
        };
        let mut next = rolled.telegraph();
        let player = self
            .creatures
            .iter()
            .find(|creature| creature.side == <V::CombatSide as SideKinds>::PLAYER);
        if let Some(player) = player {
            for intent in &mut next.intents {
                if let Some(attack) = &mut intent.attack {
                    attack.damage = super::board::modified::<V>(
                        super::listen::Walk { live: self, relics },
                        Some(fighter.id),
                        Some(player.id),
                        None,
                        attack.damage,
                        // A move's attack is powered.
                        true,
                    );
                }
            }
        }
        Some(next)
    }

    /// The player, which every fight has one of.
    /// The orb queue as the game reports it, each number its getter's
    /// `PassiveVal` or `EvokeVal`. The queue itself, while nothing listens.
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
            let modify = |base| super::board::orb_value::<V>(walk(), held.orb, base);
            let passive = V::orb_passive_val(held, modify);
            held.evoke = V::orb_evoke_val(held, modify);
            held.passive = passive;
        }
        Some(std::borrow::Cow::Owned(reported))
    }

    /// A change a card writes on itself, wherever in the fight it stands.
    /// That includes the play pile, since a card hears the attack its own play
    /// makes.
    ///
    /// # Panics
    ///
    /// A card no combat pile holds, and that is not being played.
    pub(super) fn modify_self(
        &mut self,
        id: CardId<V>,
        change: sts2_core::Change<V>,
        lasting: sts2_core::Lasting,
    ) {
        let write = |held: &mut Face<V>| {
            held.modify(change, lasting);
            true
        };
        let found = [
            sts2_core::Pile::Hand,
            sts2_core::Pile::Draw,
            sts2_core::Pile::Discard,
            sts2_core::Pile::Exhaust,
        ]
        .into_iter()
        .find_map(|pile| self.cards.find(pile, id).map(|at| (pile, at)));
        match found {
            Some((pile, at)) => self.cards.write(pile, at, write),
            None => self.cards.write_in_play(id, write),
        }
    }

    /// `CombatManager.History`, as a listener reads it.
    pub(crate) fn history(&self) -> sts2_core::History<'_, V> {
        self.history.view()
    }

    /// `SwitchSides`: the turn now over joins the history, and `side` takes
    /// its turn.
    pub(super) fn switch_side(&mut self, side: V::CombatSide) {
        self.history.end_turn(self.round, self.side, self.turn);
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

    /// Ask the power at `at` among `owner`'s about `hook`, with the fight in
    /// view, and keep what its model saved. What it asks for goes on `asked`.
    pub(crate) fn ask_power(
        &mut self,
        relics: Option<&crate::run::Belt<V>>,
        owner: CombatId,
        at: usize,
        hook: sts2_core::EventHook<V>,
        asked: &mut sts2_core::HookEffects<V>,
    ) {
        let Some(carrier) = self
            .creatures
            .iter()
            .find(|c| c.id == owner)
            .map(|c| c.carrier())
        else {
            return;
        };
        self.ask_power_as(relics, carrier, at, hook, asked);
    }

    /// [`Live::ask_power`] for a caller that has the power's carrier already.
    pub(crate) fn ask_power_as(
        &mut self,
        relics: Option<&crate::run::Belt<V>>,
        carrier: Carrier<V>,
        at: usize,
        hook: sts2_core::EventHook<V>,
        asked: &mut sts2_core::HookEffects<V>,
    ) {
        let owner = carrier.id;
        let Some(&held) = self.powers.of(owner).nth(at) else {
            return;
        };
        let mut state = held.state;
        let fight = Onlookers {
            creatures: &self.creatures,
            powers: &self.powers,
            kept: &self.kept,
            relics,
        };
        V::power_event_hook(
            held.event_power(&mut state, carrier, self.history.view(), &fight),
            hook,
            asked,
        );
        if state != held.state
            && let Some(held) = self.powers.of_mut(owner).nth(at)
        {
            held.state = state;
        }
    }

    /// `PowerCmd.Remove`: `power` off the creature `owner`, if it is still
    /// there to take it off, and then its `AfterRemoved`.
    ///
    /// # Panics
    ///
    /// A power asking for something as it comes off that nothing here carries
    /// out, or taking block that breaks as it goes.
    pub(crate) fn remove_power(&mut self, owner: CombatId, power: V::Power) {
        if let Some(effect) = self.remove_power_asking(owner, power).first() {
            unimplemented!("{power:?} asking for {effect:?} as it comes off, outside a board")
        }
    }

    /// [`Live::remove_power`], handing back what `AfterRemoved` asked that
    /// only a board carries out.
    pub(super) fn remove_power_asking(
        &mut self,
        owner: CombatId,
        power: V::Power,
    ) -> sts2_core::HookEffects<V> {
        let mut left = sts2_core::HookEffects::new();
        let Some(carrier) = self
            .creatures
            .iter()
            .find(|c| c.id == owner)
            .map(|c| c.carrier())
        else {
            return left;
        };
        let Some(at) = self.powers.of(owner).position(|held| held.power() == power) else {
            return left;
        };
        let held = self.powers.remove(owner, at);
        let mut asked = sts2_core::HookEffects::new();
        // Taken off already, as `AfterRemoved` finds it. No belt is lent here.
        let mut state = held.state;
        let fight = Onlookers {
            creatures: &self.creatures,
            powers: &self.powers,
            kept: &self.kept,
            relics: None,
        };
        V::power_event_hook(
            held.event_power(&mut state, carrier, self.history.view(), &fight),
            sts2_core::EventHook::Removed,
            &mut asked,
        );
        // What it kept goes with it, once it has read it.
        self.kept
            .retain(|(holder, kept, _)| !(*holder == owner && *kept == power));
        let block = self
            .creatures
            .iter()
            .find(|c| c.id == owner)
            .map_or_else(num_traits::zero, |c| c.block);
        for effect in asked {
            match effect {
                // `CreatureCmd.LoseBlock` for all of it. Block it takes raises
                // `AfterBlockBroken`, which a power coming off has no fight to
                // raise it on.
                sts2_core::HookEffect::LoseBlock if block > num_traits::zero() => {
                    unimplemented!("{power:?} taking block as it comes off")
                }
                sts2_core::HookEffect::LoseBlock => {}
                // `BaseReplayCount` over `AllCards`, the play and exhaust piles
                // among them.
                sts2_core::HookEffect::AddReplays {
                    to: sts2_core::Writing::EveryCopyOf(card),
                    amount,
                } => self
                    .cards
                    .write_all_cards(|held| held.ident.card == card, |held| held.replay(amount)),
                // `CardCmd.ClearAffliction` over `AllCards`.
                sts2_core::HookEffect::ClearAffliction(sts2_core::Writing::EveryAfflictedWith(
                    affliction,
                )) => self.cards.write_all_cards(
                    |held| held.ident.affliction == Some(affliction),
                    |held| held.ident.affliction = None,
                ),
                other => left.push(other),
            }
        }
        left
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
            (CostChange::Relative(1), Lasting::ThisFight),
        ] {
            card.face.modify_cost(CostPool::Energy, change, lasting);
        }

        card.face.turn_ended();

        let kept: Vec<CostChange<i16>> = card
            .face
            .cost_modifiers
            .iter()
            .take(usize::from(card.face.modifiers))
            .map(|held| held.change())
            .collect();
        assert_eq!(
            kept,
            [CostChange::Relative(-1), CostChange::Relative(1)],
            "the turn's own goes, and the two around it close up",
        );
    }

    /// Every cost either build prints fits what a modifier sets.
    ///
    /// [`SET_AT_MOST`] is eight bits. The dearest card either build prints
    /// costs twelve. A build printing a dearer one moves the packing rather
    /// than finding out in a fight.
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
                        i16::from(printed.get()) <= SET_AT_MOST,
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

    /// Only the Mad Sciences, in each ported build, print more than a copy
    /// holds.
    ///
    /// [`VARS`] is a measured constant, so it goes stale the moment a build
    /// prints another wide card. Nothing raises a Mad Science's numbers. A
    /// wide card of any other kind is a decision somebody has to make rather
    /// than a panic in a fight.
    #[test]
    fn only_mad_science_prints_more_than_a_copy_holds() {
        fn wide<V: sts2_core::ContentVersion>(build: &str) {
            // The nine print one class's numbers, so they print the same.
            let mut wide = 0;
            let mut first = None;
            for &card in <V::Card as sts2_core::Table>::ALL {
                let printed = card.vars();
                if printed.len() > VARS {
                    wide += 1;
                    let first = *first.get_or_insert(printed);
                    assert!(
                        printed == first,
                        "{build} holds {VARS} a copy, and {card:?} prints {}",
                        printed.len(),
                    );
                }
            }
            assert_eq!(
                wide, 9,
                "{build} prints the nine Mad Sciences wider than {VARS}"
            );
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
            gold: 0,
            round: 1,
            side: CombatSide::Player,
            history: CombatHistory::new(),
            kept: Vec::new(),
            encounter: Encounter::SlimesWeak,
            cards: super::cards::Cards::with_cards(0),
            removed_from_deck: Vec::new(),
            extra_cards: Vec::new(),
            exhaust_next: smallvec::SmallVec::new(),
            leaving: Vec::new(),
            for_run: Vec::new(),
            after_pick: Vec::new(),
            creatures: creatures.into_iter().collect(),
            powers: PowerStore::new(),
            orbs: None,
            machines: Bounded::new(),
            skip_tick: Bounded::new(),
            card_ids: CardIds::new(),
            kept_faces: Vec::new(),
            attacks: 0,
            last_creature: CombatId::new(nonmax::NonMaxU8::ZERO),
            notes: smallvec::SmallVec::new(),
            asking: None,
            deferred: Vec::new(),
            deferring_at: None,
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
        assert_eq!(size_of::<Telegraphed<V>>(), 20);
        assert_eq!(
            size_of::<Option<Telegraphed<V>>>(),
            20,
            "the move name carries the tag"
        );
        assert_eq!(size_of::<Stacked>(), 2);
        assert_eq!(size_of::<Fighter<V>>(), 40);
    }
}
