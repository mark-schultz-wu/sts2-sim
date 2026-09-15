//! The Regent's cards, filed by rarity.

use sts2_core::{CardTable, Fight};

use super::super::super::generated::cards::{CardRarity, Cards, RegentCard};
use super::{CardNumber, V};

mod basic;
mod common;

/// `Upgrade` for the Regent's cards, under the rarity the table files each
/// one by.
///
/// A rarity gets a module when its first card is upgraded. A card from a
/// rarity nobody has reached stops here instead.
pub(super) fn upgrade(card: RegentCard, printed: &mut [CardNumber]) {
    match Cards::Regent(card).rarity() {
        CardRarity::Basic => basic::upgrade(card, printed),
        CardRarity::Common => common::upgrade(card, printed),
        _ => unimplemented!("card upgrade: {card:?}"),
    }
}

pub(super) fn on_play<F: Fight<V>>(card: RegentCard, fight: &mut F) {
    match card {
        RegentCard::Strike => basic::strike(fight),
        RegentCard::Defend => basic::defend(fight),
        RegentCard::FallingStar => basic::falling_star(fight),
        RegentCard::Venerate => basic::venerate(fight),
        RegentCard::GuidingStar => common::guiding_star(fight),
        RegentCard::Glitterstream => common::glitterstream(fight),
        RegentCard::KnowThyPlace => common::know_thy_place(fight),
        RegentCard::RefineBlade => common::refine_blade(fight),
        RegentCard::Begone => common::begone(fight),
        card => unimplemented!("what {card:?} does when it is played"),
    }
}
