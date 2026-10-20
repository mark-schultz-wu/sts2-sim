//! `TheArchitect`, whose `PROCEED` here wins the run itself and then empties
//! the event: `RunManager.WinRun`, then `SetEventState` with no options.
//!
//! Its dialogue is the shared port's, which this hands everything else to.

use sts2_core::rng::GameRng;
use sts2_core::{EventBehavior, EventView, HookEffects, Room};

use super::super::super::V0_111_0 as V;
use crate::behaviour::events::the_architect;
use crate::ids::EventIds as _;

/// The shared dialogue, and whether `PROCEED` has won the run.
#[derive(Debug, Clone)]
pub struct State {
    dialogue: the_architect::State,
    won: bool,
}

pub struct TheArchitect;

impl EventBehavior<V> for TheArchitect {
    type State = State;

    fn allowed(&self, run: &impl EventView<V>) -> bool {
        the_architect::TheArchitect.allowed(run)
    }

    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> State {
        State {
            dialogue: the_architect::TheArchitect.enter(run, rng),
            won: false,
        }
    }

    /// The dialogue, until `PROCEED` empties the event.
    fn observe(&self, state: &State, run: &impl EventView<V>) -> Room<V> {
        if !state.won {
            return the_architect::TheArchitect.observe(&state.dialogue, run);
        }
        Room::Event {
            event: <V as sts2_core::ContentVersion>::Event::THE_ARCHITECT,
            finished: true,
            title: None,
            text: None,
            options: Vec::new(),
        }
    }

    /// The shared dialogue, and on `PROCEED` the run won.
    fn choose<R: GameRng>(
        &self,
        state: &mut State,
        option: u8,
        run: &impl EventView<V>,
        rng: &mut R,
    ) -> HookEffects<V> {
        state.won = state.dialogue.proceeding();
        the_architect::TheArchitect.choose(&mut state.dialogue, option, run, rng)
    }
}
