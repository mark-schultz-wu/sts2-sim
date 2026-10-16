//! Doll Room: one of three doll relics, at random for free, or picked from two
//! or all three for HP.

use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

use crate::behaviour::events::ancient::{Offered, next_item};
use crate::ids::{EventIds as _, EventOptionIds as _};

/// `DamageVar("TakeTimeHpLoss", 5)`.
const TAKE_TIME_HP_LOSS: i32 = 5;

/// `DamageVar("ExamineHpLoss", 15)`.
const EXAMINE_HP_LOSS: i32 = 15;

#[derive(Debug, Clone)]
pub enum State<V: crate::Build> {
    /// The first page, and the option taken there.
    Initial { chosen: Option<NonMaxU8> },
    /// The dolls a page of relics offers.
    Dolls(Offered<V>),
}

pub struct DollRoom;

impl<V: crate::Build> EventBehavior<V> for DollRoom {
    type State = State<V>;

    /// Whether this is the second act, which the shared eligibility check
    /// answers.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(
        &self,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> State<V> {
        State::Initial { chosen: None }
    }

    fn observe(&self, state: &State<V>, _run: &impl EventView<V>) -> Room<V> {
        match state {
            State::Initial { chosen } => crate::behaviour::events::page::<V>(
                V::Event::DOLL_ROOM,
                &[
                    (V::EventOption::DOLL_ROOM_RANDOM, false),
                    (V::EventOption::DOLL_ROOM_TAKE_SOME_TIME, false),
                    (V::EventOption::DOLL_ROOM_EXAMINE, false),
                ],
                *chosen,
            ),
            State::Dolls(offered) => offered.observe(V::Event::DOLL_ROOM),
        }
    }

    /// `Random`: `Rng.NextItem` over the dolls, obtained, which finishes the
    /// event. `TakeSomeTime` and `Examine`: the damage, then the dolls
    /// `StableShuffle`d off the event's stream, the first two or all three.
    /// A doll's option obtains it and finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State<V>,
        option: u8,
        _run: &impl EventView<V>,
        rng: &mut R,
    ) -> HookEffects<V> {
        match state {
            State::Initial { chosen } => match option {
                0 => {
                    *chosen = NonMaxU8::new(option);
                    let doll = next_item(rng, V::DOLL_ROOM_DOLLS).expect("there are dolls");
                    smallvec::smallvec![HookEffect::ObtainRelic(doll), HookEffect::FinishEvent]
                }
                _ => {
                    let (damage, shown) = match option {
                        1 => (TAKE_TIME_HP_LOSS, 2),
                        _ => (EXAMINE_HP_LOSS, V::DOLL_ROOM_DOLLS.len()),
                    };
                    let mut dolls = V::DOLL_ROOM_DOLLS_BY_ID.to_vec();
                    rng.unstable_shuffle(&mut dolls);
                    *state = State::Dolls(Offered::new(dolls[..shown].to_vec()));
                    smallvec::smallvec![HookEffect::DamageOwner(damage)]
                }
            },
            State::Dolls(offered) => offered.choose(option),
        }
    }
}

/// `DOLLS`, `DOLLS_BY_ID`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `_dolls`, in the order the event lists them.
        const DOLL_ROOM_DOLLS: &'static [<$build as sts2_core::ContentVersion>::Relic] = &[
            <$build as sts2_core::ContentVersion>::Relic::DAUGHTER_OF_THE_WIND,
            <$build as sts2_core::ContentVersion>::Relic::MR_STRUGGLES,
            <$build as sts2_core::ContentVersion>::Relic::BING_BONG,
        ];
        /// `_dolls` in the order `StableShuffle` sorts them first, which is by id.
        const DOLL_ROOM_DOLLS_BY_ID: &'static [<$build as sts2_core::ContentVersion>::Relic] = &[
            <$build as sts2_core::ContentVersion>::Relic::BING_BONG,
            <$build as sts2_core::ContentVersion>::Relic::DAUGHTER_OF_THE_WIND,
            <$build as sts2_core::ContentVersion>::Relic::MR_STRUGGLES,
        ];
    };
}

pub(crate) use lists;
