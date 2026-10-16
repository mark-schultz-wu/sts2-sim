//! `Vambrace`: the first card to give block in a fight gives twice as much.

use sts2_core::{CardId, EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// `TriggeringCard` and `BlockGainedThisCombat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State<V: crate::Build> {
    triggering: Option<CardId<V>>,
    gained: bool,
}

impl<V: crate::Build> Default for State<V> {
    fn default() -> Self {
        Self {
            triggering: Default::default(),
            gained: Default::default(),
        }
    }
}

/// `BeforeCombatStart` and `AfterCombatEnd` forget the card and the gain.
/// `AfterModifyingBlockAmount` takes the card whose play it doubled, where
/// the gain came to anything. `AfterCardPlayed` of that card spends it.
pub(super) fn event_hook<V: crate::Build>(state: &mut State<V>, hook: EventHook<V>) {
    match hook {
        EventHook::CombatStarting { .. } | EventHook::CombatEnd { .. } => {
            *state = State::default();
        }
        EventHook::BlockAmountModified {
            amount,
            source: Some(card),
        } if amount > 0 => state.triggering = Some(card),
        EventHook::CardPlayed { id, .. } if state.triggering == Some(id) => state.gained = true,
        _ => {}
    }
}

/// `ModifyBlockMultiplicative`: twice the block a card gives, while
/// that card is the one it took or it took none, and nothing spent it. A
/// fight here has one player, whose every card is its owner's. The entry's
/// `powered` stands for `IsCardOrMonsterMove`, as Unmovable reads it.
pub(super) fn modify_hook<V: crate::Build>(
    state: &State<V>,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        ModifyHook::BlockMultiplicative {
            powered: true,
            source: Some(card),
            ..
        } if state.triggering.is_none_or(|taken| taken == card) && !state.gained => {
            Modify::Scaled(sts2_core::Fixed::new(2, 0))
        }
        _ => Modify::Nothing,
    }
}
