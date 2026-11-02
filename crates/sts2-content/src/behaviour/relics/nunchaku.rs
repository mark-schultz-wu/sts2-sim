//! `Nunchaku`: an energy for every tenth attack played.

use crate::vars::RelicVars as _;
use sts2_core::{CardKind, EventHook, HookEffect};

use crate::behaviour::relics::Number;

/// `AttacksPlayed`, its one `[SavedProperty]`, which carries from one fight
/// to the next.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    attacks_played: u32,
}

/// `AfterCardPlayed` counts each attack its owner plays, and every tenth
/// gives its energy. The game gives none once the fight is over, and energy
/// left then is never spent.
pub(super) fn event_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardPlayed { card, player, .. } = hook
        && player == owner
        && <V as sts2_core::Rules>::card_kind(card) == Some(CardKind::Attack)
    {
        state.attacks_played += 1;
        if state
            .attacks_played
            .is_multiple_of(V::Relic::NUNCHAKU_CARDS as u32)
        {
            asked.push(HookEffect::GainEnergy(V::Relic::NUNCHAKU_ENERGY));
        }
    }
}

/// `DisplayAmount`: the attacks counted towards the next energy.
pub(super) fn display_amount<V: crate::Build>(state: &State) -> Number {
    Number::try_from(state.attacks_played % (V::Relic::NUNCHAKU_CARDS as u32))
        .expect("under ten fits a game number")
}
