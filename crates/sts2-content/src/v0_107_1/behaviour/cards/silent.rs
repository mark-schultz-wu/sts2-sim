//! The Silent's cards the generator leaves to a hand port.

use sts2_core::{
    Amount, CardIdent, CardKind, Change, CostPool, Effect, EventHook, Fight, HookEffect, Lasting,
    Pile, Played, PowerAmount, Rules as _, Scaling, Selection,
};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardVar, Cards, SilentCard, TokenCard};
use super::super::super::generated::powers::Power;
use super::V;

/// Where a Silent card stopped inside its own play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    /// `Survivor`, after its owner picked the card to discard.
    SurvivorDiscard,
}

pub(super) fn on_play<F: Fight<V>>(card: SilentCard, fight: &mut F) -> Played<V> {
    match card {
        SilentCard::Survivor => survivor(fight),
        SilentCard::Haze => haze(fight),
        SilentCard::UpMySleeve => up_my_sleeve(fight),
        card => card_plays::silent::on_play(card, fight),
    }
}

/// `Haze`: its Poison onto each enemy that can be hit, one at a time.
fn haze<F: Fight<V>>(fight: &mut F) -> Played<V> {
    for target in fight.hittable_opponents() {
        fight.apply_power(
            target,
            Power::PoisonPower,
            PowerAmount::Counted(fight.var(CardVar::PoisonPower)),
        );
    }
    Played::Done
}

pub(super) fn resume_play<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        // `CardCmd.Discard` of the one taken.
        Resume::SurvivorDiscard => {
            fight.act_on(sts2_core::PileAct::Discard, chosen.as_slice());
            Played::Done
        }
    }
}

/// `Survivor`: its block, then one card from hand to discard.
fn survivor<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(CardVar::Block),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    Played::Asking {
        pile: Pile::Hand,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: super::super::Resume::Card(super::Resume::Silent(Resume::SurvivorDiscard)),
    }
}

/// `UpMySleeve`: its `Cards` of Shivs into hand one at a time, then one off
/// its own cost for the rest of the fight.
fn up_my_sleeve<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let count: usize =
        num_traits::cast(fight.var(CardVar::Cards)).expect("a count of Shivs is not negative");
    for _ in 0..count {
        fight.add_generated_cards(CardIdent::new(Cards::Token(TokenCard::Shiv)), Pile::Hand, 1);
    }
    fight.modify_this(Change::ChangeCost(CostPool::Energy, -1), Lasting::ThisFight);
    Played::Done
}

/// Deliver a notification to one Silent card in the fight.
pub(super) fn event_hook(
    card: SilentCard,
    id: sts2_core::CardId<V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    let skill = |card: Cards| V::card_kind(card) == Some(CardKind::Skill);
    match (card, hook) {
        // `Pinpoint.AfterCardPlayed`: one off its cost this turn for each
        // Skill its owner plays. A fight here has one player.
        (SilentCard::Pinpoint, EventHook::CardPlayed { card, .. }) => {
            if skill(card) {
                asked.push(HookEffect::ModifySelf {
                    change: Change::ChangeCost(CostPool::Energy, -1),
                    lasting: Lasting::ThisTurn,
                });
            }
        }
        // `Pinpoint.AfterCardEnteredCombat`, for its own entry alone: one off
        // for each Skill played this turn before it came.
        (SilentCard::Pinpoint, EventHook::CardEnteredCombat { at, .. }) => {
            if at.id() == id {
                unimplemented!("a Pinpoint entering the fight, which counts the turn's Skills")
            }
        }
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
}
