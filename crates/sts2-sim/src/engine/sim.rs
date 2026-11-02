use sts2_core::{Answered, Decision, Reason, Refusal, Rules, Setup, Snapshot, Start, Step};

use crate::run::{Settled, State};

use crate::combat::Error;

/// A run held entirely in this process.
///
/// Cloning one branches the run. The setup a run drew before play began is
/// shared by every branch, so a branch copies only what play can change.
#[derive(Debug)]
pub struct Sim<V: Rules> {
    state: State<V>,
}

/// A search branches with `clone_from`, into a node it already has. A fight
/// is written into that node without allocating.
impl<V: Rules> Clone for Sim<V> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.state.clone_from(&source.state);
    }
}

/// The one seat of a solo run, which is every seat `Sim` plays.
const SOLO: sts2_core::SeatIndex = sts2_core::SeatIndex(0);

impl<V: Rules> Sim<V> {
    /// The solo run `start` names, as it opens.
    ///
    /// # Panics
    ///
    /// On a run of several players, which a [`crate::SimTable`] holds.
    pub fn begin(start: &Start<V>) -> Result<Self, Error> {
        start.alone();
        Ok(Self {
            state: started(start),
        })
    }

    /// Plays one decision and reports where it left the run.
    pub fn play(&mut self, decision: &Decision<V>) -> Result<Answered<V>, Error> {
        Ok(self.decide(decision)?.map(|decided| self.reported(decided)))
    }

    /// Where the run stands, built in full.
    pub fn observe(&mut self) -> Result<Snapshot<V>, Error> {
        Ok(self.state.report(SOLO))
    }

    /// Puts the run into a position, and reports where that left it. Every
    /// one of these is a debug command the game itself has, so a state reached
    /// through them is a state the game reaches.
    pub fn setup(&mut self, step: &Setup<V>) -> Result<Answered<V>, Error> {
        Ok(self.construct(step)?.map(|decided| self.reported(decided)))
    }

    /// Plays one line of a recording, whichever half it is, and reports where
    /// it left the run.
    pub fn step(&mut self, step: &Step<V>) -> Result<Answered<V>, Error> {
        match step {
            Step::Decision(decision) => self.play(decision),
            Step::Setup(setup) => self.setup(setup),
        }
    }

    /// The run's facts, lent to a comparison.
    ///
    /// What [`Sim::observe`] reports, read off the run instead of built. A
    /// digest of this is the digest of that report, and costs no allocation.
    pub fn observed(&self) -> impl sts2_core::Observed<V> + Copy + '_ {
        self.state.seen_by(SOLO)
    }

    /// Every decision the run accepts right now, in the order a driver picks
    /// from.
    ///
    /// The moves the observation lists, read off the run instead. A search
    /// asks this at every node, and builds nothing but the moves.
    pub fn legal(&self) -> crate::Legal<V> {
        self.state.legal(SOLO)
    }

    /// [`Sim::legal`], written over `moves`. A search that asks at every node
    /// keeps one list for this. A returned list is copied out, and it is over
    /// a kilobyte.
    pub fn legal_into(&self, moves: &mut crate::Legal<V>) {
        moves.clear();
        self.state.legal_into(SOLO, moves);
    }

    /// Plays one decision and reports what it decided, and nothing more.
    ///
    /// A search steps through here. Building the observation costs more than
    /// most decisions do, and a search throws every one of them away.
    #[tracing::instrument(level = "trace", skip_all, fields(decision = name(decision)))]
    pub fn decide(&mut self, decision: &Decision<V>) -> Result<Settled<V>, Error> {
        decide(&mut self.state, SOLO, decision)
    }

    /// Puts the run into a position, and reports what that decided.
    ///
    /// [`Sim::decide`], for the debug commands rather than the decisions.
    pub fn construct(&mut self, step: &Setup<V>) -> Result<Settled<V>, Error> {
        constructed(&mut self.state, SOLO, step)
    }

    /// Plays one line of a recording and reports what it decided.
    ///
    /// [`Sim::step`], without the observation. A search that replays a
    /// recording to reach its position steps through here.
    pub fn advance(&mut self, step: &Step<V>) -> Result<Settled<V>, Error> {
        match step {
            Step::Decision(decision) => self.decide(decision),
            Step::Setup(setup) => self.construct(setup),
        }
    }

    /// Where the run is standing, beside what the decision decided.
    ///
    /// The one place an observation is built for a decision. Every command
    /// hands back a [`sts2_core::Decided`] alone, so nothing pays for this but a caller
    /// that asked to see where it landed.
    fn reported(&self, decided: sts2_core::Decided) -> sts2_core::Applied<V> {
        self.state.report(SOLO).applied(decided)
    }
}

/// The run `start` names, as it opens.
///
/// # Panics
///
/// A modifier nobody has ported.
pub(super) fn started<V: Rules>(start: &Start<V>) -> State<V> {
    if let Some(modifier) = start
        .modifiers
        .iter()
        .find(|m| !V::modifier_ported(m.modifier))
    {
        unimplemented!("a run started with {modifier:?}");
    }
    State::start(
        &start.seed,
        &start.players,
        start.ascension,
        &start.modifiers,
    )
}

/// `decision`, taken by `seat`'s player, on `state`.
pub(super) fn decide<V: Rules>(
    state: &mut State<V>,
    seat: sts2_core::SeatIndex,
    decision: &Decision<V>,
) -> Result<Settled<V>, Error> {
    // What another player does on a screen of their own machine, which
    // this one never shows. Its results reach this machine as messages.
    if state.answers_elsewhere(seat, decision) {
        return Ok(Ok(sts2_core::Decided::Resolved));
    }
    let pending = state.pending(seat);
    // A reward screen leaves the belt open, so a discard is the one
    // other decision it takes.
    if let Some(kind) = pending
        && !matches!(decision, Decision::Answer(_))
        && !(matches!(decision, Decision::DiscardPotion { .. })
            && kind == sts2_core::QuestionKind::Rewards)
    {
        return Ok(Err(Refusal {
            what: name(decision).into(),
            reason: Reason::Pending { kind },
        }));
    }
    if pending.is_none() && matches!(decision, Decision::Answer(_)) {
        return Ok(Err(Refusal {
            what: "answer".into(),
            reason: Reason::NothingPending,
        }));
    }
    match decision {
        Decision::ChooseOption {
            index,
            target: None,
        } => state.choose(seat, *index).map(Ok),
        // A run of one has no other player to aim an option at.
        Decision::ChooseOption {
            target: Some(target),
            ..
        } if state.player_count() == 1 => Ok(Err(Refusal {
            what: "choose_option".into(),
            reason: Reason::WrongOptionTarget {
                target: Some(*target),
            },
        })),
        Decision::ChooseOption {
            index,
            target: Some(target),
        } => state.mend(seat, *index, *target).map(Ok),
        Decision::Answer(answer) => state.respond(seat, answer),
        Decision::PlayCard { index, target } => state.play_card(seat, *index, *target),
        Decision::EndTurn => state.end_turn(seat),
        Decision::UsePotion { index, target } => state.use_potion(seat, *index, *target),
        Decision::Proceed => state.proceed(seat).map(Ok),
        Decision::Buy { index } => state.buy(seat, *index).map(Ok),
        Decision::DiscardPotion { index } => state.discard_potion(seat, *index),
        Decision::EnterRoom { room } => state.enter_room(*room).map(Ok),
        Decision::Travel { to } => match state.resolve(seat, *to) {
            Err(reason) => Ok(Err(Refusal {
                what: "travel".into(),
                reason,
            })),
            // A run of several players votes, and travels once all have.
            Ok(_) if state.player_count() > 1 => state.vote(seat, *to).map(Ok),
            Ok(moving) => state.travel(seat, moving).map(Ok),
        },
    }
}

pub(super) fn name<V: Rules>(decision: &Decision<V>) -> &'static str {
    match decision {
        Decision::Travel { .. } => "travel",
        Decision::EnterRoom { .. } => "enter_room",
        Decision::PlayCard { .. } => "play_card",
        Decision::EndTurn => "end_turn",
        Decision::UsePotion { .. } => "use_potion",
        Decision::Proceed => "proceed",
        Decision::ChooseOption { .. } => "choose_option",
        Decision::Buy { .. } => "buy",
        Decision::Answer(_) => "answer",
        Decision::DiscardPotion { .. } => "discard_potion",
    }
}

/// `step`, on the machine of the player in `me`, whose own player a setup
/// that names none is for.
pub(super) fn constructed<V: Rules>(
    state: &mut State<V>,
    me: sts2_core::SeatIndex,
    step: &Setup<V>,
) -> Result<Settled<V>, Error> {
    match step {
        Setup::Fight { encounter } => state.fight_here(me, *encounter).map(Ok),
        Setup::Grant(gift) => state.grant(me, gift).map(Ok),
        Setup::GrantTo { seat, gift } => state.grant(*seat, gift).map(Ok),
        Setup::Power { at, power, amount } => state.apply_power(me, *at, *power, *amount).map(Ok),
        Setup::Enchant {
            enchantment,
            amount,
            index,
        } => state.enchant(me, *enchantment, *amount, *index),
        Setup::Event { event } => state.event_here(*event).map(Ok),
        Setup::Hp { at, hp } => state.set_hp(me, *at, *hp).map(Ok),
    }
}
