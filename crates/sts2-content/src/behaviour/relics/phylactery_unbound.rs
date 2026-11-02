//! `PhylacteryUnbound`: Bound Phylactery refined, a larger Osty as each fight
//! opens and more each turn.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect};

/// `BeforeCombatStart`, and `AfterSideTurnStart` on every turn of its
/// owner's. Both are `OstyCmd.Summon` for its owner, the only player.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CombatStarting { .. } => asked.push(HookEffect::Summon(
            V::Relic::PHYLACTERY_UNBOUND_START_OF_COMBAT,
        )),
        EventHook::SideTurnStart { participants, .. } if participants.contains(owner) => {
            asked.push(HookEffect::Summon(
                V::Relic::PHYLACTERY_UNBOUND_START_OF_TURN,
            ));
        }
        _ => {}
    }
}
