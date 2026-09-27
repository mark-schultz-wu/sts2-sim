//! `TouchOfOrobas`: the starter relic, refined, as it is picked up.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::relics::Relic;

/// `RefinementUpgrades`: each character's starter and what it refines to.
const REFINEMENTS: [(Relic, Relic); 5] = [
    (Relic::BurningBlood, Relic::BlackBlood),
    (Relic::RingOfTheSnake, Relic::RingOfTheDrake),
    (Relic::DivineRight, Relic::DivineDestiny),
    (Relic::BoundPhylactery, Relic::PhylacteryUnbound),
    (Relic::CrackedCore, Relic::InfusedCore),
];

/// `AfterObtained`: `RelicCmd.Replace` on the first starter relic held,
/// with its refinement or a Circlet. The model comes with nothing written on
/// it. So the starter is looked up as the relic is picked up, the same one
/// `SetupForPlayer` reads.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained = hook {
        asked.push(HookEffect::ReplaceStarterRelic {
            upgrades: &REFINEMENTS,
            fallback: Relic::Circlet,
        });
    }
}
