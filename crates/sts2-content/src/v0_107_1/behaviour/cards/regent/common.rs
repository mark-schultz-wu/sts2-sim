//! The Regent's common cards.

use sts2_core::{Fight, PowerAmount};

use super::super::super::super::generated::cards::{CardVar, Cards, RegentCard, TokenCard};
use super::super::super::super::generated::powers::Power;
use super::super::{Number, Printed, V, raise};

/// `Upgrade`, which raises what these cards print.
pub(super) fn upgrade(card: RegentCard, printed: &mut [Printed]) {
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
    fight.attack(target, fight.var(CardVar::Damage));
    fight.draw(fight.var(CardVar::Cards));
}

/// `Glitterstream`. Gain block now and after the next block clear.
pub(super) fn glitterstream<F: Fight<V>>(fight: &mut F) {
    fight.gain_block(fight.var(CardVar::Block));
    fight.apply_to_actor(
        Power::BlockNextTurnPower,
        PowerAmount::Counted(fight.var(CardVar::BlockNextTurn)),
    );
}

/// `KnowThyPlace`. Apply Weak, then Vulnerable, to its target.
pub(super) fn know_thy_place<F: Fight<V>>(fight: &mut F) {
    let target = fight.target();
    fight.apply(
        target,
        Power::WeakPower,
        PowerAmount::Counted(fight.var(CardVar::WeakPower)),
    );
    fight.apply(
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
