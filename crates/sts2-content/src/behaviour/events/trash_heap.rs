//! Trash Heap: a relic from the heap for HP, or gold and a card.

use crate::ids::{EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

/// `HpLossVar(8)`.
const HP_LOSS: i32 = 8;

/// `GoldVar(100)`.
const GOLD: i32 = 100;

#[derive(Debug, Clone)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct TrashHeap;

impl<V: crate::Build> EventBehavior<V> for TrashHeap {
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
        crate::behaviour::events::page::<V>(
            V::Event::TRASH_HEAP,
            &[
                (V::EventOption::TRASH_HEAP_DIVE_IN, false),
                (V::EventOption::TRASH_HEAP_GRAB, false),
            ],
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
                let relic = *rng
                    .choose(V::TRASH_HEAP_RELICS)
                    .expect("the heap holds relics");
                smallvec::smallvec![
                    HookEffect::DamageOwner(HP_LOSS),
                    HookEffect::ObtainRelic(relic),
                    HookEffect::FinishEvent,
                ]
            }
            _ => {
                let card = *rng
                    .choose(V::TRASH_HEAP_CARDS)
                    .expect("the heap holds cards");
                smallvec::smallvec![
                    HookEffect::GainGold(GOLD),
                    HookEffect::AddToDeck(card),
                    HookEffect::FinishEvent,
                ]
            }
        }
    }
}

/// `RELICS`, `CARDS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `Relics`, in the order the event lists them.
        const TRASH_HEAP_RELICS: &'static [<$build as sts2_core::ContentVersion>::Relic] = &[
            <$build as sts2_core::ContentVersion>::Relic::DARKSTONE_PERIAPT,
            <$build as sts2_core::ContentVersion>::Relic::DREAM_CATCHER,
            <$build as sts2_core::ContentVersion>::Relic::HAND_DRILL,
            <$build as sts2_core::ContentVersion>::Relic::MAW_BANK,
            <$build as sts2_core::ContentVersion>::Relic::THE_BOOT,
        ];
        /// `Cards`, in the order the event lists them.
        const TRASH_HEAP_CARDS: &'static [<$build as sts2_core::ContentVersion>::Card] = &[
            <$build as sts2_core::ContentVersion>::Card::CALTROPS,
            <$build as sts2_core::ContentVersion>::Card::CLASH,
            <$build as sts2_core::ContentVersion>::Card::DISTRACTION,
            <$build as sts2_core::ContentVersion>::Card::DUAL_WIELD,
            <$build as sts2_core::ContentVersion>::Card::ENTRENCH,
            <$build as sts2_core::ContentVersion>::Card::HELLO_WORLD,
            <$build as sts2_core::ContentVersion>::Card::OUTMANEUVER,
            <$build as sts2_core::ContentVersion>::Card::REBOUND,
            <$build as sts2_core::ContentVersion>::Card::RIP_AND_TEAR,
            <$build as sts2_core::ContentVersion>::Card::STACK,
        ];
    };
}

pub(crate) use lists;
