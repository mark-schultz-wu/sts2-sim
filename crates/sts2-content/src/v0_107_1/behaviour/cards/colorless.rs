//! The cards no character's pool holds.

use sts2_core::Fight;

use super::super::super::generated::cards::{CardVar, Cards, ColorlessCard};
use super::{V, printed};

pub(super) fn on_play<F: Fight<V>>(card: ColorlessCard, fight: &mut F) {
    let id = Cards::Colorless(card);
    match card {
        ColorlessCard::FlashOfSteel => flash_of_steel(id, fight),
        card => unimplemented!("what {card:?} does when it is played"),
    }
}

/// `FlashOfSteel`. Deal damage, then draw.
fn flash_of_steel<F: Fight<V>>(card: Cards, fight: &mut F) {
    let target = fight.target();
    fight.attack(target, printed(card, CardVar::Damage));
    fight.draw(printed(card, CardVar::Cards));
}
