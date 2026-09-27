//! The colorless cards the generator leaves to a hand port.

use sts2_core::{Amount, Effect, Fight, Played, Scaling};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardVar, ColorlessCard};
use super::V;

pub(super) fn on_play<F: Fight<V>>(card: ColorlessCard, fight: &mut F) -> Played<V> {
    match card {
        // `DamageCmd.Attack(CalculatedDamage).FromCard(this)` at the target.
        ColorlessCard::MindBlast => {
            let target = fight.target();
            fight.attack(Scaling::Dynamic, |fight| {
                let hit = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(CardVar::CalculatedDamage),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(hit);
            });
            Played::Done
        }
        card => card_plays::colorless::on_play(card, fight),
    }
}
