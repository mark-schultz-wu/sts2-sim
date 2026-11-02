//! `VitruvianMinion`: twice the damage and the block of a Minion card.

use sts2_core::{CardTable as _, CardTagKinds as _, EventHook, Fixed, Modify, ModifyHook};

use crate::behaviour::relics::Number;
use crate::generated::cards::CardTag;

/// `ModifyDamageMultiplicative` and `ModifyBlockMultiplicative`: twice the
/// number a card tagged Minion deals or gives. A fight here has one player,
/// whose every card is its owner's.
pub(super) fn modify_hook<V: crate::Build>(
    owner: sts2_core::CombatId,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    let doubled = Modify::Scaled(Fixed::new(2, 0));
    match hook {
        ModifyHook::DamageMultiplicative {
            card: Some(source), ..
        } if source.owner == owner && source.card.tagged(CardTag::MINION) => doubled,
        ModifyHook::BlockMultiplicative {
            card: Some(card),
            player: Some(player),
            ..
        } if player == owner && card.tagged(CardTag::MINION) => doubled,
        ModifyHook::DamageMultiplicative { .. } | ModifyHook::BlockMultiplicative { .. } => {
            Modify::Nothing
        }
        hook => unimplemented!("what VitruvianMinion does for {hook:?}"),
    }
}

/// It answers no event, but its obtaining.
pub(super) fn event_hook<V: crate::Build>(hook: EventHook<V>) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what VitruvianMinion does for {hook:?}"),
    }
}
