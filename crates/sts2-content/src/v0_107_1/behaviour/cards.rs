//! The cards whose rules this build ports for itself, which the shared cards
//! hand over through [`crate::build::OwnRules`].

use sts2_core::{Fight, Played, PowerAmount};

use super::super::V0_107_1 as V;
use super::super::generated::cards::{CardVar, Cards, IroncladCard, SilentCard};
use super::super::generated::powers::Power;

type Number = i32;

/// What a card of this build's own does when it is played, or nothing for a
/// shared one.
pub(crate) fn play<F: Fight<V>>(card: Cards, fight: &mut F) -> Option<Played<V>> {
    Some(match card {
        // `ExpectAFight`: its calculated energy, then No Energy Gain on its
        // owner.
        Cards::Ironclad(IroncladCard::ExpectAFight) => {
            fight.modify_energy(fight.var(CardVar::CalculatedEnergy));
            fight.apply_to_actor(Power::NoEnergyGainPower, PowerAmount::Counted(1));
            Played::Done
        }
        // `Scare`: one Weak on each enemy that can be hit.
        Cards::Silent(SilentCard::Scare) => {
            for target in fight.hittable_opponents() {
                fight.apply_power(target, Power::WeakPower, PowerAmount::Counted(1));
            }
            Played::Done
        }
        _ => return None,
    })
}

/// A number a card of this build's own works out, or nothing for a shared one.
pub(crate) fn calculated<P: sts2_core::Printed<V> + ?Sized>(
    card: Cards,
    key: CardVar,
    printed: &P,
    owner: sts2_core::CardOwner<'_, V>,
) -> Option<Number> {
    Some(match (card, key) {
        // `ExpectAFight`: a base, and one more energy for each Attack in its
        // owner's hand.
        (Cards::Ironclad(IroncladCard::ExpectAFight), CardVar::CalculatedEnergy) => {
            let attacks = owner
                .hand
                .iter()
                .filter(|&&card| {
                    <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Attack)
                })
                .count();
            let attacks = Number::try_from(attacks).expect("a hand fits the game's numbers");
            printed.face(CardVar::CalculationBase)
                + printed.face(CardVar::CalculationExtra) * attacks
        }
        _ => return None,
    })
}
