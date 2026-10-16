//! `FurCoat`: seven fights of the act it is picked up in, marked, where every
//! enemy starts at one hit point.

use std::sync::Arc;

use sts2_core::{Coord, EventHook, HookEffect};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;
use crate::ids::RelicIds as _;

/// `DynamicVar("Combats", 7)`.
const COMBATS: usize = 7;

/// The id its own stream is seeded with.
const NAME: &str = "FUR_COAT";

/// What `CreatureCmd.SetCurrentHp` sets a marked fight's enemies to.
const HP: Number = 1;

/// `FurCoatActIndex`, and the points `FurCoatCoordCols` and
/// `FurCoatCoordRows` name. Nothing is marked before `AfterObtained`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    act: Option<usize>,
    marked: Option<Arc<[Coord]>>,
}

impl State {
    pub(super) fn marked(&self) -> &[Coord] {
        self.marked.as_deref().unwrap_or(&[])
    }

    pub(super) fn mark(&mut self, points: &[Coord]) {
        self.marked = Some(points.into());
    }

    fn marks(&self, point: Option<Coord>) -> bool {
        point.is_some_and(|point| self.marked().contains(&point))
    }
}

/// `AfterObtained`: the act it is picked up in, then `AddMarkedRooms`.
/// `ModifyGeneratedMapLate` marks a map of that act again.
/// `BeforeCombatStart` in a marked fight sets every enemy it can hit to one
/// hit point. `AfterCreatureAddedToCombat` does the same for an enemy that
/// joins one.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let mark = HookEffect::MarkCombatPoints {
        relic: V::Relic::FUR_COAT,
        count: COMBATS,
        name: NAME,
    };
    match hook {
        EventHook::Obtained { act, .. } => {
            state.act = Some(act);
            asked.push(mark);
        }
        EventHook::GeneratedMapLate { act } if state.act == Some(act) => asked.push(mark),
        EventHook::CombatStarting { point, .. } if state.marks(point) => {
            asked.push(HookEffect::SetEnemiesHp {
                creature: None,
                hp: HP,
            });
        }
        EventHook::CreatureAdded {
            creature,
            side: CombatSide::Enemy,
            point,
        } if state.marks(point) => {
            asked.push(HookEffect::SetEnemiesHp {
                creature: Some(creature),
                hp: HP,
            });
        }
        _ => {}
    }
}
