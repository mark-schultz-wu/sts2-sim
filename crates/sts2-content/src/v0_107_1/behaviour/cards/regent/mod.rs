//! The Regent's cards, filed by rarity.

use sts2_core::{CardTable, Fight, Played};

use super::super::super::generated::cards::{CardRarity, Cards, RegentCard};
use super::{Change, V};

mod basic;
mod common;
mod rare;
mod uncommon;

/// Where a Regent card stopped inside its own play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    /// `Begone`, after it selected the card to turn into a Minion Strike.
    BegoneTransform,
}

/// `Upgrade` for the Regent's cards, under the rarity the table files each
/// one by.
///
/// A rarity gets a module when its first card is upgraded. A card from a
/// rarity nobody has reached stops here instead.
pub(super) fn upgrade(card: RegentCard) -> &'static [Change] {
    match Cards::Regent(card).rarity() {
        CardRarity::Basic => basic::upgrade(card),
        CardRarity::Common => common::upgrade(card),
        CardRarity::Uncommon => uncommon::upgrade(card),
        CardRarity::Rare => rare::upgrade(card),
        _ => unimplemented!("card upgrade: {card:?}"),
    }
}

pub(super) fn on_play<F: Fight<V>>(card: RegentCard, fight: &mut F) -> Played<V> {
    match card {
        RegentCard::Strike => basic::strike(fight),
        RegentCard::Defend => basic::defend(fight),
        RegentCard::FallingStar => basic::falling_star(fight),
        RegentCard::Venerate => basic::venerate(fight),
        RegentCard::GuidingStar => common::guiding_star(fight),
        RegentCard::Glitterstream => common::glitterstream(fight),
        RegentCard::KnowThyPlace => common::know_thy_place(fight),
        RegentCard::RefineBlade => common::refine_blade(fight),
        RegentCard::SpoilsOfBattle => common::spoils_of_battle(fight),
        RegentCard::Begone => common::begone(),
        RegentCard::CrushUnder => common::crush_under(fight),
        RegentCard::Bulwark => uncommon::bulwark(fight),
        RegentCard::TheSmith => rare::the_smith(fight),
        RegentCard::BigBang => rare::big_bang(fight),
        card => unimplemented!("what {card:?} does when it is played"),
    }
}

/// The rest of a play that stopped, once the fight has its answer.
pub(super) fn resume_play<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        Resume::BegoneTransform => common::begone_transform(fight, chosen),
    }
}
