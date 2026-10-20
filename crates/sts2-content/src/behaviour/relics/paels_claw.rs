//! `PaelsClaw`: the deck's Defends made Goopy as it is picked up.

use crate::ids::EnchantmentIds as _;
use sts2_core::{EventHook, HookEffect};

/// `AfterObtained`: `CardCmd.Enchant<Goopy>` at one on each deck card the
/// enchantment takes, which is each Defend it can enchant.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::EnchantDeck {
            enchantment: V::Enchantment::GOOPY,
            amount: 1,
        });
    }
}
