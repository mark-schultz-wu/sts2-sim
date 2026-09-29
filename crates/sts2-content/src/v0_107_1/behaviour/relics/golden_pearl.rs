//! `GoldenPearl`: gold, once, as it is picked up.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `GoldVar(150)`.
const GOLD: Number = 150;

/// `AfterObtained`, which is `PlayerCmd.GainGold`.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::GainGold(GOLD));
    }
}
