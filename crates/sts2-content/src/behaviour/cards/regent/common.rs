//! The Regent's common cards.

use crate::behaviour::Played;
use sts2_core::{Amount, CardIdent, Effect, EffectResolution, Fight, Pile, PowerAmount, Scaling};

use crate::ids::{CardIds as _, CardVarIds as _, PowerIds as _};

/// The rest of `CollisionCourse`, once its attack is over: a Debris into
/// hand.
pub(super) fn collision_course_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    fight.add_generated_cards(CardIdent::new(V::Card::DEBRIS), Pile::Hand, 1);
}

/// `Glitterstream`. Gain block now and after the next block clear.
///
/// The number the power carries runs through `Hook.ModifyBlock` first, which
/// this card does itself rather than leave to the gain. It reads the hook
/// before the block it gains lands, as the game's own order does. A fraction
/// below one lands a power of nothing, as Dodge and Roll's does.
pub(super) fn glitterstream<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let next_turn = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(V::CardVar::BLOCK_NEXT_TURN),
        scaling: Scaling::Dynamic,
    });
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(V::CardVar::BLOCK),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    match next_turn.amount() == 0 && next_turn.positive() {
        true => fight.apply_fraction_to_actor(V::Power::BLOCK_NEXT_TURN_POWER),
        false => fight.apply_to_actor(
            V::Power::BLOCK_NEXT_TURN_POWER,
            PowerAmount::Counted(next_turn.amount()),
        ),
    }
    Played::Done
}

/// `Begone`, with the card the selection took, turned into a Minion Strike.
///
/// The selection takes the hand itself while the hand holds one card, so it
/// asks nothing there. An empty hand selects nothing and the card does
/// nothing, which is `FirstOrDefault` finding none.
///
/// Upgraded, the Minion Strike it creates is upgraded with it. Begone prints
/// no number for an upgrade to raise.
pub(super) fn begone_transform<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    chosen: &sts2_core::SelectedCards<V>,
) {
    if let Some(card) = chosen.one_or_none() {
        let mut into = CardIdent::new(V::Card::MINION_STRIKE);
        into.upgrade = u8::from(fight.upgraded());
        fight.transform(card, into);
    }
}
