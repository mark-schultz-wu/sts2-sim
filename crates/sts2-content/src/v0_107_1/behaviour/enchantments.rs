//! Which cards an enchantment takes.

use sts2_core::{CardIdent, CardKind, CardTable as _, Cost, Rules as _};

use super::super::V0_107_1 as V;
use super::super::generated::cards::{CardRarity, CardTag};
use super::super::generated::enchantments::Enchantment;

/// `EnchantmentModel.CanEnchant`, for a card in the deck.
///
/// The base refuses a status, a curse or a quest, an unplayable card, and a
/// card enchanted already. No enchantment in this build is stackable.
pub(crate) fn can_enchant(enchantment: Enchantment, card: &CardIdent<V>) -> bool {
    let base = !matches!(
        V::card_kind(card.card),
        Some(CardKind::Status | CardKind::Curse | CardKind::Quest)
    ) && !card.card.unplayable()
        && card.enchantment.is_none();
    match enchantment {
        Enchantment::PerfectFit => base,
        Enchantment::Spiral => {
            base && card.card.rarity() == CardRarity::Basic
                && (card.card.tagged(CardTag::Strike) || card.card.tagged(CardTag::Defend))
        }
        Enchantment::SoulsPower => base && card.card.exhausts(),
        // Its own check adds an X cost to the base's refusals.
        Enchantment::Slither => base && card.card.cost() != Cost::X,
        Enchantment::Sown => base,
        _ => unimplemented!("which cards {enchantment:?} can enchant"),
    }
}

/// `CardCmd.Enchant` with an amount of one, and the enchantment's `OnEnchant`.
///
/// # Panics
///
/// An enchantment whose `OnEnchant` changes the card, which this port has
/// not reached.
pub(crate) fn enchant(enchantment: Enchantment, card: &mut CardIdent<V>) {
    match enchantment {
        Enchantment::PerfectFit
        | Enchantment::Slither
        | Enchantment::Sown
        | Enchantment::Spiral => {
            card.enchantment = Some(enchantment);
        }
        _ => unimplemented!("what enchanting a card with {enchantment:?} does to it"),
    }
}
