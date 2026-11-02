//! `RazorTooth`: each Attack and Skill its owner plays, upgraded.

use sts2_core::{CardKind, EventHook, HookEffect};

/// `AfterCardPlayed` of an Attack or a Skill: `CardCmd.Upgrade` on the copy
/// played. The command itself skips a copy that cannot be upgraded.
///
/// # Panics
///
/// A card whose type the fight works out.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardPlayed {
        card, id, player, ..
    } = hook
        && player == owner
    {
        let kind = V::card_kind(card)
            .unwrap_or_else(|| unimplemented!("Razor Tooth reading a card whose type is computed"));
        if matches!(kind, CardKind::Attack | CardKind::Skill) {
            asked.push(HookEffect::UpgradeCopy { card: id, times: 1 });
        }
    }
}
