//! Reflections: two upgrades undone for four made, or the deck doubled.

use crate::ids::{CardIds as _, EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room, Stream};

/// `TouchAMirror`: the upgraded cards it downgrades, then the cards it
/// upgrades.
const DOWNGRADED: usize = 2;
const UPGRADED: usize = 4;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct Reflections;

impl<V: crate::Build> EventBehavior<V> for Reflections {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::REFLECTIONS,
            &[
                (V::EventOption::REFLECTIONS_TOUCH_AMIRROR, false),
                (V::EventOption::REFLECTIONS_SHATTER, false),
            ],
            state.chosen,
        )
    }

    /// `TouchAMirror`: `NextItem` off the event's stream over the upgraded
    /// cards twice, each downgraded, then over the upgradable cards four
    /// times, each upgraded. Every card upgrades once, so the lists the game
    /// draws from are the deck's as each draw is made. `Shatter`: a clone of
    /// each deck card, then Bad Luck. Each finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let stream = Stream::Event;
        match option {
            0 => std::iter::repeat_n(HookEffect::DowngradeRandomDeckCard { stream }, DOWNGRADED)
                .chain(std::iter::repeat_n(
                    HookEffect::UpgradeRandomDeckCard { stream },
                    UPGRADED,
                ))
                .chain([HookEffect::FinishEvent])
                .collect(),
            _ => smallvec::smallvec![
                HookEffect::CloneDeck,
                HookEffect::AddToDeck(V::Card::BAD_LUCK),
                HookEffect::FinishEvent,
            ],
        }
    }
}
