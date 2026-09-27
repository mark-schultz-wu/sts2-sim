//! Which cards an enchantment takes, and what it does on them in a fight.

use sts2_core::{
    CardId, CardIdent, CardKind, CardTable as _, Cost, EnchantHook, Enchanted, EnchantmentStatus,
    EventHook, Fight, HookEffect, HookEffects, Modify, Rules as _,
};

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
        Enchantment::Sown | Enchantment::Swift => base,
        // `CanEnchantCardType`, which takes an attack alone.
        Enchantment::Corrupted | Enchantment::Sharp | Enchantment::Vigorous => {
            base && V::card_kind(card.card) == Some(CardKind::Attack)
        }
        Enchantment::Nimble => base && card.card.gains_block(),
        Enchantment::Goopy => base && card.card.tagged(CardTag::Defend),
        _ => unimplemented!("which cards {enchantment:?} can enchant"),
    }
}

/// `CardCmd.Enchant` by `amount`, and the enchantment's `OnEnchant`.
///
/// # Panics
///
/// An enchantment whose `OnEnchant` changes the card, which this port has
/// not reached.
pub(crate) fn enchant(enchantment: Enchantment, amount: u8, card: &mut CardIdent<V>) {
    match enchantment {
        Enchantment::Corrupted
        | Enchantment::Vigorous
        | Enchantment::Nimble
        | Enchantment::PerfectFit
        | Enchantment::Sharp
        | Enchantment::Slither
        | Enchantment::Sown
        | Enchantment::Spiral
        | Enchantment::Swift => {
            card.enchantment = Some(Enchanted {
                enchantment,
                amount,
            });
        }
        _ => unimplemented!("what enchanting a card with {enchantment:?} does to it"),
    }
}

/// An enchantment's `EnchantDamageAdditive`, `EnchantDamageMultiplicative`,
/// `EnchantBlockAdditive` or `EnchantPlayCount`, over the card it is on.
pub(crate) fn modify(
    enchanted: Enchanted<V>,
    status: EnchantmentStatus,
    hook: EnchantHook,
) -> Modify<i32> {
    let amount = i32::from(enchanted.amount);
    match (enchanted.enchantment, hook) {
        // Sharp's own check: a powered attack, and nothing for any other hit.
        (Enchantment::Sharp, EnchantHook::DamageAdditive { powered: true }) => {
            Modify::Added(amount)
        }
        (Enchantment::Corrupted, EnchantHook::DamageMultiplicative { powered: true }) => {
            Modify::Scaled(1.5)
        }
        (Enchantment::Nimble, EnchantHook::BlockAdditive) => Modify::Added(amount),
        // Its amount on a powered hit, until its card is first played.
        (Enchantment::Vigorous, EnchantHook::DamageAdditive { powered: true })
            if status == EnchantmentStatus::Normal =>
        {
            Modify::Added(amount)
        }
        // Its `Times`, which it prints as one.
        (Enchantment::Spiral, EnchantHook::PlayCount) => Modify::Added(1),
        (
            Enchantment::Corrupted
            | Enchantment::Vigorous
            | Enchantment::Nimble
            | Enchantment::PerfectFit
            | Enchantment::Sharp
            | Enchantment::Slither
            | Enchantment::Sown
            | Enchantment::Spiral
            | Enchantment::Swift,
            _,
        ) => Modify::Nothing,
        (enchantment, hook) => unimplemented!("what {enchantment:?} does for {hook:?}"),
    }
}

/// What an enchantment asks for when `hook` reaches the card `id` it is on.
/// The fight asks only an enchantment whose class overrides the hook.
pub(crate) fn event_hook(
    enchanted: Enchanted<V>,
    id: CardId<V>,
    hook: EventHook<V>,
    asked: &mut HookEffects<V>,
) {
    match (enchanted.enchantment, hook) {
        // `CombatEnergyCosts.NextInt(4)` for its own card, which the fight
        // draws only while that card is in hand.
        (Enchantment::Slither, EventHook::CardDrawn { id: drawn, .. }) => {
            if drawn == id {
                asked.push(HookEffect::RandomSelfCost { below: 4 });
            }
        }
        // `AfterCardPlayed` for its own card: spent once that play is over.
        (Enchantment::Vigorous, EventHook::CardPlayed { id: played, .. }) => {
            if played == id {
                asked.push(HookEffect::DisableOwnEnchantment);
            }
        }
        // To the top of a shuffle, but not the one a fight opens on.
        (Enchantment::PerfectFit, EventHook::ShuffleOrder { initial }) => {
            if !initial {
                asked.push(HookEffect::ToTopOfDrawPile);
            }
        }
        (enchantment, hook) => unimplemented!("what {enchantment:?} does for {hook:?}"),
    }
}

/// `EnchantmentModel.OnPlay`, after each play of the card it is on.
pub(crate) fn on_play<F: Fight<V>>(
    enchanted: Enchanted<V>,
    status: &mut EnchantmentStatus,
    fight: &mut F,
) {
    let amount = i32::from(enchanted.amount);
    match enchanted.enchantment {
        Enchantment::Sown => {
            if *status == EnchantmentStatus::Normal {
                fight.modify_energy(amount);
                *status = EnchantmentStatus::Disabled;
            }
        }
        Enchantment::Swift => {
            if *status == EnchantmentStatus::Normal {
                fight.draw(amount);
                *status = EnchantmentStatus::Disabled;
            }
        }
        // Two to its owner, unblockable and unpowered, on every play.
        Enchantment::Corrupted => {
            let owner = fight.actor();
            fight.damage_unblockable(owner, 2);
        }
        Enchantment::Nimble
        | Enchantment::PerfectFit
        | Enchantment::Sharp
        | Enchantment::Slither
        | Enchantment::Spiral
        | Enchantment::Vigorous => {}
        enchantment => unimplemented!("what {enchantment:?} does when its card is played"),
    }
}
