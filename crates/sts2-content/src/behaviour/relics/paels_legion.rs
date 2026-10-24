//! `PaelsLegion`: a pet that doubles a card's block, then sleeps two turns.

use crate::vars::RelicVars as _;
use sts2_core::{CardId, EventHook, HookEffect, Modify, ModifyHook, SideKinds as _};

use crate::behaviour::relics::Number;
use crate::ids::MonsterIds as _;

/// `Cooldown`, `TriggeredBlockLastTurn` and `AffectedCardPlay`, the play
/// named by the card it plays. `Skin` draws off its own `Rng`, which no other
/// stream shares, and only the pet's look reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State<V: crate::Build> {
    cooldown: i32,
    triggered_block_last_turn: bool,
    affected: Option<CardId<V>>,
}

impl<V: crate::Build> Default for State<V> {
    fn default() -> Self {
        Self {
            cooldown: Default::default(),
            triggered_block_last_turn: Default::default(),
            affected: Default::default(),
        }
    }
}

/// `BeforeCombatStart`: `PlayerCmd.AddPet<PaelsLegion>`. `AfterObtained`
/// adds it too inside a fight, and a run picks it up only outside one.
/// `AfterModifyingBlockAmount` takes the play it doubled, where the gain came
/// to anything. `AfterCardPlayed` of that play puts it to sleep.
/// `AfterSideTurnStart` of the player's side counts a turn of sleep off, and
/// `AfterCombatEnd` wakes it.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State<V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let player = <V as sts2_core::ContentVersion>::CombatSide::PLAYER;
    match hook {
        EventHook::CombatStarting { .. } => {
            asked.push(HookEffect::AddPet(V::Monster::PAELS_LEGION))
        }
        EventHook::BlockAmountModified {
            amount,
            source: Some(card),
        } if amount > 0 && state.affected.is_none_or(|taken| taken == card) => {
            state.affected = Some(card);
        }
        EventHook::CardPlayed { id, .. } if state.affected == Some(id) => {
            state.affected = None;
            state.cooldown = V::Relic::PAELS_LEGION_TURNS;
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
/// while it is awake, where the block is `IsCardOrMonsterMove`. A fight
/// here has one player, whose every card is its owner's.
pub(super) fn modify_hook<V: crate::Build>(
    state: &State<V>,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        ModifyHook::BlockMultiplicative {
            moved: true,
            source: Some(_),
            ..
        } if state.cooldown <= 0 => Modify::Scaled(sts2_core::Fixed::new(2, 0)),
        ModifyHook::BlockMultiplicative { .. } => Modify::Nothing,
        hook => unimplemented!("what PaelsLegion does for {hook:?}"),
    }
}

/// `DisplayAmount`: the turns it still sleeps, or `-1` while it is awake,
/// and outside a fight, which a player's death leaves the run.
pub(super) fn display_amount<V: crate::Build>(state: &State<V>, fighting: bool) -> Number {
    match fighting && state.cooldown > 0 {
        true => state.cooldown,
        false => -1,
    }
}
