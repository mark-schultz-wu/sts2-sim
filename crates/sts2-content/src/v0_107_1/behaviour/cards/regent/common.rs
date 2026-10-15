//! The Regent's common cards.

use sts2_core::{
    Amount, CardIdent, Effect, EffectResolution, Fight, Pile, Played, PowerAmount, Scaling,
    Selection,
};

use super::super::super::super::generated::cards::{CardVar, Cards, StatusCard, TokenCard};
use super::super::super::super::generated::powers::Power;
use super::super::V;
use super::Resume;

/// `CollisionCourse`: its damage to the target, then a Debris into hand.
pub(super) fn collision_course<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    fight.attack(Scaling::Dynamic, |fight| {
        let attack = fight.resolve(Effect::Attack {
            dealer: fight.actor(),
            target,
            damage: Amount::Var(CardVar::Damage),
            scaling: Scaling::Dynamic,
        });
        fight.apply(attack);
    });
    if fight.waiting() {
        return super::wait(super::Resume::CollisionCourseHit);
    }
    collision_course_hit(fight)
}

/// The rest of `CollisionCourse`, once its attack is over: a Debris into
/// hand.
pub(super) fn collision_course_hit<F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.add_generated_cards(
        CardIdent::new(Cards::Status(StatusCard::Debris)),
        Pile::Hand,
        1,
    );
    Played::Done
}

/// `Glitterstream`. Gain block now and after the next block clear.
///
/// The number the power carries runs through `Hook.ModifyBlock` first, which
/// this card does itself rather than leave to the gain. It reads the hook
/// before the block it gains lands, as the game's own order does.
pub(super) fn glitterstream<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let next_turn = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(CardVar::BlockNextTurn),
        scaling: Scaling::Dynamic,
    });
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(CardVar::Block),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    fight.apply_to_actor(
        Power::BlockNextTurnPower,
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
pub(super) fn begone() -> Played<V> {
    Played::Asking {
        pile: Pile::Hand,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: super::super::super::Resume::Card(super::super::Resume::Regent(
            Resume::BegoneTransform,
        )),
    }
}

/// `Begone` again, with the card the selection took.
///
/// An empty hand selects nothing and the card does nothing, which is
/// `FirstOrDefault` finding none.
pub(super) fn begone_transform<F: Fight<V>>(
    fight: &mut F,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    if let Some(card) = chosen.one_or_none() {
        let mut into = CardIdent::new(Cards::Token(TokenCard::MinionStrike));
        into.upgrade = u8::from(fight.upgraded());
        fight.transform(card, into);
    }
    Played::Done
}
