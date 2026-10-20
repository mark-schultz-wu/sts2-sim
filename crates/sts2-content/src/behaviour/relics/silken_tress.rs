//! `SilkenTress`: every coin gone, and the first card reward enchanted with
//! Glam.

use sts2_core::{EventHook, HookEffect};

use crate::behaviour::enchantments::can_enchant;
use crate::ids::EnchantmentIds as _;

/// What it saves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    /// `IsUsed`.
    used: bool,
}

/// `AfterObtained` loses every coin its owner holds. Until it is used,
/// `TryModifyCardRewardOptionsLate` replaces each reward card Glam can
/// enchant with a clone enchanted at one, and
/// `AfterModifyingCardRewardOptions` then marks it used.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Obtained { .. } => asked.push(HookEffect::LoseAllGold),
        EventHook::CardRewardOption {
            card,
            card_reward: true,
        } if !state.used => {
            if can_enchant(V::Enchantment::GLAM, &card) {
                asked.push(HookEffect::EnchantCard {
                    enchantment: V::Enchantment::GLAM,
                    amount: 1,
                });
            }
        }
        EventHook::CardRewardOption { .. } | EventHook::CardRewardPick { .. } => {}
        EventHook::CardRewardOptionsModified { card_reward: true } => state.used = true,
        EventHook::CardRewardOptionsModified { .. } => {}
        hook => unimplemented!("what SilkenTress does for {hook:?}"),
    }
}

/// `IsUsedUp`, which is `IsUsed`.
pub(super) fn used_up(state: &State) -> bool {
    state.used
}
