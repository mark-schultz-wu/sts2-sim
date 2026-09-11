//! The Regent's cards, filed by rarity.

use sts2_core::Fight;

use super::super::super::generated::cards::RegentCard;
use super::V;

mod basic;
mod common;

pub(super) fn on_play<F: Fight<V>>(card: RegentCard, fight: &mut F) {
    match card {
        RegentCard::Strike => basic::strike(fight),
        RegentCard::Defend => basic::defend(fight),
        RegentCard::FallingStar => basic::falling_star(fight),
        RegentCard::Venerate => basic::venerate(fight),
        RegentCard::GuidingStar => common::guiding_star(fight),
        RegentCard::RefineBlade => common::refine_blade(fight),
        card => unimplemented!("what {card:?} does when it is played"),
    }
}
