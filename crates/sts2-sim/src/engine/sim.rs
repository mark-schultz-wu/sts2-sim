use sts2_core::{
    Answered, Construct, Decision, Play, Reason, Refusal, Rules, Setup, Snapshot, Start, Step,
};

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

impl<V: Rules> Play<V> for Sim<V> {
    type Error = Error;
    type Host = ();

    fn begin(_: Self::Host, start: &Start<V>) -> Result<Self, Self::Error> {
        if let Some(modifier) = start
            .modifiers
            .iter()
            .find(|m| !V::modifier_ported(m.modifier))
        {
            unimplemented!("a run started with {modifier:?}");
        }
        Ok(Self {
            state: State::start(
                &start.seed,
                start.character,
                start.ascension,
                &start.modifiers,
                start.wins,
            ),
        })
    }

    fn play(&mut self, decision: &Decision<V>) -> Result<Answered<V>, Self::Error> {
        Ok(self.decide(decision)?.map(|decided| self.reported(decided)))
    }

    fn observe(&mut self) -> Result<Snapshot<V>, Self::Error> {
        Ok(self.state.report())
    }
}

impl<V: Rules> Sim<V> {
    /// The run's facts, lent to a comparison.
    ///
    /// What [`Play::observe`] reports, read off the run instead of built. A
    /// digest of this is the digest of that report, and costs no allocation.
    pub fn observed(&self) -> &impl sts2_core::Observed<V> {
        &self.state
    }

    /// Every decision the run accepts right now, in the order a driver picks
    /// from.
    ///
    /// The moves the observation lists, read off the run instead. A search
    /// asks this at every node, and builds nothing but the moves.
    pub fn legal(&self) -> crate::Legal<V> {
        self.state.legal()
    }

    /// [`Sim::legal`], written over `moves`. A search that asks at every node
    /// keeps one list for this. A returned list is copied out, and it is over
    /// a kilobyte.
    pub fn legal_into(&self, moves: &mut crate::Legal<V>) {
        moves.clear();
        self.state.legal_into(moves);
    }

    /// Plays one decision and reports what it decided, and nothing more.
    ///
    /// A search steps through here. Building the observation costs more than
    /// most decisions do, and a search throws every one of them away.
    #[tracing::instrument(level = "trace", skip_all, fields(decision = name(decision)))]
    pub fn decide(&mut self, decision: &Decision<V>) -> Result<Settled<V>, Error> {
        let pending = self.state.pending();
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
            Decision::ChooseOption { index } => self.state.choose(*index).map(Ok),
            Decision::Answer(answer) => self.state.respond(answer),
            Decision::PlayCard { index, target } => self.state.play_card(*index, *target),
            Decision::EndTurn => self.state.end_turn(),
            Decision::UsePotion { index, target } => self.state.use_potion(*index, *target),
            Decision::Proceed => self.state.proceed().map(Ok),
            Decision::Buy { index } => self.state.buy(*index).map(Ok),
            Decision::DiscardPotion { index } => self.state.discard_potion(*index),
            Decision::EnterRoom { room } => self.state.enter_room(*room).map(Ok),
            Decision::Travel { to } => match self.state.resolve(*to) {
                Err(reason) => Ok(Err(Refusal {
                    what: "travel".into(),
                    reason,
                })),
                Ok(moving) => self.state.travel(moving).map(Ok),
            },
        }
    }

    /// Puts the run into a position, and reports what that decided.
    ///
    /// [`Sim::decide`], for the debug commands rather than the decisions.
    pub fn construct(&mut self, step: &Setup<V>) -> Result<Settled<V>, Error> {
        match step {
            Setup::Fight { encounter } => self.state.fight_here(*encounter).map(Ok),
            Setup::Grant(gift) => self.state.grant(gift).map(Ok),
            Setup::Power { at, power, amount } => {
                self.state.apply_power(*at, *power, *amount).map(Ok)
            }
            Setup::Enchant {
                enchantment,
                amount,
                index,
            } => self.state.enchant(*enchantment, *amount, *index),
            Setup::Event { event } => self.state.event_here(*event).map(Ok),
            Setup::Hp { at, hp } => self.state.set_hp(*at, *hp).map(Ok),
        }
    }

    /// Plays one line of a recording and reports what it decided.
    ///
    /// [`Construct::step`], without the observation. A search that replays a
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
    /// hands back a [`Decided`] alone, so nothing pays for this but a caller
    /// that asked to see where it landed.
    fn reported(&self, decided: sts2_core::Decided) -> sts2_core::Applied<V> {
        self.state.report().applied(decided)
    }
}

impl<V: Rules> Construct<V> for Sim<V> {
    /// Reaching a position without playing to it.
    ///
    /// Every one of these is a debug command the game itself has, so a state
    /// reached through them is a state the game reaches. That is what makes a
    /// constructed scenario worth holding the simulator to.
    fn setup(&mut self, step: &Setup<V>) -> Result<Answered<V>, Self::Error> {
        Ok(self.construct(step)?.map(|decided| self.reported(decided)))
    }
}

fn name<V: Rules>(decision: &Decision<V>) -> &'static str {
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
