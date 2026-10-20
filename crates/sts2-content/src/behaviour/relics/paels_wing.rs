//! `PaelsWing`: a card reward can be sacrificed, and every second sacrifice
//! is a relic.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, HookEffect, RelicPull, RewardAlternative};

use crate::behaviour::relics::Number;

/// `RewardsSacrificed`, which it saves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    rewards_sacrificed: u8,
}

/// `TryModifyCardRewardAlternatives`: `SACRIFICE`, which completes the
/// reward. Its `OnSacrifice` counts one, and each second is
/// `RelicCmd.Obtain` of the next relic off the front of the bag.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardRewardAlternatives => {
            asked.push(HookEffect::OfferRewardAlternative(
                RewardAlternative::Sacrifice,
            ));
        }
        EventHook::CardRewardAlternativeChosen(RewardAlternative::Sacrifice) => {
            state.rewards_sacrificed = (state.rewards_sacrificed.checked_add(1))
                .unwrap_or_else(|| unimplemented!("more sacrifices than a byte counts"));
            if state
                .rewards_sacrificed
                .is_multiple_of(V::Relic::PAELS_WING_SACRIFICES as u8)
            {
                asked.push(HookEffect::ObtainNextRelic(RelicPull::Rolled));
            }
        }
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what PaelsWing does for {hook:?}"),
    }
}

/// `DisplayAmount`: the sacrifices toward the next relic.
pub(super) fn display_amount<V: crate::Build>(state: &State) -> Number {
    Number::from(state.rewards_sacrificed % (V::Relic::PAELS_WING_SACRIFICES as u8))
}
