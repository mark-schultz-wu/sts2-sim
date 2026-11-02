//! `GoldenCompass`: the act it is picked up in becomes one fixed path, and
//! every unknown point on it is an event.

use sts2_core::{EventHook, HookEffect};

use crate::generated::encounters::RoomType;
use crate::generated::runstate::MapPointType;

/// `GoldenPathActMap._defaultPointTypes`, for a run of one player.
static GOLDEN_PATH: [MapPointType; 16] = [
    MapPointType::Monster,
    MapPointType::Unknown,
    MapPointType::Monster,
    MapPointType::RestSite,
    MapPointType::Monster,
    MapPointType::RestSite,
    MapPointType::Unknown,
    MapPointType::Treasure,
    MapPointType::Unknown,
    MapPointType::Treasure,
    MapPointType::Unknown,
    MapPointType::Shop,
    MapPointType::Elite,
    MapPointType::RestSite,
    MapPointType::Elite,
    MapPointType::RestSite,
];

/// The golden path for a run of several players, which
/// `list.RemoveAt(2)` takes the second Monster out of.
static GOLDEN_PATH_SEVERAL: [MapPointType; 15] = {
    let mut path = [MapPointType::Monster; 15];
    let mut at = 0;
    while at < path.len() {
        path[at] = GOLDEN_PATH[if at < 2 { at } else { at + 1 }];
        at += 1;
    }
    path
};

/// `GoldenPathAct`, which it saves, and nothing before it is picked up.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    golden_path_act: Option<usize>,
}

/// `AfterObtained`: the act it is picked up in, then
/// `RunManager.GenerateMap`. `ModifyGeneratedMap`: the golden path for that
/// act. `ModifyUnknownMapPointRoomTypes`: an event alone in that act.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Obtained { act, .. } => {
            state.golden_path_act = Some(act);
            asked.push(HookEffect::RegenerateMap);
        }
        EventHook::GeneratedMap { act, players } if state.golden_path_act == Some(act) => {
            asked.push(HookEffect::FixedPathMap(match players > 1 {
                true => &GOLDEN_PATH_SEVERAL,
                false => &GOLDEN_PATH,
            }));
        }
        EventHook::UnknownRoomTypes { act } if state.golden_path_act == Some(act) => {
            // Every room an unknown point can be besides an event.
            for room in [
                RoomType::Monster,
                RoomType::Elite,
                RoomType::Treasure,
                RoomType::Shop,
            ] {
                asked.push(HookEffect::RemoveRoomType(room));
            }
        }
        EventHook::GeneratedMap { .. } | EventHook::UnknownRoomTypes { .. } => {}
        hook => unimplemented!("what GoldenCompass does for {hook:?}"),
    }
}
