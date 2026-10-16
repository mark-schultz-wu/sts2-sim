//! The Regent's common cards.

use sts2_core::{
    Amount, CardIdent, Effect, EffectResolution, Fight, Pile, Played, PowerAmount, Scaling,
    Selection,
};

use crate::behaviour::cards::regent::Resume;
use crate::ids::{CardIds as _, CardVarIds as _, PowerIds as _};

/// `CollisionCourse`: its damage to the target, then a Debris into hand.
pub(super) fn collision_course<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    fight.attack(Scaling::Dynamic, |fight| {
        let attack = fight.resolve(Effect::Attack {
            dealer: fight.actor(),
            target,
            damage: Amount::Var(V::CardVar::DAMAGE),
            scaling: Scaling::Dynamic,
        });
        fight.apply(attack);
    });
    if fight.waiting() {
        return crate::behaviour::cards::regent::wait::<V>(
            crate::behaviour::cards::regent::Resume::CollisionCourseHit,
        );
    }
    collision_course_hit::<V, _>(fight)
}

/// The rest of `CollisionCourse`, once its attack is over: a Debris into
/// hand.
pub(super) fn collision_course_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.add_generated_cards(CardIdent::new(V::Card::DEBRIS), Pile::Hand, 1);
    Played::Done
}

/// `Glitterstream`. Gain block now and after the next block clear.
///
/// The number the power carries runs through `Hook.ModifyBlock` first, which
/// this card does itself rather than leave to the gain. It reads the hook
/// before the block it gains lands, as the game's own order does.
pub(super) fn glitterstream<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
    fight.apply_to_actor(
        V::Power::BLOCK_NEXT_TURN_POWER,
        PowerAmount::Counted(next_turn.amount()),
    );
    Played::Done
}

/// `Begone`. Turn a card in hand into a Minion Strike.
///
/// The selection takes the hand itself while the hand holds one card, so this
/// asks nothing there. An empty hand selects nothing and the card does
/// nothing, which is `FirstOrDefault` finding none.
///
/// Upgraded, the Minion Strike it creates is upgraded with it. Begone prints
/// no number for an upgrade to raise.
pub(super) fn begone<V: crate::Build>() -> Played<V> {
    Played::Asking {
        pile: Pile::Hand,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: crate::behaviour::cards::Resume::Regent(Resume::BegoneTransform).into(),
    }
}

/// `Begone` again, with the card the selection took.
///
/// An empty hand selects nothing and the card does nothing, which is
/// `FirstOrDefault` finding none.
pub(super) fn begone_transform<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    if let Some(card) = chosen.one_or_none() {
        let mut into = CardIdent::new(V::Card::MINION_STRIKE);
        into.upgrade = u8::from(fight.upgraded());
        fight.transform(card, into);
    }
    Played::Done
}
