//! Doll Room: one of three doll relics, at random for free, or picked from two
//! or all three for HP.

use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::events::{Event, EventOption as Said};
use super::super::super::generated::relics::Relic;
use super::ancient::{Offered, next_item};

/// `_dolls`, in the order the event lists them.
const DOLLS: [Relic; 3] = [
    Relic::DaughterOfTheWind,
    Relic::MrStruggles,
    Relic::BingBong,
];

/// `_dolls` in the order `StableShuffle` sorts them first, which is by id.
const DOLLS_BY_ID: [Relic; 3] = [
    Relic::BingBong,
    Relic::DaughterOfTheWind,
    Relic::MrStruggles,
];

/// `DamageVar("TakeTimeHpLoss", 5)`.
const TAKE_TIME_HP_LOSS: i32 = 5;

/// `DamageVar("ExamineHpLoss", 15)`.
const EXAMINE_HP_LOSS: i32 = 15;

#[derive(Debug, Clone)]
pub enum State {
    /// The first page, and the option taken there.
    Initial { chosen: Option<NonMaxU8> },
    /// The dolls a page of relics offers.
    Dolls(Offered),
}

pub struct DollRoom;

impl EventBehavior<V> for DollRoom {
    type State = State;

    /// Whether this is the second act, which the shared eligibility check
    /// answers.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::Initial { chosen: None }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        match state {
            State::Initial { chosen } => super::page(
                Event::DollRoom,
                &[
                    (Said::DollRoomRandom, false),
                    (Said::DollRoomTakeSomeTime, false),
                    (Said::DollRoomExamine, false),
                ],
                *chosen,
            ),
            State::Dolls(offered) => offered.observe(Event::DollRoom),
        }
    }

    /// `Random`: `Rng.NextItem` over the dolls, obtained, which finishes the
    /// event. `TakeSomeTime` and `Examine`: the damage, then the dolls
    /// `StableShuffle`d off the event's stream, the first two or all three.
    /// A doll's option obtains it and finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        rng: &mut R,
    ) -> HookEffects<V> {
        match state {
            State::Initial { chosen } => match option {
                0 => {
                    *chosen = NonMaxU8::new(option);
                    let doll = next_item(rng, &DOLLS).expect("there are dolls");
                    smallvec::smallvec![HookEffect::ObtainRelic(doll), HookEffect::FinishEvent]
                }
                _ => {
                    let (damage, shown) = match option {
                        1 => (TAKE_TIME_HP_LOSS, 2),
                        _ => (EXAMINE_HP_LOSS, DOLLS.len()),
                    };
                    let mut dolls = DOLLS_BY_ID;
                    rng.unstable_shuffle(&mut dolls);
                    *state = State::Dolls(Offered::new(dolls[..shown].to_vec()));
                    smallvec::smallvec![HookEffect::DamageOwner(damage)]
                }
            },
            State::Dolls(offered) => offered.choose(option),
        }
    }
}
