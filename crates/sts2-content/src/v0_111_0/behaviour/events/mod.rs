//! The events whose rules this build ports for itself, which the shared
//! events hand over through [`crate::build::OwnRules`].

mod amalgamator;
mod battleworn_dummy;
mod the_architect;
mod tinker_time;

use sts2_core::rng::GameRng;
use sts2_core::{EventBehavior, EventView, HookEffects, Room};

use super::super::V0_111_0 as V;
use super::super::generated::events::Event;
use super::super::generated::relics::Relic;

/// An event of the build's own, as it stands.
#[derive(Debug, Clone)]
pub enum State {
    Amalgamator(amalgamator::State),
    BattlewornDummy(battleworn_dummy::State),
    TheArchitect(the_architect::State),
    TinkerTime(tinker_time::State),
}

/// The build's own events, as one behaviour over their states.
#[derive(Debug, Clone, Default)]
pub struct OwnEvents;

/// `IsAllowed` for an event of the build's own, or nothing for a shared one.
pub(crate) fn allowed(event: Event, run: &impl EventView<V>) -> Option<bool> {
    match event {
        // `BattlewornDummy`'s encounters and rewards changed in this build.
        Event::BattlewornDummy => Some(battleworn_dummy::BattlewornDummy.allowed(run)),
        Event::TheArchitect => Some(the_architect::TheArchitect.allowed(run)),
        Event::TinkerTime => Some(tinker_time::TinkerTime.allowed(run)),
        _ => None,
    }
}

/// An event of the build's own, entered, or nothing for a shared one.
pub(crate) fn enter<R: GameRng>(
    event: Event,
    run: &impl EventView<V>,
    rng: &mut R,
) -> Option<State> {
    Some(match event {
        Event::Amalgamator => State::Amalgamator(amalgamator::Amalgamator.enter(run, rng)),
        Event::BattlewornDummy => {
            State::BattlewornDummy(battleworn_dummy::BattlewornDummy.enter(run, rng))
        }
        Event::TheArchitect => State::TheArchitect(the_architect::TheArchitect.enter(run, rng)),
        Event::TinkerTime => State::TinkerTime(tinker_time::TinkerTime.enter(run, rng)),
        _ => return None,
    })
}

impl EventBehavior<V> for OwnEvents {
    type State = State;

    /// The shared events ask [`allowed`], which knows the event.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        unreachable!("the shared events ask whether one of the build's own is allowed by its id")
    }

    /// The shared events ask [`enter`], which knows the event.
    fn enter<R: GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        unreachable!("the shared events enter one of the build's own by its id")
    }

    fn set_up<R: GameRng>(&self, state: &mut State, run: &impl EventView<V>, rewards: &mut R) {
        match state {
            State::Amalgamator(state) => amalgamator::Amalgamator.set_up(state, run, rewards),
            State::BattlewornDummy(state) => {
                battleworn_dummy::BattlewornDummy.set_up(state, run, rewards)
            }
            State::TheArchitect(state) => the_architect::TheArchitect.set_up(state, run, rewards),
            State::TinkerTime(state) => tinker_time::TinkerTime.set_up(state, run, rewards),
        }
    }

    fn stock<R: GameRng>(&self, state: &mut State, shops: &mut R) {
        match state {
            State::Amalgamator(state) => amalgamator::Amalgamator.stock(state, shops),
            State::BattlewornDummy(state) => battleworn_dummy::BattlewornDummy.stock(state, shops),
            State::TheArchitect(state) => the_architect::TheArchitect.stock(state, shops),
            State::TinkerTime(state) => tinker_time::TinkerTime.stock(state, shops),
        }
    }

    fn buy(&self, state: &mut State, index: u8) -> HookEffects<V> {
        match state {
            State::Amalgamator(state) => amalgamator::Amalgamator.buy(state, index),
            State::BattlewornDummy(state) => battleworn_dummy::BattlewornDummy.buy(state, index),
            State::TheArchitect(state) => the_architect::TheArchitect.buy(state, index),
            State::TinkerTime(state) => tinker_time::TinkerTime.buy(state, index),
        }
    }

    fn relics_pulled(&self, state: &State) -> &'static [sts2_core::RelicPull<V>] {
        match state {
            State::Amalgamator(state) => amalgamator::Amalgamator.relics_pulled(state),
            State::BattlewornDummy(state) => battleworn_dummy::BattlewornDummy.relics_pulled(state),
            State::TheArchitect(state) => the_architect::TheArchitect.relics_pulled(state),
            State::TinkerTime(state) => tinker_time::TinkerTime.relics_pulled(state),
        }
    }

    fn hand_pulled_relics(&self, state: &mut State, relics: &[Relic]) {
        match state {
            State::Amalgamator(state) => amalgamator::Amalgamator.hand_pulled_relics(state, relics),
            State::BattlewornDummy(state) => {
                battleworn_dummy::BattlewornDummy.hand_pulled_relics(state, relics)
            }
            State::TheArchitect(state) => {
                the_architect::TheArchitect.hand_pulled_relics(state, relics)
            }
            State::TinkerTime(state) => tinker_time::TinkerTime.hand_pulled_relics(state, relics),
        }
    }

    fn observe(&self, state: &State, run: &impl EventView<V>) -> Room<V> {
        match state {
            State::Amalgamator(state) => amalgamator::Amalgamator.observe(state, run),
            State::BattlewornDummy(state) => battleworn_dummy::BattlewornDummy.observe(state, run),
            State::TheArchitect(state) => the_architect::TheArchitect.observe(state, run),
            State::TinkerTime(state) => tinker_time::TinkerTime.observe(state, run),
        }
    }

    fn choose<R: GameRng>(
        &self,
        state: &mut State,
        option: u8,
        run: &impl EventView<V>,
        rng: &mut R,
    ) -> HookEffects<V> {
        match state {
            State::Amalgamator(state) => amalgamator::Amalgamator.choose(state, option, run, rng),
            State::BattlewornDummy(state) => {
                battleworn_dummy::BattlewornDummy.choose(state, option, run, rng)
            }
            State::TheArchitect(state) => {
                the_architect::TheArchitect.choose(state, option, run, rng)
            }
            State::TinkerTime(state) => tinker_time::TinkerTime.choose(state, option, run, rng),
        }
    }

    fn question(&self, state: &State) -> Option<sts2_core::Question<V>> {
        match state {
            State::Amalgamator(state) => amalgamator::Amalgamator.question(state),
            State::BattlewornDummy(state) => battleworn_dummy::BattlewornDummy.question(state),
            State::TheArchitect(state) => the_architect::TheArchitect.question(state),
            State::TinkerTime(state) => tinker_time::TinkerTime.question(state),
        }
    }

    fn answer(&self, state: &mut State, answer: &sts2_core::Answer) -> HookEffects<V> {
        match state {
            State::Amalgamator(state) => amalgamator::Amalgamator.answer(state, answer),
            State::BattlewornDummy(state) => {
                battleworn_dummy::BattlewornDummy.answer(state, answer)
            }
            State::TheArchitect(state) => the_architect::TheArchitect.answer(state, answer),
            State::TinkerTime(state) => tinker_time::TinkerTime.answer(state, answer),
        }
    }

    fn resume<R: GameRng>(
        &self,
        state: &mut State,
        run: &impl EventView<V>,
        rng: &mut R,
    ) -> HookEffects<V> {
        match state {
            State::Amalgamator(state) => amalgamator::Amalgamator.resume(state, run, rng),
            State::BattlewornDummy(state) => {
                battleworn_dummy::BattlewornDummy.resume(state, run, rng)
            }
            State::TheArchitect(state) => the_architect::TheArchitect.resume(state, run, rng),
            State::TinkerTime(state) => tinker_time::TinkerTime.resume(state, run, rng),
        }
    }

    fn resume_from_fight(&self, state: &mut State, escaped: bool) -> HookEffects<V> {
        match state {
            State::Amalgamator(state) => amalgamator::Amalgamator.resume_from_fight(state, escaped),
            State::BattlewornDummy(state) => {
                battleworn_dummy::BattlewornDummy.resume_from_fight(state, escaped)
            }
            State::TheArchitect(state) => {
                the_architect::TheArchitect.resume_from_fight(state, escaped)
            }
            State::TinkerTime(state) => tinker_time::TinkerTime.resume_from_fight(state, escaped),
        }
    }

    fn rewards(&self, state: &State) -> Vec<sts2_core::CustomReward<V>> {
        match state {
            State::Amalgamator(state) => amalgamator::Amalgamator.rewards(state),
            State::BattlewornDummy(state) => battleworn_dummy::BattlewornDummy.rewards(state),
            State::TheArchitect(state) => the_architect::TheArchitect.rewards(state),
            State::TinkerTime(state) => tinker_time::TinkerTime.rewards(state),
        }
    }
}
