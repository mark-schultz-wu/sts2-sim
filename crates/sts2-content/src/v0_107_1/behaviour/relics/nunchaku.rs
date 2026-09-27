//! `Nunchaku`: an energy for every tenth attack played.

use sts2_core::{CardKind, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `CardsVar(10)`.
const CARDS: u32 = 10;

/// `EnergyVar(1)`.
const ENERGY: Number = 1;

/// `AttacksPlayed`, its one `[SavedProperty]`, which carries from one fight
/// to the next.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    attacks_played: u32,
}

/// `AfterCardPlayed` counts each attack its owner plays, and every tenth
/// gives its energy. The game gives none once the fight is over, and energy
/// left then is never spent. A fight here has one player.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::CardPlayed { card, .. } = hook
        && <V as sts2_core::Rules>::card_kind(card) == Some(CardKind::Attack)
    {
        state.attacks_played += 1;
        if state.attacks_played.is_multiple_of(CARDS) {
            asked.push(HookEffect::GainEnergy(ENERGY));
        }
    }
}

/// `DisplayAmount`: the attacks counted towards the next energy.
pub(super) fn display_amount(state: &State) -> Number {
    Number::try_from(state.attacks_played % CARDS).expect("under ten fits a game number")
}
