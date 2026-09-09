use sts2_core::{Answered, Applied, Construct, Decision, Play, Rules, Setup, Start};

use super::Error;

/// A run held entirely in this process.
#[derive(Debug)]
pub struct Sim<V: Rules> {
    pub(crate) current: Applied<V>,
    pub(crate) reward_seed: u32,
}

impl<V: Rules> Play<V> for Sim<V> {
    type Error = Error;
    type Host = ();

    fn begin(_: Self::Host, start: &Start<V>) -> Result<Self, Self::Error> {
        if !start.modifiers.is_empty() {
            return Err(Error::NotImplemented("run modifiers"));
        }
        Ok(Self::start(&start.seed, start.character, start.ascension))
    }

    fn play(&mut self, decision: &Decision<V>) -> Result<Answered<V>, Self::Error> {
        match decision {
            Decision::ChooseOption { index } => self.choose(*index).map(Ok),
            Decision::Answer(answer) => self.answer(answer).map(Ok),
            _ => Err(Error::NotImplemented(name(decision))),
        }
    }

    fn observe(&mut self) -> Result<Answered<V>, Self::Error> {
        Ok(Ok(self.current.clone()))
    }
}

impl<V: Rules> Construct<V> for Sim<V> {
    fn setup(&mut self, _: &Setup<V>) -> Result<Answered<V>, Self::Error> {
        Err(Error::NotImplemented("setup"))
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
