//! `TouchOfOrobas`: the starter relic, refined, as it is picked up.

use crate::ids::RelicIds as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterObtained`: `RelicCmd.Replace` on the first starter relic held,
/// with its refinement or a Circlet. The model comes with nothing written on
/// it. So the starter is looked up as the relic is picked up, the same one
/// `SetupForPlayer` reads.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::ReplaceStarterRelic {
            upgrades: V::TOUCH_OF_OROBAS_REFINEMENTS,
            fallback: V::Relic::CIRCLET,
        });
    }
}

/// `REFINEMENTS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `RefinementUpgrades`: each character's starter and what it refines to.
        const TOUCH_OF_OROBAS_REFINEMENTS: &'static [(
            <$build as sts2_core::ContentVersion>::Relic,
            <$build as sts2_core::ContentVersion>::Relic,
        )] = &[
            (
                <$build as sts2_core::ContentVersion>::Relic::BURNING_BLOOD,
                <$build as sts2_core::ContentVersion>::Relic::BLACK_BLOOD,
            ),
            (
                <$build as sts2_core::ContentVersion>::Relic::RING_OF_THE_SNAKE,
                <$build as sts2_core::ContentVersion>::Relic::RING_OF_THE_DRAKE,
            ),
            (
                <$build as sts2_core::ContentVersion>::Relic::DIVINE_RIGHT,
                <$build as sts2_core::ContentVersion>::Relic::DIVINE_DESTINY,
            ),
            (
                <$build as sts2_core::ContentVersion>::Relic::BOUND_PHYLACTERY,
                <$build as sts2_core::ContentVersion>::Relic::PHYLACTERY_UNBOUND,
            ),
            (
                <$build as sts2_core::ContentVersion>::Relic::CRACKED_CORE,
                <$build as sts2_core::ContentVersion>::Relic::INFUSED_CORE,
            ),
        ];
    };
}

pub(crate) use lists;
