//! `PaelsClaw`: the deck's Defends made Goopy as it is picked up.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::enchantments::Enchantment;

/// `AfterObtained`: `CardCmd.Enchant<Goopy>` at one on each deck card the
/// enchantment takes, which is each Defend it can enchant.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.push(HookEffect::EnchantDeck {
            enchantment: Enchantment::Goopy,
            amount: 1,
        });
    }
}
