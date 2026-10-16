//! `UnsettlingLamp`: the first card to debuff an enemy in a fight applies
//! its debuffs twice over.

use sts2_core::{CardId, EventHook, Modify, ModifyHook};

use crate::behaviour::powers::{debuff_at, lends};
use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;

/// `TriggeringCard`, `DoubledPowers` and `IsFinishedTriggering`.
///
/// `DoubledPowers` holds the one power that took the card, since nothing
/// adds to it once a card is taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State<V: crate::Build> {
    triggering: Option<CardId<V>>,
    doubled: Option<V::Power>,
    finished: bool,
}

impl<V: crate::Build> Default for State<V> {
    fn default() -> Self {
        Self {
            triggering: Default::default(),
            doubled: Default::default(),
            finished: Default::default(),
        }
    }
}

/// `BeforeCombatStart` and `AfterCombatEnd` forget it all.
/// `BeforePowerAmountChanged` takes the card whose play is about to put a
/// visible debuff from its owner on an enemy, while no card is taken.
/// `AfterCardPlayed` of that card finishes it.
///
/// A power on an enemy is always visible.
pub(super) fn event_hook<V: crate::Build>(state: &mut State<V>, hook: EventHook<V>) {
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
            && debuff_at::<V>(power, amount) =>
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
pub(super) fn modify_hook<V: crate::Build>(
    state: &State<V>,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        ModifyHook::PowerAmountGivenMultiplicative {
            power,
            amount,
            source: Some(card),
            ..
        } if state.triggering == Some(card)
            && !state.finished
            && !state.doubled.is_some_and(|doubled| {
                lends::<V>(doubled).is_some_and(|(lent, _)| lent == power)
            })
            && debuff_at::<V>(power, amount) =>
        {
            Modify::Scaled(sts2_core::Fixed::new(2, 0))
        }
        _ => Modify::Nothing,
    }
}
