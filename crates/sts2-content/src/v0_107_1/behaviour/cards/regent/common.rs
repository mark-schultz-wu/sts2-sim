//! The Regent's common cards.

use sts2_core::{
    Amount, CardIdent, Effect, EffectResolution, Fight, Pile, Played, PowerAmount, Scaling,
    Selection,
};

use super::super::super::super::generated::cards::{CardVar, Cards, RegentCard, TokenCard};
use super::super::super::super::generated::powers::Power;
use super::super::{Change, V};
use super::Resume;

/// `Upgrade`, which raises what these cards print.
pub(super) fn upgrade(card: RegentCard) -> &'static [Change] {
    match card {
        RegentCard::GuidingStar => &[
            Change::Raise(CardVar::Damage, 1),
            Change::Raise(CardVar::Cards, 1),
        ],
        // `Begone` prints no number an upgrade raises. Upgraded, its own
        // `OnPlay` upgrades the Minion Strike it transforms a card into.
        RegentCard::Begone => &[],
        RegentCard::CrushUnder => &[
            Change::Raise(CardVar::Damage, 1),
            Change::Raise(CardVar::StrengthLoss, 1),
        ],
        RegentCard::Glitterstream => &[
            Change::Raise(CardVar::Block, 2),
            Change::Raise(CardVar::BlockNextTurn, 2),
        ],
        RegentCard::SpoilsOfBattle => &[Change::Raise(CardVar::Forge, 3)],
        card => unimplemented!("card upgrade: {card:?}"),
    }
}

/// `GuidingStar`. Deal damage, then draw.
pub(super) fn guiding_star<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    let attack = fight.resolve(Effect::Attack {
        dealer: fight.actor(),
        target,
        damage: Amount::Var(CardVar::Damage),
        scaling: Scaling::Dynamic,
    });
    fight.apply(attack);
    fight.draw(fight.var(CardVar::Cards));
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
        at: super::super::Resume::Regent(Resume::BegoneTransform),
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

/// `CrushUnder`. Hit every enemy, then take a point of Strength off each.
///
/// The command takes the creatures across before its first hit lands, and the
/// powers go to that same list. One the damage took out holds no combat state
/// by then, and `PowerCmd.Apply` returns on such a creature.
pub(super) fn crush_under<F: Fight<V>>(fight: &mut F) -> Played<V> {
    for target in fight.opponents() {
        let attack = fight.resolve(Effect::Attack {
            dealer: fight.actor(),
            target,
            damage: Amount::Var(CardVar::Damage),
            scaling: Scaling::Dynamic,
        });
        fight.apply(attack);
    }
    fight.apply_to_opponents(
        Power::CrushUnderPower,
        PowerAmount::Counted(fight.var(CardVar::StrengthLoss)),
    );
    Played::Done
}

/// `KnowThyPlace`. Apply Weak, then Vulnerable, to its target.
pub(super) fn know_thy_place<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    fight.apply_power(
        target,
        Power::WeakPower,
        PowerAmount::Counted(fight.var(CardVar::WeakPower)),
    );
    fight.apply_power(
        target,
        Power::VulnerablePower,
        PowerAmount::Counted(fight.var(CardVar::VulnerablePower)),
    );
    Played::Done
}

/// `RefineBlade`. Forge every Sovereign Blade, then gain energy next turn.
///
/// `ForgeCmd.Forge` creates a blade when the fight holds none it can reach,
/// and raises the damage on every one it holds. A blade in the exhaust pile is
/// raised and does not count as one it can reach.
pub(super) fn refine_blade<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let forge = fight.var(CardVar::Forge);
    fight.forge(
        Cards::Token(TokenCard::SovereignBlade),
        CardVar::Damage,
        forge,
    );
    fight.apply_to_actor(
        Power::EnergyNextTurnPower,
        PowerAmount::Counted(fight.var(CardVar::Energy)),
    );
    Played::Done
}
