//! `PaelsFlesh`: one more energy from the third turn of each fight.

use crate::vars::RelicVars as _;
use sts2_core::{EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;

/// The `TurnNumber` from which it adds.
const FROM_TURN: u32 = 3;

/// `BeforeCombatStart`, `BeforeSideTurnStart` and `AfterSideTurnStart`,
/// which refresh its counter and light it from the third turn. None
/// changes the fight, and it does nothing for any other hook.
pub(super) fn event_hook<V: crate::Build>(_hook: EventHook<V>) {}

/// `ModifyMaxEnergy`: its energy while its owner's `TurnNumber` is three or
/// more.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    match hook {
        // `player != base.Owner`: another player's number is left as it is.
        ModifyHook::MaxEnergy { player, .. } if player != owner => Modify::Nothing,
        ModifyHook::MaxEnergy { turn, .. } if turn >= FROM_TURN => {
            Modify::Added(V::Relic::PAELS_FLESH_ENERGY)
        }
        ModifyHook::MaxEnergy { .. } => Modify::Nothing,
        hook => unimplemented!("what PaelsFlesh does for {hook:?}"),
    }
}

/// `DisplayAmount`: `PlayerCombatState?.TurnNumber ?? 1`, which is the
/// fight's while one runs and the last one's otherwise.
pub(super) fn display_amount(turn: Option<u32>) -> Number {
    turn.map_or(1, |turn| {
        Number::try_from(turn).expect("a turn number fits the game's numbers")
    })
}
