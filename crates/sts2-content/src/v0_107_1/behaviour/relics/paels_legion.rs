//! `PaelsLegion`: a pet that doubles a card's block, then sleeps two turns.

use sts2_core::{CardId, EventHook, HookEffect, Modify, ModifyHook, SideKinds as _};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::monsters::Monster;
use super::Number;

/// `Turns`: how long it sleeps after doubling a card's block.
const TURNS: i32 = 2;

/// `Cooldown`, `TriggeredBlockLastTurn` and `AffectedCardPlay`, the play
/// named by the card it plays. `Skin` draws off its own `Rng`, which no other
/// stream shares, and only the pet's look reads it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    cooldown: i32,
    triggered_block_last_turn: bool,
    affected: Option<CardId<V>>,
}

/// `BeforeCombatStart`: `PlayerCmd.AddPet<PaelsLegion>`. `AfterObtained`
/// adds it too inside a fight, and a run picks it up only outside one.
/// `AfterModifyingBlockAmount` takes the play it doubled, where the gain came
/// to anything. `AfterCardPlayed` of that play puts it to sleep.
/// `AfterSideTurnStart` of the player's side counts a turn of sleep off, and
/// `AfterCombatEnd` wakes it.
pub(super) fn event_hook(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let player = <V as sts2_core::ContentVersion>::CombatSide::PLAYER;
    match hook {
        EventHook::CombatStarting { .. } => asked.push(HookEffect::AddPet(Monster::PaelsLegion)),
        EventHook::BlockAmountModified {
            amount,
            source: Some(card),
        } if amount > 0 && state.affected.is_none_or(|taken| taken == card) => {
            state.affected = Some(card);
        }
        EventHook::CardPlayed { id, .. } if state.affected == Some(id) => {
            state.affected = None;
            state.cooldown = TURNS;
            state.triggered_block_last_turn = true;
        }
        EventHook::SideTurnStart { side, .. } if side == player => {
            state.cooldown -= 1;
            state.triggered_block_last_turn = false;
        }
        EventHook::CombatEnd { .. } => *state = State::default(),
        EventHook::Obtained { .. }
        | EventHook::BlockAmountModified { .. }
        | EventHook::CardPlayed { .. }
        | EventHook::SideTurnStart { .. } => {}
        hook => unimplemented!("what PaelsLegion does for {hook:?}"),
    }
}

/// `ModifyBlockMultiplicative`: twice the block its owner's card gives
/// while it is awake. A fight here has one player, whose every card is its
/// owner's. The entry's `powered` stands for `IsCardOrMonsterMove`, as
/// Vambrace reads it.
pub(super) fn modify_hook(state: &State, hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::BlockMultiplicative {
            powered: true,
            source: Some(_),
            ..
        } if state.cooldown <= 0 => Modify::Scaled(sts2_core::Fixed::new(2, 0)),
        ModifyHook::BlockMultiplicative { .. } => Modify::Nothing,
        hook => unimplemented!("what PaelsLegion does for {hook:?}"),
    }
}

/// `DisplayAmount`: the turns it still sleeps, or `-1` while it is awake.
pub(super) fn display_amount(state: &State) -> Number {
    match state.cooldown > 0 {
        true => state.cooldown,
        false => -1,
    }
}
