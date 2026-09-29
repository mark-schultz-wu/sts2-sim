//! `ArchaicTooth`: the character's starter card, transformed into its
//! ancient form as the relic is picked up.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{
    Cards, DefectCard, IroncladCard, NecrobinderCard, RegentCard, SilentCard,
};

/// `TranscendenceUpgrades`: each starter card and the card it becomes.
pub(crate) const TRANSCENDENCE: [(Cards, Cards); 5] = [
    (
        Cards::Ironclad(IroncladCard::Bash),
        Cards::Ironclad(IroncladCard::Break),
    ),
    (
        Cards::Silent(SilentCard::Neutralize),
        Cards::Silent(SilentCard::Suppress),
    ),
    (
        Cards::Necrobinder(NecrobinderCard::Unleash),
        Cards::Necrobinder(NecrobinderCard::Protector),
    ),
    (
        Cards::Regent(RegentCard::FallingStar),
        Cards::Regent(RegentCard::MeteorShower),
    ),
    (
        Cards::Defect(DefectCard::Dualcast),
        Cards::Defect(DefectCard::Quadcast),
    ),
];

/// `AfterObtained`: `CardCmd.Transform` on the first deck card
/// `TranscendenceUpgrades` names.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::TransformFirstInDeck(&TRANSCENDENCE));
    }
}
