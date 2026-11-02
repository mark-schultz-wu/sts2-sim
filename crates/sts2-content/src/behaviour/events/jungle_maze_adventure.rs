//! Jungle Maze Adventure: gold alone for HP, or less gold in company.

use crate::ids::{EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::rng::Interval;
use sts2_core::{Decimal, EventBehavior, EventView, HookEffect, HookEffects, Room};

/// `DynamicVar("SoloGold", 150)`, before `CalculateVars` moves it.
const SOLO_GOLD: i64 = 150;

/// `DynamicVar("JoinForcesGold", 50)`, likewise.
const JOIN_FORCES_GOLD: i64 = 50;

/// `DamageVar("SoloHp", 18)`, unblockable and unpowered.
const SOLO_HP: i32 = 18;

#[derive(Debug, Clone)]
pub struct State {
    /// What each option pays, drawn as the event opens. Each keeps its
    /// fraction, which `PlayerCmd.GainGold` drops after the gold hooks.
    solo_gold: Decimal,
    join_forces_gold: Decimal,
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct JungleMazeAdventure;

impl<V: crate::Build> EventBehavior<V> for JungleMazeAdventure {
    type State = State;

    /// Any run of one player. With several, each player has more health
    /// than its `SoloHp`, which the run asks of every player.
    fn allowed(&self, run: &impl EventView<V>) -> bool {
        !run.multiplayer() || i64::from(run.hp()) > i64::from(SOLO_HP)
    }

    /// `CalculateVars`: `Rng.NextFloat(-15, 15)` onto each gold, the solo
    /// one first.
    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, rng: &mut R) -> State {
        let mut moved = |base: i64| {
            Decimal::from(base) + decimal(rng.next_double_range(Interval::new(-15.0, 15.0)))
        };
        let solo_gold = moved(SOLO_GOLD);
        let join_forces_gold = moved(JOIN_FORCES_GOLD);
        State {
            solo_gold,
            join_forces_gold,
            chosen: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::JUNGLE_MAZE_ADVENTURE,
            &[
                (V::EventOption::JUNGLE_MAZE_ADVENTURE_SOLO_QUEST, false),
                (V::EventOption::JUNGLE_MAZE_ADVENTURE_JOIN_FORCES, false),
            ],
            state.chosen,
        )
    }

    /// `WillKillPlayer`, for `SoloQuest`: its hit point loss.
    fn kills(&self, _state: &State, option: u8, run: &impl EventView<V>) -> bool {
        option == 0 && run.hp() <= SOLO_HP
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
                    HookEffect::GainGoldExact(state.solo_gold),
                    HookEffect::FinishEvent,
                ]
            }
            _ => smallvec::smallvec![
                HookEffect::GainGoldExact(state.join_forces_gold),
                HookEffect::FinishEvent,
            ],
        }
    }
}

/// `NextFloat` as the `decimal` it is added in: the draw is a `float`, and
/// the cast to `decimal` keeps seven significant digits.
fn decimal(drawn: f64) -> Decimal {
    let single = drawn as f32;
    Decimal::from_scientific(&format!("{:.6e}", f64::from(single)))
        .expect("a formatted number reads back")
}
