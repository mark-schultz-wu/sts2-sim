//! The cards no character's pool holds.

use sts2_core::{Amount, Effect, Fight};

use super::super::super::generated::cards::{CardVar, ColorlessCard};
use super::V;

pub(super) fn on_play<F: Fight<V>>(card: ColorlessCard, fight: &mut F) {
    match card {
        ColorlessCard::FlashOfSteel => flash_of_steel(fight),
        card => unimplemented!("what {card:?} does when it is played"),
    }
}

/// `FlashOfSteel`. Deal damage, then draw.
fn flash_of_steel<F: Fight<V>>(fight: &mut F) {
    let target = fight.target();
    let attack = fight.resolve(Effect::Attack {
        dealer: fight.actor(),
        target,
        damage: Amount::Printed(CardVar::Damage),
    });
    fight.apply(attack);
    fight.draw(fight.var(CardVar::Cards));
}
