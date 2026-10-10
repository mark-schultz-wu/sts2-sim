//! `VitruvianMinion`: twice the damage and the block of a Minion card.

use sts2_core::{CardTable as _, CardTagKinds as _, EventHook, Fixed, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::CardTag;
use super::Number;

/// `ModifyDamageMultiplicative` and `ModifyBlockMultiplicative`: twice the
/// number a card tagged Minion deals or gives. A fight here has one player,
/// whose every card is its owner's.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    let doubled = Modify::Scaled(Fixed::new(2, 0));
    match hook {
        ModifyHook::DamageMultiplicative {
            card: Some(source), ..
        } if source.card.tagged(CardTag::MINION) => doubled,
        ModifyHook::BlockMultiplicative {
            card: Some(card), ..
        } if card.tagged(CardTag::MINION) => doubled,
        ModifyHook::DamageMultiplicative { .. } | ModifyHook::BlockMultiplicative { .. } => {
            Modify::Nothing
        }
        hook => unimplemented!("what VitruvianMinion does for {hook:?}"),
    }
}

/// It answers no event, but its obtaining.
pub(super) fn event_hook(hook: EventHook<V>) {
    match hook {
        EventHook::Obtained { .. } => {}
        hook => unimplemented!("what VitruvianMinion does for {hook:?}"),
    }
}
