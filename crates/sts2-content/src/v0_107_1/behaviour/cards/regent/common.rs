//! The Regent's common cards.

use sts2_core::{
    Amount, BlockProps, CardIdent, Effect, EffectResolution, Fight, PowerAmount, Selection,
};

use super::super::super::super::generated::cards::{CardVar, Cards, RegentCard, TokenCard};
use super::super::super::super::generated::powers::Power;
use super::super::{CardNumber, Number, V, raise};

/// `Upgrade`, which raises what these cards print.
pub(super) fn upgrade(card: RegentCard, printed: &mut [CardNumber]) {
    let changes: &[(CardVar, Number)] = match card {
        RegentCard::GuidingStar => &[(CardVar::Damage, 1), (CardVar::Cards, 1)],
        // `Begone` prints no number an upgrade raises. Upgraded, its own
        // `OnPlay` upgrades the Minion Strike it transforms a card into.
        RegentCard::Begone => &[],
        card => unimplemented!("card upgrade: {card:?}"),
    };
    raise(printed, changes);
}

/// `GuidingStar`. Deal damage, then draw.
pub(super) fn guiding_star<F: Fight<V>>(fight: &mut F) {
    let target = fight.target();
    let attack = fight.resolve(Effect::Attack {
        dealer: fight.actor(),
        target,
        damage: Amount::Var(CardVar::Damage),
    });
    fight.apply(attack);
    fight.draw(fight.var(CardVar::Cards));
}

/// `Glitterstream`. Gain block now and after the next block clear.
///
/// The number the power carries runs through `Hook.ModifyBlock` first, which
/// this card does itself rather than leave to the gain. It reads the hook
/// before the block it gains lands, as the game's own order does.
pub(super) fn glitterstream<F: Fight<V>>(fight: &mut F) {
    let next_turn = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(CardVar::BlockNextTurn),
        props: BlockProps::Move,
    });
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(CardVar::Block),
        props: BlockProps::Move,
    });
    fight.apply(block);
    fight.apply_to_actor(
        Power::BlockNextTurnPower,
        PowerAmount::Counted(next_turn.amount()),
    );
}

/// `Begone`. Turn a card in hand into a Minion Strike.
///
/// The selection takes the hand itself while the hand holds one card, so this
/// asks nothing there. An empty hand selects nothing and the card does
/// nothing, which is `FirstOrDefault` finding none.
///
/// Upgraded, the Minion Strike it creates is upgraded with it. Begone prints
/// no number for an upgrade to raise.
pub(super) fn begone<F: Fight<V>>(fight: &mut F) {
    let Some(&card) = fight
        .select_cards_from_hand(Selection::Exactly(nonmax::NonMaxUsize::ONE))
        .first()
    else {
        return;
    };
    let mut into = CardIdent::new(Cards::Token(TokenCard::MinionStrike));
    into.upgrade = u8::from(fight.upgraded());
    fight.transform(card, into);
}

/// `KnowThyPlace`. Apply Weak, then Vulnerable, to its target.
pub(super) fn know_thy_place<F: Fight<V>>(fight: &mut F) {
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
}

/// `RefineBlade`. Forge every Sovereign Blade, then gain energy next turn.
///
/// `ForgeCmd.Forge` creates a blade when the fight holds none it can reach,
/// and raises the damage on every one it holds. A blade in the exhaust pile is
/// raised and does not count as one it can reach.
pub(super) fn refine_blade<F: Fight<V>>(fight: &mut F) {
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
}
