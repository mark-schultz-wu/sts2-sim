//! Jungle Maze Adventure: gold alone for HP, or less gold in company.

use nonmax::NonMaxU8;
use sts2_core::rng::Interval;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::events::{Event, EventOption as Said};

/// `DynamicVar("SoloGold", 150)`, before `CalculateVars` moves it.
const SOLO_GOLD: f64 = 150.0;

/// `DynamicVar("JoinForcesGold", 50)`, likewise.
const JOIN_FORCES_GOLD: f64 = 50.0;

/// `DamageVar("SoloHp", 18)`, unblockable and unpowered.
const SOLO_HP: i32 = 18;

#[derive(Debug, Clone)]
pub struct State {
    /// What each option pays, drawn as the event opens.
    solo_gold: i32,
    join_forces_gold: i32,
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct JungleMazeAdventure;

impl EventBehavior<V> for JungleMazeAdventure {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `CalculateVars`: `Rng.NextFloat(-15, 15)` onto each gold, the solo
    /// one first.
    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, rng: &mut R) -> State {
        let mut moved =
            |base| paid(base + decimal(rng.next_double_range(Interval::new(-15.0, 15.0))));
        let solo_gold = moved(SOLO_GOLD);
        let join_forces_gold = moved(JOIN_FORCES_GOLD);
        State {
            solo_gold,
            join_forces_gold,
            chosen: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::JungleMazeAdventure,
            &[
                (Said::JungleMazeAdventureSoloQuest, false),
                (Said::JungleMazeAdventureJoinForces, false),
            ],
            state.chosen,
        )
    }

    /// `SoloQuest`: `StableShuffle` of its three effects off the event's
    /// stream, the damage, then the gold. `JoinForces`: the gold. Each
    /// finishes the event.
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
                rng.stable_shuffle(&mut [0, 1, 2]);
                smallvec::smallvec![
                    HookEffect::DamageOwner(SOLO_HP),
                    HookEffect::GainGold(state.solo_gold),
                    HookEffect::FinishEvent,
                ]
            }
            _ => smallvec::smallvec![
                HookEffect::GainGold(state.join_forces_gold),
                HookEffect::FinishEvent,
            ],
        }
    }
}

/// `NextFloat` as the `decimal` it is added in: the draw is a `float`, and
/// the cast to `decimal` keeps seven significant digits.
fn decimal(drawn: f64) -> f64 {
    let single = drawn as f32;
    format!("{:.6e}", f64::from(single))
        .parse()
        .expect("a formatted number reads back")
}

/// `PlayerCmd.GainGold`'s `(int)amount`, which drops the fraction.
fn paid(amount: f64) -> i32 {
    amount.trunc() as i32
}
