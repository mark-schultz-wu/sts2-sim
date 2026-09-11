//! The cards no character's pool holds.

use sts2_core::Fight;

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
    fight.attack(target, fight.var(CardVar::Damage));
    fight.draw(fight.var(CardVar::Cards));
}
