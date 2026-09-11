//! The Regent's common cards.

use sts2_core::{Fight, PowerAmount};

use super::super::super::super::generated::cards::{CardVar, Cards, TokenCard};
use super::super::super::super::generated::powers::Power;
use super::super::V;

/// `GuidingStar`. Deal damage, then draw.
pub(super) fn guiding_star<F: Fight<V>>(fight: &mut F) {
    let target = fight.target();
    fight.attack(target, fight.var(CardVar::Damage));
    fight.draw(fight.var(CardVar::Cards));
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
