//! Trash Heap: a relic from the heap for HP, or gold and a card.

use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, EventCard};
use super::super::super::generated::events::{Event, EventOption as Said};
use super::super::super::generated::relics::Relic;

/// `HpLossVar(8)`.
const HP_LOSS: i32 = 8;

/// `GoldVar(100)`.
const GOLD: i32 = 100;

/// `Relics`, in the order the event lists them.
const RELICS: [Relic; 5] = [
    Relic::DarkstonePeriapt,
    Relic::DreamCatcher,
    Relic::HandDrill,
    Relic::MawBank,
    Relic::TheBoot,
];

/// `Cards`, in the order the event lists them.
const CARDS: [EventCard; 10] = [
    EventCard::Caltrops,
    EventCard::Clash,
    EventCard::Distraction,
    EventCard::DualWield,
    EventCard::Entrench,
    EventCard::HelloWorld,
    EventCard::Outmaneuver,
    EventCard::Rebound,
    EventCard::RipAndTear,
    EventCard::Stack,
];

#[derive(Debug, Clone)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct TrashHeap;

impl EventBehavior<V> for TrashHeap {
    type State = State;

    /// The event's own check, `CurrentHp > 5`, sits with the others in the
    /// event table.
    fn allowed(&self, run: &impl EventView<V>) -> bool {
        run.hp() > 5
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State { chosen: None }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::TrashHeap,
            &[(Said::TrashHeapDiveIn, false), (Said::TrashHeapGrab, false)],
            state.chosen,
        )
    }

    /// `DiveIn`: the damage, then one relic of `Relics` off the event's
    /// stream. `Grab`: the gold, then one card of `Cards` off the same
    /// stream, into the deck. Each finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        match option {
            0 => {
                let relic = *rng.choose(&RELICS).expect("the heap holds relics");
                smallvec::smallvec![
                    HookEffect::DamageOwner(HP_LOSS),
                    HookEffect::ObtainRelic(relic),
                    HookEffect::FinishEvent,
                ]
            }
            _ => {
                let card = *rng.choose(&CARDS).expect("the heap holds cards");
                smallvec::smallvec![
                    HookEffect::GainGold(GOLD),
                    HookEffect::AddToDeck(Cards::Event(card)),
                    HookEffect::FinishEvent,
                ]
            }
        }
    }
}
