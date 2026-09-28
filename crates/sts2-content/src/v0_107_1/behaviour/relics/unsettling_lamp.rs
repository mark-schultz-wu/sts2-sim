//! `UnsettlingLamp`: the first card to debuff an enemy in a fight applies
//! its debuffs twice over.

use sts2_core::{CardId, EventHook, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::powers::Power;
use super::super::super::generated::runstate::CombatSide;
use super::super::powers::{debuff_at, lends};
use super::Number;

/// `TriggeringCard`, `DoubledPowers` and `IsFinishedTriggering`.
///
/// `DoubledPowers` holds the one power that took the card, since nothing
/// adds to it once a card is taken.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    triggering: Option<CardId<V>>,
    doubled: Option<Power>,
    finished: bool,
}

/// `BeforeCombatStart` and `AfterCombatEnd` forget it all.
/// `BeforePowerAmountChanged` takes the card whose play is about to put a
/// visible debuff from its owner on an enemy, while no card is taken.
/// `AfterCardPlayed` of that card finishes it.
///
/// A power on an enemy is always visible.
pub(super) fn event_hook(state: &mut State, hook: EventHook<V>) {
    match hook {
        EventHook::CombatStarting { .. } | EventHook::CombatEnd { .. } => {
            *state = State::default();
        }
        EventHook::PowerAmountChanging {
            power,
            amount,
            target_side,
            applier,
            player,
            source: Some(card),
            ..
        } if state.triggering.is_none()
            && !state.finished
            && applier == Some(player)
            && target_side != CombatSide::Player
            && debuff_at(power, amount) =>
        {
            state.triggering = Some(card);
            state.doubled = Some(power);
        }
        EventHook::CardPlayed { id, .. } if state.triggering == Some(id) && !state.finished => {
            state.finished = true;
        }
        _ => {}
    }
}

/// `ModifyPowerAmountGivenMultiplicative`: twice a debuff the taken card
/// gives before it finishes, unless the power it took lends that one.
pub(super) fn modify_hook(state: &State, hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::PowerAmountGivenMultiplicative {
            power,
            amount,
            source: Some(card),
            ..
        } if state.triggering == Some(card)
            && !state.finished
            && !state
                .doubled
                .is_some_and(|doubled| lends(doubled).is_some_and(|(lent, _)| lent == power))
            && debuff_at(power, amount) =>
        {
            Modify::Scaled(2.0)
        }
        _ => Modify::Nothing,
    }
}
