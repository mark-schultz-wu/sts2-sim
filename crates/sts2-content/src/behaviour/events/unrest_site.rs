//! Unrest Site: a rest that costs a Poor Sleep, or a relic for max HP.

use crate::ids::{CardIds as _, EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, RelicPull, Room};

/// `DynamicVar("MaxHpLoss", 8)`.
const MAX_HP_LOSS: i32 = 8;

#[derive(Debug, Clone)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct UnrestSite;

impl<V: crate::Build> EventBehavior<V> for UnrestSite {
    type State = State;

    /// `IsAllowed` is the health check the event table asks beside this.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `CalculateVars` sets the heal to what the player lacks of full
    /// health, and draws nothing.
    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State { chosen: None }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::UNREST_SITE,
            &[
                (V::EventOption::UNREST_SITE_REST, false),
                (V::EventOption::UNREST_SITE_KILL, false),
            ],
            state.chosen,
        )
    }

    /// `Rest`: a heal to full, then a Poor Sleep into the deck. `Kill`: its
    /// max HP lost, then the next relic pulled from the front. Each finishes
    /// the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        match option {
            0 => smallvec::smallvec![
                HookEffect::HealMissing,
                HookEffect::AddToDeck(V::Card::POOR_SLEEP),
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::LoseMaxHp(MAX_HP_LOSS),
                HookEffect::ObtainNextRelic(RelicPull::Rolled),
                HookEffect::FinishEvent,
            ],
        }
    }
}
