//! `ToyBox`: four wax relics offered as it is picked up, one melted every
//! third fight.

use sts2_core::{CustomReward, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `Combats`: the fights between melts.
const COMBATS: u8 = 3;

/// `Relics`: the wax relics it offers.
static WAX_RELICS: [CustomReward<V>; 4] = [CustomReward::WaxRelic; 4];

/// `CombatsSeen`, which it saves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    combats_seen: u8,
}

/// `AfterObtained`: `RewardsCmd.OfferCustom` of a `RelicReward` for each of
/// four relics off the front of the bag, each made wax.
/// `AfterCombatEnd`: one more fight seen, and each third melts the first wax
/// relic still whole.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::Obtained { .. } => asked.push(HookEffect::OfferRewards(&WAX_RELICS)),
        EventHook::CombatEnd { .. } => {
            state.combats_seen = (state.combats_seen.checked_add(1))
                .unwrap_or_else(|| unimplemented!("more fights than a byte counts"));
            if state.combats_seen.is_multiple_of(COMBATS) {
                asked.push(HookEffect::MeltFirstWax);
            }
        }
        hook => unimplemented!("what ToyBox does for {hook:?}"),
    }
}

/// `IsUsedUp`: every wax relic's fights seen.
pub(super) fn used_up(state: &State) -> bool {
    usize::from(state.combats_seen) >= usize::from(COMBATS) * WAX_RELICS.len()
}

/// `DisplayAmount`: the fights toward the next melt.
pub(super) fn display_amount(state: &State) -> Number {
    Number::from(state.combats_seen % COMBATS)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Twelve fights, three for each wax relic, use the box up, and eleven
    /// do not.
    #[test]
    fn twelve_fights_use_the_box_up() {
        let seen = |combats_seen| State { combats_seen };
        assert!(!used_up(&seen(11)));
        assert!(used_up(&seen(12)));
    }
}
