//! `ArchaicTooth`: the character's starter card, transformed into its
//! ancient form as the relic is picked up.

use sts2_core::{EventHook, HookEffect};

/// `AfterObtained`: `CardCmd.Transform` on the first deck card
/// `TranscendenceUpgrades` names.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::TransformFirstInDeck(
            V::ARCHAIC_TOOTH_TRANSCENDENCE,
        ));
    }
}

/// `TRANSCENDENCE`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `TranscendenceUpgrades`: each starter card and the card it becomes.
        const ARCHAIC_TOOTH_TRANSCENDENCE: &'static [(
            <$build as sts2_core::ContentVersion>::Card,
            <$build as sts2_core::ContentVersion>::Card,
        )] = &[
            (
                <$build as sts2_core::ContentVersion>::Card::BASH,
                <$build as sts2_core::ContentVersion>::Card::BREAK,
            ),
            (
                <$build as sts2_core::ContentVersion>::Card::NEUTRALIZE,
                <$build as sts2_core::ContentVersion>::Card::SUPPRESS,
            ),
            (
                <$build as sts2_core::ContentVersion>::Card::UNLEASH,
                <$build as sts2_core::ContentVersion>::Card::PROTECTOR,
            ),
            (
                <$build as sts2_core::ContentVersion>::Card::FALLING_STAR,
                <$build as sts2_core::ContentVersion>::Card::METEOR_SHOWER,
            ),
            (
                <$build as sts2_core::ContentVersion>::Card::DUALCAST,
                <$build as sts2_core::ContentVersion>::Card::QUADCAST,
            ),
        ];
    };
}

pub(crate) use lists;
