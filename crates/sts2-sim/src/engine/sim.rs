use sts2_core::{
    Answered, Construct, Decision, Play, Reason, Refusal, Rules, Setup, Snapshot, Start,
};

use crate::run::State;

use crate::combat::Error;

/// A run held entirely in this process.
///
/// Cloning one branches the run. The setup a run drew before play began is
/// shared by every branch, so a branch copies only what play can change.
#[derive(Debug, Clone)]
pub struct Sim<V: Rules> {
    state: State<V>,
}

impl<V: Rules> Play<V> for Sim<V> {
    type Error = Error;
    type Host = ();

    fn begin(_: Self::Host, start: &Start<V>) -> Result<Self, Self::Error> {
        if !start.modifiers.is_empty() {
            unimplemented!("a run started with modifiers");
        }
        Ok(Self {
            state: State::start(&start.seed, start.character, start.ascension),
        })
    }

    fn play(&mut self, decision: &Decision<V>) -> Result<Answered<V>, Self::Error> {
        let pending = self.state.pending();
        if let Some(kind) = pending
            && !matches!(decision, Decision::Answer(_))
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
            Decision::Answer(answer) => self.state.answer(answer).map(Ok),
            Decision::PlayCard { index, target } => self.state.play_card(*index, *target),
            Decision::EndTurn => self.state.end_turn(),
            Decision::UsePotion { index, target } => self.state.use_potion(*index, *target),
            Decision::Travel { to } => match self.state.resolve(*to) {
                Err(reason) => Ok(Err(Refusal {
                    what: "travel".into(),
                    reason,
                })),
                Ok(moving) => self.state.travel(moving).map(Ok),
            },
            _ => unimplemented!("{}", name(decision)),
        }
    }

    fn observe(&mut self) -> Result<Snapshot<V>, Self::Error> {
        Ok(self.state.report().snapshot())
    }
}

impl<V: Rules> Construct<V> for Sim<V> {
    /// Reaching a position without playing to it.
    ///
    /// Every one of these is a debug command the game itself has, so a state
    /// reached through them is a state the game reaches. That is what makes a
    /// constructed scenario worth holding the simulator to.
    fn setup(&mut self, step: &Setup<V>) -> Result<Answered<V>, Self::Error> {
        match step {
            Setup::Fight { encounter } => self.state.fight_here(*encounter).map(Ok),
            Setup::Grant(gift) => self.state.grant(gift).map(Ok),
            Setup::Power { at, power, amount } => {
                self.state.apply_power(*at, *power, *amount).map(Ok)
            }
            Setup::Event { event } => unimplemented!("jumping to {event:?}"),
        }
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
    }
}
