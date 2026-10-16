//! `BeltBuckle`: Dexterity while a fight's belt holds no potion.

use crate::ids::PowerIds as _;
use sts2_core::{EventHook, HookEffect, PowerAmount};

/// `PowerVar<DexterityPower>(2)`.
const DEXTERITY: i32 = 2;

/// `DexterityApplied`: whether the Dexterity it gave is on its owner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct State {
    dexterity_applied: bool,
}

/// `BeforeCombatStart` and the potion hooks: `ApplyDexterity` once a fight
/// finds the belt empty, and `RemoveDexterity` once it holds a potion again.
/// `AfterCombatVictory` forgets it.
///
/// Its `AfterObtained` applies in a fight alone, and the run obtains relics
/// outside one. Its `AfterCombatEnd` only sets the relic's `Status`.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let empty = match hook {
        EventHook::CombatStarting {
            holding_potions, ..
        } => {
            state.dexterity_applied = false;
            !holding_potions
        }
        EventHook::PotionProcured { in_fight: true } => {
            if state.dexterity_applied {
                state.dexterity_applied = false;
                asked.push(HookEffect::ApplyToOwner(
                    V::Power::DEXTERITY_POWER,
                    PowerAmount::Counted(-DEXTERITY),
                ));
            }
            return;
        }
        EventHook::PotionUsed {
            in_fight: true,
            holding_potions,
        }
        | EventHook::PotionDiscarded {
            in_fight: true,
            holding_potions,
        } => !holding_potions,
        EventHook::CombatVictory { .. } => {
            state.dexterity_applied = false;
            return;
        }
        _ => return,
    };
    if empty && !state.dexterity_applied {
        state.dexterity_applied = true;
        asked.push(HookEffect::ApplyToOwner(
            V::Power::DEXTERITY_POWER,
            PowerAmount::Counted(DEXTERITY),
        ));
    }
}
