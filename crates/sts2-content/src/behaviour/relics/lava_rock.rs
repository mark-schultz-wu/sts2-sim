//! `LavaRock`: two more relics from the first act's boss.

use sts2_core::{EventHook, HookEffect};

use crate::generated::encounters::RoomType;

/// `DynamicVar("Relics", 2)`.
const RELICS: usize = 2;

/// `HasTriggered`, the one `[SavedProperty]` this relic declares.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    triggered: bool,
}

/// `TryModifyRewards`: a relic reward for each of `Relics`, added to the
/// first act's boss rewards, once.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::ModifyRewards {
        room: Some(RoomType::Boss),
        act: 0,
        ..
    } = hook
        && !state.triggered
    {
        state.triggered = true;
        asked.push(HookEffect::AddRelicRewards(RELICS));
    }
}
