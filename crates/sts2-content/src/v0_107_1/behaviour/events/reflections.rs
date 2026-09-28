//! Reflections: two upgrades undone for four made, or the deck doubled.

use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room, Stream};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, CurseCard};
use super::super::super::generated::events::{Event, EventOption as Said};

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

impl EventBehavior<V> for Reflections {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::Reflections,
            &[
                (Said::ReflectionsTouchAMirror, false),
                (Said::ReflectionsShatter, false),
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
                HookEffect::AddToDeck(Cards::Curse(CurseCard::BadLuck)),
                HookEffect::FinishEvent,
            ],
        }
    }
}
