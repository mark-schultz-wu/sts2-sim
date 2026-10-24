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
use nonmax::{NonMaxI16, NonMaxI32, NonMaxU16};
use num_traits::CheckedAdd;
use std::sync::Arc;
use sts2_core::{
    CardId, CardIdent, CardNumber, CardTable, CardVars, Combat, CombatId, Creature, Decimal, Orbs,
    Pile, Rules, SideKinds,
};

use super::monster::Machine;
use super::powers::{PowerStore, Powers as _};
use crate::bounded::Bounded;
use crate::run::BELT;

/// The powers that skip their next tick, by the creature holding each.
pub(super) type Skips<V> = Bounded<(CombatId, <V as sts2_core::ContentVersion>::Power), SKIPS>;

/// How a power holds its stack, and an attack its damage.
pub(super) type Amount = i16;

/// How a copy holds how far one of the numbers it prints has moved.
///
/// The build's own game number, an `int`, wide. Both ported builds print
/// inside `-1..=600`, and a fight writes on top of that without end:
/// `ForgeCmd` raises `SovereignBlade`'s damage each time a smith is played,
/// and once a turn under a power that forges. A fight nobody ends ran one
/// past what a 16-bit difference holds.
pub(super) type Moved = i32;

/// What one copy of a card prints, as the difference from its own table row.
///
/// A fresh copy is zeroes, whatever the card prints. An upgrade moves one and
/// `ForgeCmd` moves one again.
///
/// `None` for a card printing more than [`VARS`] numbers. Nothing raises one
/// of those, and [`CardInstance::raise`] says so if something tries.
pub(super) type CardVarList = Option<[NonMaxI32; VARS]>;

/// The powers one creature holds.
pub(crate) type PowerList<V> = Bounded<HeldPower<V>, POWERS>;

/// The most powers one creature holds.
///
/// The game's list has no bound, and an instanced power takes a place per
/// instance. This one is fixed so a fight copies as flat memory, and a
/// creature gaining one more panics by name.
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
/// The game's list has no bound. A card collects one from its own upgrade,
/// and two from a power potion, which sets both pools at once. A turn of
/// cheapening plays writes a pair each time, a shift and a set, which never
/// merge. Random play has stacked five on one Silent card.
const MODIFIERS: usize = 8;

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
    /// `CardPlay.IsAutoPlay`.
    pub(super) auto: bool,
    /// `EnergyCost.CapturedXValue` and `LastStarsSpent`, which every play
    /// of the series reads.
    pub(super) x: V::Count,
    pub(super) stars: V::Count,
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

/// A task a pause boundary set aside, with the question it paused on. The
/// boundary's caller runs on without it. A turn's setup is one, and so is
/// each listener of the hooks the game runs under a boundary per listener.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Paused<V: Rules> {
    pub(super) asking: Asking<V>,
    /// What waits behind it.
    pub(super) waiting: Vec<Deferred<V>>,
    /// How many cards the hand held as it was set aside.
    pub(super) hand: usize,
}

/// The listeners of one hook that the game runs each under a pause boundary
/// of its own, `AssignTaskAndWaitForPauseOrCompletion`. A listener that
/// pauses is set aside, and the next one runs.
pub(super) struct Listening<V: Rules> {
    aside: Vec<Paused<V>>,
    /// The listeners set aside, each of which asks nothing more until it
    /// goes on.
    listeners: Vec<super::listen::Listener<V>>,
}

/// The boundary [`Listening::enter`] opened for one listener.
#[must_use]
pub(super) struct Boundary<V: Rules> {
    /// Nothing for a task that is not one listener's.
    listener: Option<super::listen::Listener<V>>,
    fresh: (usize, Option<usize>),
    boundary: bool,
}

impl<V: Rules> Listening<V> {
    pub(super) fn new() -> Self {
        Listening {
            aside: Vec::new(),
            listeners: Vec::new(),
        }
    }

    /// Open `listener`'s boundary, before what it asked for runs.
    ///
    /// # Panics
    ///
    /// A listener asking for more while its own task waits.
    #[inline]
    pub(super) fn enter(
        &self,
        live: &mut Live<V>,
        listener: super::listen::Listener<V>,
    ) -> Boundary<V> {
        if !self.listeners.is_empty() && self.listeners.contains(&listener) {
            unimplemented!("{listener:?} asking for more behind its own pause");
        }
        Boundary {
            listener: Some(listener),
            fresh: live.deferred.open_fresh(),
            boundary: std::mem::replace(&mut live.pause_boundary, true),
        }
    }

    /// Open a boundary for a task that is not one listener's, such as
    /// `OrbQueue.AfterTurnStart`.
    pub(super) fn enter_task(&self, live: &mut Live<V>) -> Boundary<V> {
        Boundary {
            listener: None,
            fresh: live.deferred.open_fresh(),
            boundary: std::mem::replace(&mut live.pause_boundary, true),
        }
    }

    /// Close the boundary [`Listening::enter`] opened, and set the listener
    /// aside where it paused.
    #[inline]
    pub(super) fn leave(&mut self, live: &mut Live<V>, entered: Boundary<V>) {
        live.pause_boundary = entered.boundary;
        match live.asking.is_some() {
            true => self.set_aside(live, entered),
            false => live.deferred.keep_fresh(entered.fresh),
        }
    }

    /// The listener [`Listening::leave`] found paused, set aside.
    #[cold]
    fn set_aside(&mut self, live: &mut Live<V>, entered: Boundary<V>) {
        self.aside.push(live.set_aside(entered.fresh));
        self.listeners.extend(entered.listener);
    }

    /// Whether no listener paused.
    pub(super) fn none_paused(&self) -> bool {
        self.aside.is_empty()
    }

    /// `Task.WhenAll` over the listeners' tasks: each that paused goes on in
    /// the order it paused, then `then`. False where none paused, and the
    /// caller goes on itself.
    pub(super) fn join(self, live: &mut Live<V>, then: Deferred<V>) -> bool {
        let mut aside = self.aside.into_iter();
        let Some(first) = aside.next() else {
            return false;
        };
        for task in aside {
            live.defer(Deferred::Rejoin(Box::new(task)));
        }
        live.defer(then);
        live.rejoin(first);
        true
    }

    /// The listeners' tasks with no join: each that paused goes on as a
    /// queued action, once the decision is over.
    #[inline]
    pub(super) fn queue(self, live: &mut Live<V>) {
        if !self.aside.is_empty() {
            live.paused.extend(self.aside);
        }
    }

    /// The listeners' tasks where the caller has no way yet to wait for
    /// them.
    ///
    /// # Panics
    ///
    /// Any listener that paused.
    pub(super) fn none_waits(self, hook: &str) {
        if let Some(task) = self.aside.first() {
            unimplemented!("{task:?} pausing in {hook}, which goes on without waiting");
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Asking<V: Rules> {
    pub(super) then: Then<V>,
    pub(super) question: Asked<V>,
    /// `CardSelectCmd` signalled the choice under a pause boundary and has
    /// not listed it yet. It lists once its task goes on, and takes the
    /// cards without asking where its count allows.
    pub(super) signalled: bool,
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
    /// happens to the cards picked, and the rest of the setup waits in
    /// [`Live::deferred`].
    Setup { act: sts2_core::PileAct },
    /// A choice among cards offered in `SetupPlayerTurn`. The card chosen
    /// goes into hand and takes `writes`, and the rest of the setup waits in
    /// [`Live::deferred`].
    SetupChosen {
        writes: &'static [(sts2_core::Change<V>, sts2_core::Lasting)],
    },
    /// A choice a monster's move offered, which `V::resume_play` picks up.
    /// The rest of the move and of the enemy turn waits in
    /// [`Live::deferred`].
    Chosen { at: V::Resume },
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
    /// The player's `AfterTurnEnd` from the pass after one whose listeners
    /// paused, once they are over, and the rest of the turn end behind it.
    AfterTurnEnd {
        walk: Box<super::turn::TurnEndWalk<V>>,
        max_energy: V::Count,
        skipping: Skips<V>,
    },
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
    /// `CardCmd.AutoPlay` of a copy at an enemy drawn as its play begins,
    /// which came up while the question waited.
    AutoPlayAtRandom { id: CardId<V> },
    /// `CardCmd.AutoPlay` of a card `AutoPlayFromDrawPile` already took into
    /// the play pile, with `ExhaustOnNextPlay` set to `force_exhaust` first.
    AutoPlayTaken { id: CardId<V>, force_exhaust: bool },
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
    /// The rest of a play's `AfterCardPlayed` walk, which a question a
    /// listener asked stood still, then `AfterCardPlayedLate`.
    Played {
        /// Nothing where no listener heard the hook.
        walking: Option<super::listen::Walking<V>>,
        card: V::Card,
        id: CardId<V>,
        energy: V::Count,
    },
    /// The end of an outermost play whose exhaust raised a question:
    /// `CheckForEmptyHand`, and the end of a fight it won.
    PlayOver,
    /// A command acting on `cards`, which a question asked before it stood
    /// still.
    ActOn {
        act: sts2_core::PileAct,
        cards: smallvec::SmallVec<[sts2_core::CardAt<V>; 4]>,
    },
    /// `PowerCmd.Apply` over several targets, which a question one of them
    /// raised stood still.
    Land(super::board::Lands<V>),
    /// `CardPileCmd.AutoPlayFromDrawPile`, which a shuffle's question stood
    /// still while it took its cards.
    AutoPlayFromDraw(super::board::AutoPlayFromDraw<V>),
    /// The code after the `await` on a command the relic `relic` in `slot`
    /// asked for, which a question inside the command stood still.
    OwnCommandOver { slot: u8, relic: V::Relic },
    /// `CardPileCmd.Draw` of `count` cards. `resumed` for the rest of a draw
    /// a shuffle's question stopped, which asked `ShouldDraw` already.
    Draw {
        count: usize,
        hand_draw: bool,
        resumed: bool,
        /// What an effect's draw drew before it stood still, which the
        /// effect waits on. Nothing for any other draw.
        drew: Option<sts2_core::Drew<V>>,
    },
    /// The hits of an attack that came up while a question one of its
    /// earlier hits raised waited, each resolved as it lands. Then the end of
    /// the attack, over every hit in `landed` and the rest.
    ///
    /// `AttackCommand`'s hit loop awaits each hit, so the later ones stand
    /// still until the answer comes.
    /// `SetupPlayerTurn` from `stage`, which a hook stopped on its selection.
    Setup(Setup<V>),
    /// A listener's task a pause boundary set aside, which goes on here
    /// where its caller waits for it.
    Rejoin(Box<Paused<V>>),
    /// The player's turn end once every `BeforeSideTurnEnd` listener that
    /// paused is over. `skipping` is what this turn's tick skips.
    TurnEndHooksOver {
        max_energy: V::Count,
        skipping: Skips<V>,
    },
    /// `RunAutoPrePlayPhase` from what a pass still asked for, then from the
    /// pass at `next`, once an auto-play's question is answered. The phase
    /// turns to the play phase after.
    PrePlay {
        rest: super::turn::PassRest<V>,
        next: usize,
    },
    /// The post-play phase from what its listeners still asked for, once an
    /// auto-play's question is answered. The turn's end follows it.
    PostPlay {
        rest: super::turn::PassRest<V>,
        max_energy: V::Count,
    },
    /// The player's turn end from the hand's flush on, once a pick before
    /// it is answered. `skipping` is what this turn's tick skips.
    Flush {
        max_energy: V::Count,
        skipping: Skips<V>,
    },
    /// `DoTurnEnd` from the end of `card`'s `OnTurnEndInHandWrapper`, whose
    /// effect a question stood still. The cards in `rest` run theirs after,
    /// then the turn's end goes on to the flush.
    TurnEnd {
        card: CardId<V>,
        rest: smallvec::SmallVec<[CardId<V>; super::pile::MAX_CARDS_IN_HAND]>,
        max_energy: V::Count,
        skipping: Skips<V>,
    },
    Hits {
        frame: super::board::Frame<V>,
        attack: HeldAttack<V>,
        /// A hit whose `BeforeDamageReceived` asked, to land first.
        half: Option<HalfHit<V>>,
        /// The command over many targets that hit was part of.
        command: Option<Box<HeldCommand<V>>>,
        hits: Vec<sts2_core::Effect<V>>,
        landed: sts2_core::AttackResults<V>,
    },
}

/// What waits behind the question asked, in the order it runs.
///
/// A batch is what the fight defers between one answer, or one entry run,
/// and the next. A batch runs first in, first out, and ahead of every batch
/// deferred before it. That is how the game's awaited commands nest: the
/// innermost finishes first, and inside it the first to stop goes on first.
#[derive(Debug, PartialEq)]
pub(super) struct Deferrals<V: Rules> {
    /// Every entry, the next to run last.
    waiting: Vec<Deferred<V>>,
    /// Where the open batch starts in `waiting`. Each entry of it goes under
    /// the last, so the first runs first.
    batch: Option<usize>,
}

impl<V: Rules> Deferrals<V> {
    pub(super) fn new() -> Self {
        Deferrals {
            waiting: Vec::new(),
            batch: None,
        }
    }

    /// Put `deferred` last in the open batch, opening one if none is.
    pub(super) fn push(&mut self, deferred: Deferred<V>) {
        let at = *self.batch.get_or_insert(self.waiting.len());
        self.waiting.insert(at, deferred);
    }

    /// Close the open batch, so what comes next opens one that runs first.
    pub(super) fn close(&mut self) {
        self.batch = None;
    }

    /// Open a batch of its own at the end, for [`Deferrals::take_fresh`] to
    /// take back. What comes back is what to hand to that.
    pub(super) fn open_fresh(&mut self) -> (usize, Option<usize>) {
        (self.waiting.len(), self.batch.take())
    }

    /// What was deferred since [`Deferrals::open_fresh`] handed back
    /// `fresh`, taken out, with the batch that was open then open again.
    pub(super) fn take_fresh(&mut self, (len, batch): (usize, Option<usize>)) -> Vec<Deferred<V>> {
        let taken = self.waiting.split_off(len);
        self.batch = batch;
        taken
    }

    /// Back to the batch that was open when [`Deferrals::open_fresh`]
    /// handed back `fresh`, with nothing deferred since taken.
    pub(super) fn keep_fresh(&mut self, (_, batch): (usize, Option<usize>)) {
        self.batch = batch;
    }

    /// Whether nothing waits.
    fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }

    /// `taken` to run next, in the order it was deferred in.
    fn put_back(&mut self, taken: Vec<Deferred<V>>) {
        self.close();
        self.waiting.extend(taken);
    }

    /// The next entry to run, which closes the open batch first.
    pub(super) fn next(&mut self) -> Option<Deferred<V>> {
        self.close();
        self.waiting.pop()
    }
}

impl<V: Rules> Clone for Deferrals<V> {
    fn clone(&self) -> Self {
        Deferrals {
            waiting: self.waiting.clone(),
            batch: self.batch,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.waiting.clone_from(&source.waiting);
        self.batch = source.batch;
    }
}

/// A hit whose `BeforeDamageReceived` listeners have run, and which waits
/// on the question one of them asked to meet block and take hit points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct HalfHit<V: Rules> {
    pub(super) dealer: Option<CombatId>,
    pub(super) target: CombatId,
    pub(super) damage: V::Number,
    pub(super) powered: bool,
    pub(super) unblockable: bool,
    pub(super) card: Option<V::Card>,
    pub(super) attack: bool,
}

/// A `CreatureCmd.Damage` over many targets that stood still on one hit's
/// question. `landed` is the hits it landed before that one, whose hooks
/// wait. `hits` is how many of the held hits after it are its own.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct HeldCommand<V: Rules> {
    pub(super) landed: Vec<super::board::Landed<V>>,
    pub(super) hits: usize,
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
    /// `ModelSource` where it is a card.
    pub(super) source: Option<(sts2_core::CardId<V>, V::Card)>,
}

/// Where `SetupPlayerTurn` stands, with what the hook it stopped in has still
/// to carry out.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Setup<V: Rules> {
    /// Inside `BeforeHandDraw`, before the hand is drawn: the rest of its
    /// walk, boxed as `PlayerTurnStart`'s is. Nothing where no listener
    /// hears the hook.
    BeforeHandDraw(Option<Box<super::listen::Walking<V>>>),
    /// Drawing the hand, with this many cards still to draw. `resumed` for a
    /// draw that a shuffle's question paused.
    Drawing { left: usize, resumed: bool },
    /// Inside `AfterPlayerTurnStart`, the last of the setup.
    /// Boxed, as the walk is far larger than the stages before it, which
    /// move it at every step of the setup. Nothing where no listener hears
    /// the hook.
    PlayerTurnStart(Option<Box<super::listen::Walking<V>>>),
    /// Drawing for a listener inside `AfterPlayerTurnStart`, with this many
    /// cards still to draw and the listeners after it still to run.
    /// `resumed` for a draw that a shuffle's question paused.
    TurnStartDrawing {
        left: usize,
        resumed: bool,
        rest: Option<Box<super::listen::Walking<V>>>,
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

    /// The id the next [`CardIds::mint`] hands out.
    pub(super) fn peek(&self) -> CardId<V> {
        CardId::new(self.0)
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

/// `EnergyCost` and `StarCost` of a fresh card `ident` names, with what its
/// upgrade and enchantment write on them, as a merchant shows one.
pub(crate) fn printed_costs<V: Rules>(ident: CardIdent<V>) -> (sts2_core::Cost, sts2_core::Cost) {
    let face = Face::fresh(ident);
    (face.cost, face.star_cost)
}

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
    /// How many decimal places [`Face::vars`] are written in.
    ///
    /// Zero but on a copy a fight wrote a fraction on. Only Thrash does: it
    /// adds a hit worked out by `Hook.ModifyDamage`, which a share can leave
    /// a fraction in. Every difference then counts in that place's units.
    places: u8,
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
    /// The enchantment on this copy: its `EnchantmentModel.Status`, which its
    /// own `OnPlay` reads and sets, and the fields its class keeps. A fight
    /// starts every copy's fresh.
    enchantment_state: V::EnchantmentState,
    /// `IsDupe`: a copy made to be played once, which leaves the fight once
    /// its play is over.
    dupe: bool,
    /// The fields the copy's own class adds, which its `AfterDowngraded`
    /// reads. A fight starts every copy's fresh, then writes on what its deck
    /// card saved. A copy of a copy carries them over.
    fields: V::CardFields,
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

impl<V: Rules> sts2_core::Printed<V> for Face<V> {
    fn face(&self, key: V::CardVar) -> V::Number {
        self.number(key).base
    }
}

impl<V: Rules> Face<V> {
    /// The entry this copy holds for `key`.
    ///
    /// A key the card does not print is a mistake in the port, the way a card
    /// asking for a number no card prints is. So is a fraction a fight wrote
    /// on it, which only [`Face::exact`] reads.
    pub(super) fn number(&self, key: V::CardVar) -> CardNumber<V::CardVar, V::Number> {
        let vars = self.ident.card.vars();
        let at = Self::at(vars, key);
        match self.printed(&vars[at], at) {
            (number, true) => number,
            (_, false) => unimplemented!("the fractional {} a fight wrote on it", key.name()),
        }
    }

    /// `BaseValue` for `key`: the decimal this copy prints.
    pub(super) fn exact(&self, key: V::CardVar) -> Decimal {
        let vars = self.ident.card.vars();
        let at = Self::at(vars, key);
        let printed = Self::table(&vars[at]);
        let printed = Decimal::from(
            num_traits::ToPrimitive::to_i64(&printed).expect("a game number fits a long"),
        );
        printed + Decimal::new(i64::from(self.moved(at)), u32::from(self.places))
    }

    /// Whether a fight wrote a fraction on this copy, which only
    /// [`Face::exact`] reads.
    pub(super) fn fractional(&self) -> bool {
        self.places != 0
    }

    /// How far this copy has moved the number its row holds at `at`, in
    /// units of [`Face::places`].
    fn moved(&self, at: usize) -> Moved {
        self.vars.map_or(0, |row| row[at].get())
    }

    /// Every number this copy prints, in the order its table prints them.
    ///
    /// The table names each one and says how it is worked out. This copy
    /// carries the amounts, which is the half a fight writes on. A fraction
    /// reads as its whole part, which is the `(int)BaseValue` a front end
    /// sees.
    pub(super) fn numbers(&self) -> impl Iterator<Item = CardNumber<V::CardVar, V::Number>> + '_ {
        self.ident
            .card
            .vars()
            .iter()
            .enumerate()
            .map(|(at, named)| self.printed(named, at).0)
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

    /// What the table prints for one of its numbers.
    fn table(
        named: &CardNumber<V::CardVar, <V::Card as sts2_core::CardTable>::Number>,
    ) -> V::Number {
        num_traits::NumCast::from(named.base).unwrap_or_else(|| {
            unimplemented!("the fractional {} this card prints", named.id.name())
        })
    }

    /// One of the table's numbers, the one at `at`, as this copy prints it.
    /// Its whole part, and whether that is all of it.
    #[inline]
    fn printed(
        &self,
        named: &CardNumber<V::CardVar, <V::Card as sts2_core::CardTable>::Number>,
        at: usize,
    ) -> (CardNumber<V::CardVar, V::Number>, bool) {
        if self.places != 0 {
            return self.printed_exactly(named);
        }
        let moved: V::Number =
            num_traits::NumCast::from(self.moved(at)).expect("a difference fits a game number");
        let number = CardNumber {
            id: named.id,
            base: Self::table(named) + moved,
            calculated: named.calculated,
            modifier: named.modifier,
        };
        (number, true)
    }

    /// [`Face::printed`] on a copy a fight wrote a fraction on.
    #[cold]
    #[inline(never)]
    fn printed_exactly(
        &self,
        named: &CardNumber<V::CardVar, <V::Card as sts2_core::CardTable>::Number>,
    ) -> (CardNumber<V::CardVar, V::Number>, bool) {
        let exact = self.exact(named.id);
        let base = num_traits::ToPrimitive::to_i64(&exact.trunc())
            .and_then(num_traits::cast)
            .expect("a printed number fits a game number");
        let number = CardNumber {
            id: named.id,
            base,
            calculated: named.calculated,
            modifier: named.modifier,
        };
        (number, exact.fract().is_zero())
    }

    /// Add to what this copy prints for `key`.
    ///
    /// # Panics
    ///
    /// A card too wide to hold a difference. No fight raises one: the only
    /// card either build prints that wide belongs to an event, and no event
    /// is ported.
    ///
    /// A number raised past what a [`Moved`] holds. `ForgeCmd` is the only
    /// thing that raises one repeatedly, and arriving here takes four hundred
    /// million smiths in one fight.
    pub(super) fn raise(&mut self, key: V::CardVar, by: V::Number) {
        if self.places != 0 {
            let by = num_traits::ToPrimitive::to_i64(&by).expect("a game number fits a long");
            return self.raise_exactly(key, Decimal::from(by));
        }
        let at = Self::at(self.ident.card.vars(), key);
        let by: Moved = num_traits::NumCast::from(by)
            .unwrap_or_else(|| unimplemented!("the {} raised by {by:?}", key.name()));
        let row = self.vars.as_mut().unwrap_or_else(|| {
            unimplemented!("the {} raised on a card too wide to hold one", key.name())
        });
        let moved = row[at]
            .get()
            .checked_add(by)
            .and_then(NonMaxI32::new)
            .unwrap_or_else(|| unimplemented!("the {} raised past what a card holds", key.name()));
        row[at] = moved;
    }

    /// Add `by`, which can hold a fraction, to what this copy prints for
    /// `key`.
    ///
    /// A fraction finer than the row's places moves every difference to
    /// finer units first.
    ///
    /// # Panics
    ///
    /// As [`Face::raise`] does, and for a difference too wide for an
    /// [`Moved`] in the units the row counts in.
    pub(super) fn raise_exactly(&mut self, key: V::CardVar, by: Decimal) {
        let at = Self::at(self.ident.card.vars(), key);
        let past = || -> ! { unimplemented!("the {} raised past what a card holds", key.name()) };
        let row = self.vars.as_mut().unwrap_or_else(|| {
            unimplemented!("the {} raised on a card too wide to hold one", key.name())
        });
        let by = by.normalize();
        let places = u8::try_from(by.scale()).unwrap_or_else(|_| past());
        if places > self.places {
            let finer = 10_i32
                .checked_pow(u32::from(places - self.places))
                .unwrap_or_else(|| past());
            for moved in row.iter_mut() {
                *moved = moved
                    .get()
                    .checked_mul(finer)
                    .and_then(NonMaxI32::new)
                    .unwrap_or_else(|| past());
            }
            self.places = places;
        }
        let by = by.mantissa() * 10_i128.pow(u32::from(self.places) - by.scale());
        let by = Moved::try_from(by).unwrap_or_else(|_| past());
        row[at] = row[at]
            .get()
            .checked_add(by)
            .and_then(NonMaxI32::new)
            .unwrap_or_else(|| past());
    }

    /// A fresh copy's state, printing what the table prints.
    fn fresh(ident: CardIdent<V>) -> Self {
        // Zeroes, because a fresh copy prints what its row prints. A card too
        // wide for the row carries no differences at all, which is what its
        // `None` says.
        let vars = (ident.card.vars().len() <= VARS).then_some([NonMaxI32::ZERO; VARS]);
        let mut card = Self {
            ident,
            vars,
            places: 0,
            cost: ident.card.cost(),
            star_cost: ident.card.star_cost(),
            cost_modifiers: [CostModifier::from_bits(0); MODIFIERS],
            modifiers: 0,
            added_keywords: sts2_core::FlagSet::from_members(&[]),
            removed_keywords: sts2_core::FlagSet::from_members(&[]),
            turn_keywords: sts2_core::FlagSet::from_members(&[]),
            replays: 0,
            enchantment_state: sts2_core::EnchantmentModel::new(
                ident.enchantment.map(|enchanted| enchanted.enchantment),
            ),
            dupe: false,
            fields: sts2_core::CardFields::new(ident.card),
        };
        if card.ident.upgrade != 0 {
            assert_eq!(card.ident.upgrade, 1, "only first upgrades are ported");
            // The level is already on the identity, so this writes what the
            // upgrade writes without counting it again.
            for &change in V::card_upgrade(card.ident.card) {
                card.upgrade_change(change);
            }
            card.recalculate();
        }
        card.enchantment_writes();
        card
    }

    /// `Enchantment.ModifyCard`: what the copy's enchantment writes on it,
    /// then `RecalculateForUpgradeOrEnchant`.
    fn enchantment_writes(&mut self) {
        if let Some(enchanted) = self.ident.enchantment {
            for &change in V::enchanted_changes(enchanted.enchantment) {
                self.modify(change, sts2_core::Lasting::ThisFight);
            }
            self.recalculate();
        }
    }

    /// `EnergyCost.GetWithModifiers` and its star twin, as the pair a play
    /// reads.
    ///
    /// The modifiers fold over the printed cost in the order they were
    /// written, and a cost that is not a number folds nothing. `energy` is
    /// `Hook.ModifyEnergyCostInCombat` and `stars` is `Hook.ModifyStarCost`,
    /// each run over its pool's cost after those and before the clamp. Each
    /// leaves a cost below zero alone, so this does not ask it one.
    pub(super) fn costs(
        &self,
        energy: impl FnOnce(V::Number) -> V::Number,
        stars: impl FnOnce(V::Number) -> V::Number,
    ) -> (sts2_core::Cost, sts2_core::Cost) {
        // The cost with its own modifiers folded in, or the cost itself where
        // it is not a number.
        let fold = |pool, printed| {
            let sts2_core::Cost::Fixed(printed) = printed else {
                return Err(printed);
            };
            let printed: V::Number =
                num_traits::NumCast::from(printed.get()).expect("a fixed cost fits a game number");
            Ok(self
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
                }))
        };
        // `Math.Max(0, num)`, which every read clamps at.
        let clamp = |folded: V::Number| {
            let largest: V::Number =
                num_traits::NumCast::from(u8::MAX - 1).expect("a fixed cost fits a game number");
            let folded = folded.clamp(num_traits::zero(), largest);
            let folded = num_traits::NumCast::from(folded)
                .expect("a clamped fixed cost fits its representation");
            sts2_core::Cost::fixed(folded)
        };
        let energy_cost = match fold(sts2_core::CostPool::Energy, self.cost) {
            Err(cost) => cost,
            Ok(folded) if folded >= num_traits::zero() => clamp(energy(folded)),
            Ok(folded) => clamp(folded),
        };
        let star_cost = match fold(sts2_core::CostPool::Star, self.star_cost) {
            Err(cost) => cost,
            Ok(folded) if folded >= num_traits::zero() => clamp(stars(folded)),
            Ok(folded) => clamp(folded),
        };
        (energy_cost, star_cost)
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
            sts2_core::Change::RaiseExactly(key, amount) => self.raise_exactly(key, amount.exact()),
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
            // A field of the class lasts as long as the copy does.
            sts2_core::Change::Own(write) => {
                assert!(
                    lasting == sts2_core::Lasting::ThisFight,
                    "a class's field written for less than the fight"
                );
                let mut fields = self.fields;
                let change = sts2_core::CardFields::write(&mut fields, write, self);
                self.fields = fields;
                self.modify(change, lasting);
            }
        }
    }

    /// `Keywords.Contains(keyword)` for this copy: what the table prints, as
    /// `printed` says, and what has been written on, less what was taken off.
    /// Then what a power gives it for the affliction it carries, which
    /// `TryModifyKeywordsInCombat` adds after the rest. A word set for the
    /// turn alone is a flag beside the set, not in it.
    fn keyword(&self, printed: bool, keyword: V::CardKeyword) -> bool {
        (printed || sts2_core::FlagSet::contains(self.added_keywords, keyword))
            && !sts2_core::FlagSet::contains(self.removed_keywords, keyword)
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
        self.keyword(self.prints(keyword), keyword)
    }

    /// `ShouldRetainThisTurn` and `IsSlyThisTurn`: the keyword, or the flag
    /// `GiveSingleTurnRetain` or `GiveSingleTurnSly` set for the turn.
    pub(super) fn has_keyword_this_turn(&self, keyword: V::CardKeyword) -> bool {
        self.has_keyword(keyword) || sts2_core::FlagSet::contains(self.turn_keywords, keyword)
    }

    /// `LocalKeywords.Contains(keyword)`: what the table prints and the fight
    /// wrote on, less what it took off. No word for the turn, and none a
    /// power grants.
    pub(super) fn has_local_keyword(&self, keyword: V::CardKeyword) -> bool {
        (self.prints(keyword) || sts2_core::FlagSet::contains(self.added_keywords, keyword))
            && !sts2_core::FlagSet::contains(self.removed_keywords, keyword)
    }

    /// Whether the table prints `keyword` on this card.
    fn prints(&self, keyword: V::CardKeyword) -> bool {
        use sts2_core::{CardTable as _, KeywordKinds};

        let card = self.ident.card;
        match keyword {
            k if k == <V::CardKeyword as KeywordKinds<V>>::INNATE => card.innate(),
            k if k == <V::CardKeyword as KeywordKinds<V>>::RETAIN => card.retains(),
            k if k == <V::CardKeyword as KeywordKinds<V>>::SLY => card.sly(),
            k if k == <V::CardKeyword as KeywordKinds<V>>::ETHEREAL => card.ethereal(),
            k if k == <V::CardKeyword as KeywordKinds<V>>::EXHAUST => card.exhausts(),
            k if k == <V::CardKeyword as KeywordKinds<V>>::UNPLAYABLE => card.unplayable(),
            keyword => unimplemented!("whether {card:?} prints {keyword:?}"),
        }
    }

    /// `Keywords.Contains(Exhaust)` for this copy, which an upgrade can take
    /// off.
    /// `IsDupe` for this copy.
    pub(super) fn dupe(&self) -> bool {
        self.dupe
    }

    /// `CreateDupe`: this copy as a dupe, with its Exhaust taken off.
    pub(super) fn duped(mut self) -> Self {
        use sts2_core::KeywordKinds;

        let exhaust = <V::CardKeyword as KeywordKinds<V>>::EXHAUST;
        self.dupe = true;
        self.added_keywords = sts2_core::FlagSet::without(self.added_keywords, exhaust);
        self.removed_keywords = sts2_core::FlagSet::with(self.removed_keywords, exhaust);
        self
    }

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
    /// reads Innate, or Sly given for the turn.
    pub(super) fn sly(&self) -> bool {
        use sts2_core::KeywordKinds;

        self.has_keyword_this_turn(<V::CardKeyword as KeywordKinds<V>>::SLY)
    }

    /// `ShouldRetainThisTurn` for this copy, the keyword the way
    /// [`Face::innate`] reads Innate, or Retain given for the turn.
    pub(super) fn retains(&self) -> bool {
        use sts2_core::KeywordKinds;

        self.has_keyword_this_turn(<V::CardKeyword as KeywordKinds<V>>::RETAIN)
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
                &self.enchantment_state,
                sts2_core::EnchantHook::PlayCount,
                replays,
            ),
            None => replays,
        };
        replays + num_traits::one()
    }

    /// The enchantment on this copy, as its model stands.
    pub(super) fn enchantment_state(&self) -> V::EnchantmentState {
        self.enchantment_state
    }

    /// Set the enchantment on this copy, and say whether that changed it.
    pub(super) fn set_enchantment_state(&mut self, state: V::EnchantmentState) -> bool {
        std::mem::replace(&mut self.enchantment_state, state) != state
    }

    /// Set this copy's `EnchantmentModel.Status`, and say whether that changed
    /// it.
    pub(super) fn set_enchantment_status(&mut self, status: sts2_core::EnchantmentStatus) -> bool {
        let mut state = self.enchantment_state;
        sts2_core::EnchantmentModel::set_status(&mut state, status);
        self.set_enchantment_state(state)
    }

    /// `Amount += by` on this copy's enchantment, and say whether that
    /// changed it.
    ///
    /// # Panics
    ///
    /// A copy with no enchantment, or an amount past what a byte holds.
    pub(super) fn raise_enchantment(&mut self, by: u8) -> bool {
        let enchanted = (self.ident.enchantment.as_mut())
            .unwrap_or_else(|| unreachable!("an enchantment raised on a card without one"));
        enchanted.amount = (enchanted.amount.checked_add(by))
            .unwrap_or_else(|| unimplemented!("an enchantment past what a byte holds"));
        by != 0
    }

    /// `CardCmd.Enchant`: this copy as `ident` names it, with a fresh
    /// instance of its enchantment.
    pub(super) fn enchant(&mut self, ident: CardIdent<V>) {
        self.ident = ident;
        self.enchantment_state = sts2_core::EnchantmentModel::new(
            ident.enchantment.map(|enchanted| enchanted.enchantment),
        );
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
    /// them. The cost modifiers and the class's own fields stay. Then the
    /// class's `AfterDowngraded` writes, and the enchantment writes again.
    ///
    /// A calculated number keeps the base the upgraded `CalculationBase`
    /// printed. `DynamicVars.Clone(this)` sets each one's base from its
    /// owner's vars, and the owner still holds the old set while it does.
    pub(super) fn downgrade(&mut self) {
        let mut fresh = Self::fresh(CardIdent {
            upgrade: 0,
            enchantment: None,
            ..self.ident
        });
        fresh.ident.enchantment = self.ident.enchantment;
        fresh.cost_modifiers = self.cost_modifiers;
        fresh.modifiers = self.modifiers;
        fresh.replays = self.replays;
        fresh.enchantment_state = self.enchantment_state;
        fresh.fields = self.fields;
        fresh.calculate_from(self);
        if let Some(change) = sts2_core::CardFields::after_downgraded(&fresh.fields, &fresh) {
            fresh.modify(change, sts2_core::Lasting::ThisFight);
        }
        fresh.enchantment_writes();
        *self = fresh;
    }

    /// `SovereignBlade.AddDamage(amount)`, which a forge calls.
    pub(super) fn add_damage(&mut self, amount: V::Number) {
        let mut fields = self.fields;
        let change = sts2_core::CardFields::add_damage(&mut fields, amount, self);
        self.fields = fields;
        self.modify(change, sts2_core::Lasting::ThisFight);
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
        self.recalculate();
        self.ident.upgrade += 1;
    }

    /// `RecalculateForUpgradeOrEnchant`, which gives a calculated number the
    /// base its `CalculationBase` now prints.
    pub(super) fn recalculate(&mut self) {
        let from = *self;
        self.calculate_from(&from);
    }

    /// Give each calculated number the base `from`'s `CalculationBase`
    /// prints.
    fn calculate_from(&mut self, from: &Self) {
        let base = <V::CardVar as sts2_core::CardVars>::CALCULATION_BASE;
        let calculated: smallvec::SmallVec<[_; 2]> =
            self.numbers().filter(|n| n.calculated).collect();
        for number in calculated {
            let by = from.number(base).base - number.base;
            if by != num_traits::zero() {
                self.raise(number.id, by);
            }
        }
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
    /// The results of the attack whose `AfterAttack` is being raised.
    pub(crate) attack: &'a [sts2_core::DamageResult<V>],
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

    fn attack_result(&self, creature: CombatId) -> Option<sts2_core::DamageResult<V>> {
        self.attack
            .iter()
            .find(|result| result.receiver == creature)
            .copied()
    }

    fn current_hp(&self, creature: CombatId) -> Option<V::Health> {
        self.creatures
            .iter()
            .find(|fighter| fighter.id == creature)
            .map(|fighter| fighter.hp)
    }

    fn player(&self) -> CombatId {
        super::board::player::<V>(self.creatures)
    }

    /// The other side's creatures, a pet among them.
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

    /// Its attack's hit count read again, as an intent that works its count
    /// out reads it each time it is shown.
    pub(crate) fn reread_repeats(&mut self, repeats: V::Count) {
        let repeats = num_traits::NumCast::from(repeats)
            .and_then(NonMaxU16::new)
            .unwrap_or_else(|| unimplemented!("an attack landing {repeats:?} times"));
        for intent in self.intents.iter_mut().flatten() {
            if let Some(attack) = intent.attack.as_mut() {
                attack.repeats = repeats;
            }
        }
    }

    /// Its attack's damage read again, as an intent that works its damage
    /// out reads it each time it is shown.
    pub(crate) fn reread_damage(&mut self, damage: i32) {
        let damage = i16::try_from(damage)
            .ok()
            .and_then(NonMaxI16::new)
            .unwrap_or_else(|| unimplemented!("an attack dealing {damage}"));
        for intent in self.intents.iter_mut().flatten() {
            if let Some(attack) = intent.attack.as_mut() {
                attack.damage = damage;
            }
        }
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
/// [`sts2_core::Attack`] narrowed, which the report widens to a game number.
/// The damage keeps a width that survives `ModifyDamage`, which adds Strength
/// with no ceiling. The hit count outgrows the content's `u8`, as a move whose
/// hits grow each turn passes 255 in a fight nobody ends. Both are `NonMax`,
/// so `Option<Landing>` costs no tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Landing {
    damage: NonMaxI16,
    repeats: NonMaxU16,
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
                .and_then(NonMaxU16::new)
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

/// A monster made and not yet in the fight: what joins when it is added.
#[derive(Debug, Clone)]
pub(crate) struct Created<V: Rules> {
    pub(crate) fighter: Fighter<V>,
    pub(crate) monster: V::Monster,
    /// What landed on it meanwhile, which joins with it.
    pub(crate) powers: Vec<HeldPower<V>>,
}

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
    /// Which instance this is. The store hands each power a new one as it
    /// arrives, so one taken off and applied again is another.
    pub(crate) instance: PowerInstance,
}

/// One power instance in a fight, which the game tells apart by reference.
/// A walk over the listeners names a power by this.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub(crate) struct PowerInstance(pub(crate) u32);

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

    /// A power as `PowerCmd.Apply` lands one, its model as `BeforeApplied`
    /// left it.
    pub(crate) fn landing(
        state: V::PowerState,
        amount: Stacked,
        applier: Option<CombatId>,
    ) -> Self {
        Self {
            amount,
            on_turn_start: Stacked::counted(0),
            applier,
            state,
            instance: PowerInstance::default(),
        }
    }

    /// A power as `PowerCmd.Apply` lands one, its model as new as it is.
    pub(crate) fn new(power: V::Power, amount: Stacked, applier: Option<CombatId>) -> Self {
        Self {
            amount,
            on_turn_start: Stacked::counted(0),
            applier,
            state: <V::PowerState as sts2_core::PowerModel<V>>::new(power),
            instance: PowerInstance::default(),
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

    /// `Creature.IsAlive`, which is health left. A command's hits all land
    /// before any creature they took to nothing is killed. The game reads
    /// such a creature as dead before then.
    pub(crate) fn is_alive(&self) -> bool {
        self.alive && self.hp > num_traits::zero()
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
            alive: self.is_alive(),
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
    /// `CombatState.EscapedCreatures`, as the monsters they were.
    escaped: Bounded<V::Monster, ESCAPED>,
}

/// The most creatures that escape one fight. A fight here sees at most two.
const ESCAPED: usize = 4;

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
            escaped: Bounded::new(),
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
        if let Some(sts2_core::ModelId::Monster(monster)) = self
            .iter()
            .find(|creature| creature.id == id)
            .and_then(|creature| creature.model)
        {
            self.escaped.push(monster);
        }
        self.retain(|creature| creature.id != id);
    }

    /// `EncounterModel.CalculateGoldProportion`'s own: the share of the
    /// enemies that joined the fight and did not escape.
    fn gold_proportion(&self) -> f32 {
        let escaped = u8::try_from(self.escaped.len()).expect("a few creatures escape");
        1.0 - f32::from(escaped) / f32::from(self.spawned_enemies)
    }

    /// Whether any enemy escaped the fight.
    pub(crate) fn any_escaped(&self) -> bool {
        !self.escaped.is_empty()
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

/// `CardModel.DeckVersion` for each copy the fight opened with.
///
/// The opening copies are minted one after another in deck order, so the
/// first one's id and the deck's own ids say which is which. Every copy made
/// later has none, as the game's clone clears it.
#[derive(Debug, Clone)]
pub(crate) struct DeckVersions<V: Rules> {
    first: Option<CardId<V>>,
    deck: Arc<[sts2_core::DeckCardId]>,
}

impl<V: Rules> DeckVersions<V> {
    /// A fight whose copies came from no deck.
    pub(super) fn none() -> Self {
        Self {
            first: None,
            deck: Arc::from([]),
        }
    }

    /// Copies minted from `first` on, one for each of `deck` in order.
    pub(super) fn new(first: CardId<V>, deck: Arc<[sts2_core::DeckCardId]>) -> Self {
        Self {
            first: Some(first),
            deck,
        }
    }

    /// The deck card `card` is a copy of.
    pub(super) fn of(&self, card: CardId<V>) -> Option<sts2_core::DeckCardId> {
        use num_traits::ToPrimitive as _;

        let first = self.first?.get().to_usize()?;
        let offset = card.get().to_usize()?.checked_sub(first)?;
        self.deck.get(offset).copied()
    }
}

/// One of `CombatRoom.ExtraRewards`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExtraReward<V: Rules> {
    /// A `SpecialCardReward` of this card.
    SpecialCard(CardIdent<V>),
    /// A `SpecialCardReward` of a deck card the fight took, handed back.
    Returned(sts2_core::DeckCardId),
    /// A `CardReward` at `CardCreationOptions.ForRoom` of the fight's room.
    Card,
    /// `RelicReward(player)`: the next relic from the front of the bag, at a
    /// rolled rarity.
    Relic,
    /// `PotionReward(player)`: a random potion out of combat.
    Potion,
    /// `GoldReward(amount)`.
    Gold(V::Number),
    /// `RelicReward(relic, player)` for this relic.
    SpecificRelic(V::Relic),
}

/// The modifiers the run was started with, which the fight asks after
/// every creature's listeners.
#[derive(Debug)]
pub(crate) struct RunModifiers<V: Rules> {
    held: Bounded<sts2_core::RunModifier<V>, RUN_MODIFIERS>,
    /// Every hook a held modifier's class overrides, kept with `held`.
    pub(super) hooks: sts2_core::Overrides,
}

/// Every modifier the game's custom mode offers, which a run holds at most
/// once each.
const RUN_MODIFIERS: usize = 17;

impl<V: Rules> Clone for RunModifiers<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: Rules> Copy for RunModifiers<V> {}

impl<V: Rules> RunModifiers<V> {
    pub(super) fn holding(modifiers: &[sts2_core::RunModifier<V>]) -> Self {
        Self {
            held: modifiers.iter().copied().collect(),
            hooks: modifiers
                .iter()
                .fold(sts2_core::Overrides::NONE, |hooks, entry| {
                    hooks.union(sts2_core::ModifierTable::overrides(entry.modifier))
                }),
        }
    }

    /// The held modifiers that override a hook in `hooks`, in the run's
    /// order, with what each overrides.
    pub(super) fn hearing(
        &self,
        hooks: sts2_core::Overrides,
    ) -> impl Iterator<Item = (sts2_core::RunModifier<V>, sts2_core::Overrides)> + '_ {
        self.held.iter().filter_map(move |&entry| {
            let overrides = sts2_core::ModifierTable::overrides(entry.modifier);
            overrides.intersects(hooks).then_some((entry, overrides))
        })
    }
}

/// The run's potions, in slot order, as the fight reads and spends them.
#[derive(Debug)]
pub(crate) struct PotionBelt<V: Rules> {
    held: Bounded<V::Potion, BELT>,
    /// Every hook a held potion's class overrides, kept with `held`.
    hooks: sts2_core::Overrides,
    /// Every hook the run's modifiers override, which never changes.
    modifiers: sts2_core::Overrides,
    /// `hooks` and `modifiers` together. Neither hears most hooks, and one
    /// test of this rules both out.
    either: sts2_core::Overrides,
    /// Where each potion spent since the run last looked stood.
    spent: Bounded<u8, BELT>,
}

impl<V: Rules> Clone for PotionBelt<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V: Rules> Copy for PotionBelt<V> {}

impl<V: Rules> PotionBelt<V> {
    /// `potions`, beside modifiers that override `modifiers`.
    pub(super) fn holding(potions: &[V::Potion], modifiers: sts2_core::Overrides) -> Self {
        let hooks = hooks_of::<V>(potions);
        Self {
            held: potions.iter().copied().collect(),
            hooks,
            modifiers,
            either: hooks.union(modifiers),
            spent: Bounded::new(),
        }
    }

    /// Whether a potion or a modifier overrides one of `hooks`.
    #[inline(always)]
    pub(super) fn either_hears(&self, hooks: sts2_core::Overrides) -> bool {
        self.either.intersects(hooks)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// The potions in slot order.
    pub(super) fn held(&self) -> &[V::Potion] {
        &self.held
    }

    /// No potion hears a hook after, as a dead player's potions hear none.
    pub(super) fn deafen(&mut self) {
        self.hooks = sts2_core::Overrides::default();
        self.either = self.modifiers;
    }

    /// `RemoveBeforeUse` on the potion at `at`.
    pub(super) fn spend(&mut self, at: usize) -> V::Potion {
        let potion = self.held.remove(at);
        self.hooks = hooks_of::<V>(&self.held);
        self.either = self.hooks.union(self.modifiers);
        self.spent
            .push(u8::try_from(at).expect("a belt slot fits a byte"));
        potion
    }
}

/// Every hook one of `potions` overrides.
fn hooks_of<V: Rules>(potions: &[V::Potion]) -> sts2_core::Overrides {
    potions
        .iter()
        .fold(sts2_core::Overrides::NONE, |hooks, potion| {
            hooks.union(sts2_core::PotionTable::overrides(*potion))
        })
}

/// A fight in progress.
#[derive(Debug)]
pub(crate) struct Live<V: Rules> {
    pub(super) character: V::Character,
    /// What the run is being played at, which a card or a move reads for the
    /// numbers the level moves.
    pub(super) ascension: V::AscensionLevel,
    pub(crate) turn: V::Count,
    pub(super) phase: V::PlayerTurnPhase,
    pub(super) energy: V::Count,
    pub(super) stars: V::Count,
    /// The player's `Gold`, which the fight holds while it runs. The run
    /// takes it back after each step.
    pub(super) gold: V::Number,
    /// The player's potions. The belt is the run's, and the run keeps this
    /// in step with it while the fight lasts.
    pub(super) potions: PotionBelt<V>,
    pub(super) modifiers: RunModifiers<V>,
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
    /// `CombatState.Encounter`'s own fields.
    pub(super) encounter_state: V::EncounterState,
    pub(super) cards: super::cards::Cards<V>,
    /// Deck cards a combat command removed permanently while this fight ran.
    pub(super) removed_from_deck: Vec<sts2_core::DeckCardId>,
    /// `SetReadyToEndTurn`: the player's turn ends once the play in progress
    /// is over.
    pub(super) ending_turn: bool,
    /// `CombatManager.IsInProgress`, which `StartCombatInternal` sets just
    /// before `BeforeCombatStart`. `AfterRoomEntered` runs before it.
    pub(super) in_progress: bool,
    /// `CombatManager`'s card or potion effect depth for the player: how
    /// many plays or potions are between `BeginCardOrPotionEffect` and
    /// `EndCardOrPotionEffect`.
    pub(super) effect_depth: u8,
    /// `RunState.CurrentMapPoint.coord`: where on the map the fight stands,
    /// or nothing off the map.
    pub(super) point: Option<sts2_core::Coord>,
    /// Which deck card each of the fight's opening copies came from.
    pub(super) deck_versions: DeckVersions<V>,
    /// The deck cards the run took out of the deck for this fight's
    /// removals, with what their models saved, for a thief to hand back.
    pub(super) taken_from_deck: Vec<sts2_core::DeckCard<V>>,
    /// `CombatRoom.ExtraRewards`, in the order they were added. An event
    /// adds its own as the fight opens. A listener hands a card back as it
    /// dies, and a card's play adds a card reward.
    pub(super) extra_rewards: Vec<ExtraReward<V>>,
    /// The copies whose `ExhaustOnNextPlay` is set, which their next play
    /// clears and so does the end of the turn.
    pub(super) exhaust_next: smallvec::SmallVec<[CardId<V>; 2]>,
    /// Whether a pushed card selector answers what the fight asks, which
    /// `WhisperingEarring` pushes as its `VakuuCardSelector` while it plays
    /// the hand. A question that comes up meanwhile never reaches the player.
    pub(super) selector_answers: bool,
    /// Whether the code running sits under a `WaitForPauseOrCompletion` that
    /// has not returned: `StartTurn`'s on `SetupPlayerTurn`. A choice
    /// signalled under it pauses the task there, and the caller runs on.
    pub(super) pause_boundary: bool,
    /// What `AfterCombatEnd` asked of the run, which carries it out as it
    /// takes the fight's rewards.
    pub(super) leaving: Vec<sts2_core::HookEffect<V>>,
    /// What the fight asked of the run in the middle of a decision, which the
    /// run carries out as the decision settles.
    pub(super) for_run: Vec<sts2_core::HookEffect<V>>,
    /// The rest of an `AfterShuffle` walk behind a listener that asked the
    /// player to pick, which waits for the pick.
    pub(super) after_pick: Option<Box<super::listen::Walking<V>>>,
    /// Each power taken off since the decision began, beside the creature
    /// that carried it. A walk that took its list before it came off still
    /// asks it, as a model on its own, and what it writes lands here.
    pub(super) detached: Vec<(CombatId, HeldPower<V>)>,
    /// Each monster `CombatState.CreateCreature` made that has not joined
    /// yet, which `CreatureCmd.Add` then puts in. None stands here across a
    /// decision.
    pub(super) created: Vec<Created<V>>,
    /// The draws a listener ran inside another draw, which a question
    /// stopped, with what the listeners after each asked. The outermost comes
    /// first. The draw that resumes finishes them before its own.
    pub(super) hook_draws: smallvec::SmallVec<[super::board::HookDraw<V>; 2]>,
    /// The card whose auto-play asked as it was drawn, and whether the draw
    /// was the hand's. The draw that resumes runs its late `AfterCardDrawn`
    /// pass first.
    pub(super) drawn_late: Option<(CardId<V>, bool)>,
    /// Every copy an effect's draw drew, once a question stood it still and
    /// it is over. The effect that waited on it resumes with them.
    pub(super) drew: Option<sts2_core::Drew<V>>,
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
    /// `AttackCommand.Results` of the attack whose `AfterAttack` is being
    /// raised, which a listener reads. Empty at any other time.
    pub(super) attack_results: Vec<sts2_core::DamageResult<V>>,
    /// The hits of the damage commands under way that have landed and wait
    /// for their command's last, the innermost last. Empty between effects,
    /// and kept here so its room outlasts each board.
    pub(super) landing: Vec<super::board::Landed<V>>,
    /// The last creature id this fight gave out. A creature that leaves keeps
    /// its id, and the next one to arrive takes the id after this.
    pub(super) last_creature: CombatId,
    /// A death or an escape left no primary enemy standing, and no decision
    /// has ended since. Only these two start a win, so the check at the end
    /// of a decision reads the fight again only once this is set.
    pub(super) won: bool,
    /// The player has died, which ends the fight lost.
    pub(super) lost: bool,
    /// Numbers a card keeps on its own model and prints nowhere, by copy,
    /// such as Regret's `CardsInHand`.
    pub(super) notes: smallvec::SmallVec<[(CardId<V>, V::Number); 2]>,
    /// A card play waiting on an answer, which is the only thing the fight
    /// will take while it is set.
    pub(super) asking: Option<Asking<V>>,
    /// What waits behind that answer.
    pub(super) deferred: Deferrals<V>,
    /// Questions a death's listener asked inside a play, each with what
    /// waits behind it, in the order they were asked. `Hook.AfterDeath`
    /// gives each listener a `HookPlayerChoiceContext`, whose question
    /// pauses only that listener. The play runs on, and the question is
    /// asked once the decision is over.
    pub(super) paused: Vec<Paused<V>>,
    /// The results of the last attack that ended while a question waited,
    /// or whose hits landed once it was answered. A card waiting behind its
    /// own attack reads them as it goes on.
    pub(super) waited_attack: Option<sts2_core::AttackResults<V>>,
}

/// How many history entries one turn holds before its list reaches the heap:
/// a hand's draws, and a few plays each started and finished.
const ENTRIES_INLINE: usize = 24;

/// `CombatManager.History`: the turn the fight stands in, and the turns
/// before it.
///
/// A turn joins the shared past as the sides switch. Copying a
/// fight copies this turn's few entries and one pointer.
pub(crate) struct CombatHistory<V: Rules> {
    this_turn: smallvec::SmallVec<[sts2_core::HistoryEntry<V>; ENTRIES_INLINE]>,
    before: Option<std::sync::Arc<sts2_core::PastTurns<V>>>,
}

impl<V: Rules> CombatHistory<V> {
    pub(super) fn new() -> Self {
        CombatHistory {
            this_turn: smallvec::SmallVec::new(),
            before: None,
        }
    }

    /// `CardPlaysFinished` where `HappenedThisTurn` and not `IsAutoPlay`, at
    /// most 255.
    pub(super) fn unprompted_plays_this_turn(&self) -> u8 {
        let plays = (self.this_turn.iter())
            .filter(|entry| {
                matches!(entry, sts2_core::HistoryEntry::Finished(finished) if !finished.auto)
            })
            .count();
        u8::try_from(plays).unwrap_or(u8::MAX)
    }

    /// `History.CardPlayStarted`, in the turn the fight stands in.
    pub(super) fn start(&mut self, card: V::Card, id: CardId<V>, play: CardPlay<V>, first: bool) {
        let started = sts2_core::PlayStarted {
            card,
            id,
            energy: play.energy,
            auto: play.auto,
            x: play.x,
            stars: play.stars,
            first,
        };
        self.this_turn
            .push(sts2_core::HistoryEntry::Started(started));
    }

    /// `History.CardPlayFinished`, in the turn the fight stands in.
    pub(super) fn finish(
        &mut self,
        card: V::Card,
        id: CardId<V>,
        ethereal: bool,
        auto: bool,
        dupe: bool,
    ) {
        let finished = sts2_core::PlayFinished {
            card,
            id,
            ethereal,
            auto,
            dupe,
        };
        self.this_turn
            .push(sts2_core::HistoryEntry::Finished(finished));
    }

    /// `History.CreatureAttacked`, in the turn the fight stands in.
    pub(super) fn attacked(&mut self, monster: Option<V::Monster>) {
        self.this_turn
            .push(sts2_core::HistoryEntry::Attacked(monster));
    }

    /// `History.EnergySpent`, in the turn the fight stands in, for a spend of
    /// more than nothing.
    pub(super) fn energy_spent(&mut self, amount: V::Count) {
        if amount > num_traits::zero() {
            self.this_turn
                .push(sts2_core::HistoryEntry::EnergySpent(amount));
        }
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

    /// `History.PowerReceived`, in the turn the fight stands in.
    pub(super) fn power_received(&mut self, power: V::Power, applier: Option<CombatId>) {
        self.this_turn
            .push(sts2_core::HistoryEntry::PowerReceived { power, applier });
    }

    /// `History.OrbChanneled`, in the turn the fight stands in.
    pub(super) fn channelled(&mut self, orb: V::Orb) {
        self.this_turn
            .push(sts2_core::HistoryEntry::Channelled(orb));
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
    /// over. Its entries join the past turns, unless it had none.
    fn end_turn(&mut self, round: V::Count, side: V::CombatSide, player_turn: V::Count) {
        if self.this_turn.is_empty() {
            return;
        }
        let past = self.before.get_or_insert_with(Default::default);
        std::sync::Arc::make_mut(past).end_turn(round, side, player_turn, self.this_turn.drain(..));
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
            ending_turn: self.ending_turn,
            effect_depth: self.effect_depth,
            point: self.point,
            deck_versions: self.deck_versions.clone(),
            taken_from_deck: self.taken_from_deck.clone(),
            extra_rewards: self.extra_rewards.clone(),
            exhaust_next: self.exhaust_next.clone(),
            selector_answers: self.selector_answers,
            pause_boundary: self.pause_boundary,
            history: self.history.clone(),
            kept: self.kept.clone(),
            leaving: self.leaving.clone(),
            for_run: self.for_run.clone(),
            after_pick: self.after_pick.clone(),
            detached: self.detached.clone(),
            created: self.created.clone(),
            hook_draws: self.hook_draws.clone(),
            drawn_late: self.drawn_late,
            drew: self.drew.clone(),
            powers: self.powers.clone(),
            orbs: self.orbs.clone(),
            card_ids: self.card_ids.clone(),
            kept_faces: self.kept_faces.clone(),
            attack_results: self.attack_results.clone(),
            landing: Vec::new(),
            notes: self.notes.clone(),
            asking: self.asking.clone(),
            deferred: self.deferred.clone(),
            paused: self.paused.clone(),
            waited_attack: self.waited_attack.clone(),
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
            potions,
            modifiers,
            round,
            side,
            history,
            kept,
            encounter,
            encounter_state,
            cards,
            removed_from_deck,
            ending_turn,
            in_progress,
            effect_depth,
            point,
            deck_versions,
            taken_from_deck,
            extra_rewards,
            exhaust_next,
            leaving,
            for_run,
            after_pick,
            detached,
            created,
            hook_draws,
            drawn_late,
            drew,
            creatures,
            powers,
            orbs,
            machines,
            skip_tick,
            card_ids,
            kept_faces,
            attacks,
            attack_results,
            landing,
            last_creature,
            won,
            lost,
            notes,
            asking,
            deferred,
            paused,
            waited_attack,
            selector_answers,
            pause_boundary,
        } = self;
        *character = source.character;
        *ascension = source.ascension;
        *turn = source.turn;
        *phase = source.phase;
        *energy = source.energy;
        *stars = source.stars;
        *gold = source.gold;
        *potions = source.potions;
        *modifiers = source.modifiers;
        *round = source.round;
        *side = source.side;
        history.clone_from(&source.history);
        kept.clone_from(&source.kept);
        *encounter = source.encounter;
        *encounter_state = source.encounter_state;
        cards.clone_from(&source.cards);
        removed_from_deck.clone_from(&source.removed_from_deck);
        *ending_turn = source.ending_turn;
        *in_progress = source.in_progress;
        *effect_depth = source.effect_depth;
        *point = source.point;
        deck_versions.clone_from(&source.deck_versions);
        taken_from_deck.clone_from(&source.taken_from_deck);
        extra_rewards.clone_from(&source.extra_rewards);
        exhaust_next.clone_from(&source.exhaust_next);
        *selector_answers = source.selector_answers;
        *pause_boundary = source.pause_boundary;
        leaving.clone_from(&source.leaving);
        for_run.clone_from(&source.for_run);
        after_pick.clone_from(&source.after_pick);
        detached.clone_from(&source.detached);
        created.clone_from(&source.created);
        hook_draws.clone_from(&source.hook_draws);
        *drawn_late = source.drawn_late;
        drew.clone_from(&source.drew);
        creatures.copy_from(&source.creatures);
        powers.clone_from(&source.powers);
        orbs.clone_from(&source.orbs);
        machines.copy_from(&source.machines);
        skip_tick.copy_from(&source.skip_tick);
        card_ids.clone_from(&source.card_ids);
        kept_faces.clone_from(&source.kept_faces);
        *attacks = source.attacks;
        attack_results.clone_from(&source.attack_results);
        landing.clear();
        *last_creature = source.last_creature;
        *won = source.won;
        *lost = source.lost;
        notes.clone_from(&source.notes);
        asking.clone_from(&source.asking);
        deferred.clone_from(&source.deferred);
        paused.clone_from(&source.paused);
        waited_attack.clone_from(&source.waited_attack);
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

    /// The run's belt, which now holds `potions`.
    pub(crate) fn hold_potions(&mut self, potions: &[V::Potion]) {
        self.potions = PotionBelt::holding(potions, self.modifiers.hooks);
    }

    /// Where in the belt each potion the fight spent stood, in the order it
    /// spent them, for the run to take out of its own.
    pub(crate) fn take_spent_potions(&mut self) -> Bounded<u8, BELT> {
        std::mem::take(&mut self.potions.spent)
    }

    /// The player's `Gold` as the fight left it.
    pub(crate) fn gold(&self) -> V::Number {
        self.gold
    }

    /// `CombatRoom.GoldProportion`, which `CalculateGoldProportion` sets as
    /// the fight ends.
    pub(crate) fn gold_proportion(&self) -> f32 {
        V::encounter_gold_proportion(
            self.encounter,
            &self.encounter_state,
            &self.creatures.escaped,
        )
        .unwrap_or_else(|| self.creatures.gold_proportion())
    }

    /// Whether any enemy escaped, which is what the dummy's
    /// `RanOutOfTime` records.
    pub(crate) fn any_escaped(&self) -> bool {
        self.creatures.any_escaped()
    }

    /// Whether a play asked for the turn to end.
    pub(crate) fn ending_turn(&self) -> bool {
        self.ending_turn
    }

    /// Clear a play's ask for the turn to end, as the turn ends.
    pub(crate) fn take_ending_turn(&mut self) {
        self.ending_turn = false;
    }

    pub(crate) fn removed_from_deck(&self) -> &[sts2_core::DeckCardId] {
        &self.removed_from_deck
    }

    /// Keep a deck card the run took out for one of this fight's removals.
    pub(crate) fn take_from_deck(&mut self, card: sts2_core::DeckCard<V>) {
        self.taken_from_deck.push(card);
    }

    /// The deck cards this fight took, as the run took them out.
    pub(crate) fn taken_from_deck(&self) -> &[sts2_core::DeckCard<V>] {
        &self.taken_from_deck
    }

    /// `CombatRoom.AddExtraReward`, which an event does as it builds its
    /// fight's room.
    pub(crate) fn add_extra_reward(&mut self, reward: ExtraReward<V>) {
        self.extra_rewards.push(reward);
    }

    /// The rewards this fight offers beside the room's own once it is won,
    /// in the order they were added.
    pub(crate) fn extra_rewards(&self) -> &[ExtraReward<V>] {
        &self.extra_rewards
    }

    /// Forget the powers taken off during the decision, unless a question
    /// waits, behind which a parked walk can still reach one.
    pub(crate) fn forget_detached(&mut self) {
        if self.asking.is_none() {
            self.detached.clear();
        }
    }

    /// What the fight's end asked of the run, in the order it asked.
    pub(crate) fn leaving(&self) -> &[sts2_core::HookEffect<V>] {
        &self.leaving
    }

    /// The task a pause boundary waited on, which paused on the question
    /// asked: the question, and what it deferred since `fresh` was opened.
    /// The boundary's caller runs on without it.
    pub(super) fn set_aside(&mut self, fresh: (usize, Option<usize>)) -> Paused<V> {
        let asking = self
            .asking
            .take()
            .unwrap_or_else(|| unreachable!("a task set aside paused on a question"));
        Paused {
            asking,
            waiting: self.deferred.take_fresh(fresh),
            hand: self.cards.len(sts2_core::Pile::Hand),
        }
    }

    /// A task [`Live::set_aside`] took, back where its caller waits for it:
    /// its question stands again, and what waits behind it runs next.
    ///
    /// # Panics
    ///
    /// Where the caller asked a question of its own as it ran on. Where a
    /// filtered choice signalled and the hand changed since: the choice
    /// listed the hand as it stopped, and the game lists it now.
    pub(super) fn rejoin(
        &mut self,
        Paused {
            asking,
            waiting,
            hand,
        }: Paused<V>,
    ) {
        if let Some(other) = &self.asking {
            unimplemented!("{other:?} asked while a paused task waited to rejoin");
        }
        if let Asked::Among { cards, .. } = &asking.question
            && asking.signalled
            && self.cards.len(sts2_core::Pile::Hand) != hand
        {
            unimplemented!("a filtered choice over {cards:?} whose hand changed as it paused");
        }
        self.asking = Some(asking);
        self.deferred.put_back(waiting);
    }

    /// Put `deferred` behind the question asked. See [`Deferrals::push`].
    pub(super) fn defer(&mut self, deferred: Deferred<V>) {
        self.deferred.push(deferred);
    }

    /// The first question a death's listener asked inside a play, once
    /// nothing else waits: the paused listener's `GameAction`, which runs
    /// after the play's own.
    #[inline]
    pub(crate) fn ask_paused(&mut self) {
        if self.paused.is_empty() {
            return;
        }
        self.ask_first_paused();
    }

    /// [`Live::ask_paused`] once a question waits behind the play.
    #[cold]
    fn ask_first_paused(&mut self) {
        if self.asking.is_some() || !self.deferred.is_empty() {
            return;
        }
        let task = self.paused.remove(0);
        self.rejoin(task);
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
        // The pile every option stands in, where they share one.
        let pile = match &asked.question {
            Asked::Pile { pile, .. } => Some(*pile),
            Asked::Among { cards, .. } => {
                let mut piles =
                    (cards.iter()).map(|card| self.cards.find_any(card.id()).map(|at| at.pile()));
                let first = piles.next().flatten();
                piles.all(|pile| pile == first).then_some(first).flatten()
            }
            Asked::Offered { .. } => None,
        };
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
            picked: sts2_core::Picks::none(),
            pile,
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

    /// The orb queue as the game reports it, each number its getter's
    /// `PassiveVal` or `EvokeVal`. The queue itself, while nothing listens.
    pub(crate) fn reported_orbs(
        &self,
        relics: &crate::run::Belt<V>,
    ) -> Option<std::borrow::Cow<'_, sts2_core::Orbs<V>>> {
        let orbs = self.orbs.as_ref()?;
        // No slot and no orb reads as no orbs at all, as for a character
        // that never had them, once Bulk Up takes the last slot.
        if num_traits::Zero::is_zero(&orbs.capacity) && orbs.queue.is_empty() {
            return None;
        }
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
    /// its turn. `OnSideSwitch` clears every monster's `SpawnedThisTurn`.
    pub(super) fn switch_side(&mut self, side: V::CombatSide) {
        self.history.end_turn(self.round, self.side, self.turn);
        self.side = side;
        for held in &mut self.machines {
            held.spawned_this_turn = false;
        }
    }

    /// `CurrentRoom.RoomType`, which is the encounter's.
    pub(crate) fn room(&self) -> V::RoomType {
        <V::Encounter as sts2_core::EncounterTable>::room(self.encounter)
    }

    /// The player, which every fight has one of.
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
            attack: &self.attack_results,
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

    /// [`Live::ask_power_as`] for the instance `instance` taken off `carrier`
    /// since the decision began, as a model on its own. False where there is
    /// none.
    pub(crate) fn ask_detached_as(
        &mut self,
        relics: Option<&crate::run::Belt<V>>,
        carrier: Carrier<V>,
        instance: PowerInstance,
        hook: sts2_core::EventHook<V>,
        asked: &mut sts2_core::HookEffects<V>,
    ) -> bool {
        let owner = carrier.id;
        let Some(at) = self
            .detached
            .iter()
            .position(|(holder, held)| *holder == owner && held.instance == instance)
        else {
            return false;
        };
        let held = self.detached[at].1;
        let mut state = held.state;
        let fight = Onlookers {
            creatures: &self.creatures,
            powers: &self.powers,
            kept: &self.kept,
            relics,
            attack: &self.attack_results,
        };
        V::power_event_hook(
            held.event_power(&mut state, carrier, self.history.view(), &fight),
            hook,
            asked,
        );
        self.detached[at].1.state = state;
        true
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
        let at = self.powers.of(owner).position(|held| held.power() == power);
        match at {
            Some(at) => self.remove_power_at_asking(owner, at),
            None => sts2_core::HookEffects::new(),
        }
    }

    /// [`Live::remove_power_asking`] for the instance at `at` of `owner`'s
    /// powers, which is how an instanced power takes itself off.
    pub(super) fn remove_power_at_asking(
        &mut self,
        owner: CombatId,
        at: usize,
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
        if at >= self.powers.of(owner).count() {
            return left;
        }
        let mut held = self.powers.remove(owner, at);
        let power = held.power();
        let mut asked = sts2_core::HookEffects::new();
        // Taken off already, as `AfterRemoved` finds it. No belt is lent here.
        let mut state = held.state;
        let fight = Onlookers {
            creatures: &self.creatures,
            powers: &self.powers,
            kept: &self.kept,
            relics: None,
            attack: &self.attack_results,
        };
        V::power_event_hook(
            held.event_power(&mut state, carrier, self.history.view(), &fight),
            sts2_core::EventHook::Removed,
            &mut asked,
        );
        // What it kept goes with it, once it has read it.
        self.kept
            .retain(|(holder, kept, _)| !(*holder == owner && *kept == power));
        held.state = state;
        self.detached.push((owner, held));
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
            ),
            orbs: self.reported_orbs(relics).map(std::borrow::Cow::into_owned),
        }
    }

    /// Consume a won fight into the report that remains on its reward screen.
    pub(crate) fn finish(&mut self, relics: &crate::run::Belt<V>) -> Combat<V> {
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
            potions: PotionBelt::holding(&[], sts2_core::Overrides::NONE),
            modifiers: RunModifiers::holding(&[]),
            round: 1,
            side: CombatSide::Player,
            history: CombatHistory::new(),
            kept: Vec::new(),
            encounter: Encounter::SlimesWeak,
            encounter_state:
                <_ as sts2_core::EncounterModel<sts2_content::v0_107_1::V0_107_1>>::new(
                    Encounter::SlimesWeak,
                ),
            cards: super::cards::Cards::with_cards(0),
            removed_from_deck: Vec::new(),
            ending_turn: false,
            in_progress: true,
            effect_depth: 0,
            point: None,
            deck_versions: DeckVersions::none(),
            taken_from_deck: Vec::new(),
            extra_rewards: Vec::new(),
            exhaust_next: smallvec::SmallVec::new(),
            selector_answers: false,
            pause_boundary: false,
            leaving: Vec::new(),
            for_run: Vec::new(),
            after_pick: None,
            detached: Vec::new(),
            created: Vec::new(),
            hook_draws: smallvec::SmallVec::new(),
            drawn_late: None,
            drew: None,
            creatures: creatures.into_iter().collect(),
            powers: PowerStore::new(),
            orbs: None,
            machines: Bounded::new(),
            skip_tick: Bounded::new(),
            card_ids: CardIds::new(),
            kept_faces: Vec::new(),
            attacks: 0,
            attack_results: Vec::new(),
            landing: Vec::new(),
            last_creature: CombatId::new(nonmax::NonMaxU8::ZERO),
            won: false,
            lost: false,
            notes: smallvec::SmallVec::new(),
            asking: None,
            deferred: Deferrals::new(),
            paused: Vec::new(),
            waited_attack: None,
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
        assert_eq!(
            size_of::<Face<V>>(),
            64,
            "a class's fields fit in four bytes"
        );
    }
}
