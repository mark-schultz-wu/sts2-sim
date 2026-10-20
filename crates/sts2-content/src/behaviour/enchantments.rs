//! Which cards an enchantment takes, and what it does on them in a fight.

use sts2_core::{
    CardId, CardIdent, CardKind, CardTable as _, Change, Cost, EnchantHook, Enchanted,
    EnchantmentStatus, EventHook, Fight, HookEffect, HookEffects, Modify,
};

use crate::generated::cards::{CardKeyword, CardRarity, CardTag};
use crate::ids::{EnchantmentIds as _, PowerIds as _};

/// `EnchantmentModel.CanEnchant`, for a card in the deck.
///
/// The base refuses a status, a curse or a quest, an unplayable card, and a
/// card enchanted already. No enchantment in this build is stackable.
pub(crate) fn can_enchant<V: crate::Build>(
    enchantment: V::Enchantment,
    card: &CardIdent<V>,
) -> bool {
    let base = !matches!(
        V::card_kind(card.card),
        Some(CardKind::Status | CardKind::Curse | CardKind::Quest)
    ) && !card.card.unplayable()
        && card.enchantment.is_none();
    match enchantment {
        __c0 if __c0 == V::Enchantment::PERFECT_FIT => base,
        __c0 if __c0 == V::Enchantment::SPIRAL => {
            base && card.card.rarity() == CardRarity::Basic
                && (card.card.tagged(CardTag::Strike) || card.card.tagged(CardTag::Defend))
        }
        // Its own check adds an X cost to the base's refusals.
        __c0 if __c0 == V::Enchantment::SLITHER => base && card.card.cost() != Cost::X,
        __c0 if __c0 == V::Enchantment::GLAM
            || __c0 == V::Enchantment::SOWN
            || __c0 == V::Enchantment::SWIFT
            || __c0 == V::Enchantment::ADROIT
            || __c0 == V::Enchantment::INKY
            || __c0 == V::Enchantment::TEZCATARAS_EMBER
            || __c0 == V::Enchantment::CLONE =>
        {
            base
        }
        // Exhaust among the card's own keywords, which its upgrade can take
        // off.
        __c0 if __c0 == V::Enchantment::SOULS_POWER => base && card_exhausts::<V>(card),
        // `CanEnchantCardType`, which takes an attack alone.
        __c0 if __c0 == V::Enchantment::CORRUPTED
            || __c0 == V::Enchantment::INSTINCT
            || __c0 == V::Enchantment::SHARP
            || __c0 == V::Enchantment::VIGOROUS
            || __c0 == V::Enchantment::MOMENTUM =>
        {
            base && V::card_kind(card.card) == Some(CardKind::Attack)
        }
        __c0 if __c0 == V::Enchantment::NIMBLE => base && card.card.gains_block(),
        // `CanEnchantCardType`, which takes a skill alone.
        __c0 if __c0 == V::Enchantment::IMBUED => {
            base && V::card_kind(card.card) == Some(CardKind::Skill)
        }
        // `CanEnchantCardType`, which takes an attack or a skill.
        // `Steady` overrides no check, so the base answers.
        __c0 if __c0 == V::Enchantment::STEADY => base,
        __c0 if __c0 == V::Enchantment::ROYALLY_APPROVED => {
            base && matches!(
                V::card_kind(card.card),
                Some(CardKind::Attack | CardKind::Skill)
            )
        }
        __c0 if __c0 == V::Enchantment::GOOPY => base && card.card.tagged(CardTag::Defend),
        _ => unimplemented!("which cards {enchantment:?} can enchant"),
    }
}

/// `GetKeywordsWithSources(Local).Contains(Exhaust)`: what the card prints,
/// then what its upgrade writes where it has one.
fn card_exhausts<V: crate::Build>(card: &CardIdent<V>) -> bool {
    let upgrade: &[Change<V>] = if card.upgrade > 0 {
        V::card_upgrade(card.card)
    } else {
        &[]
    };
    upgrade
        .iter()
        .fold(card.card.exhausts(), |exhausts, change| match change {
            Change::AddKeyword(CardKeyword::Exhaust) => true,
            Change::RemoveKeyword(CardKeyword::Exhaust) => false,
            _ => exhausts,
        })
}

/// `OnEnchant`'s writes on the card. Souls Power takes its Exhaust off.
pub(crate) fn enchanted_changes<V: crate::Build>(
    enchantment: V::Enchantment,
) -> &'static [Change<V>] {
    match enchantment {
        __c0 if __c0 == V::Enchantment::SOULS_POWER => {
            &[Change::RemoveKeyword(CardKeyword::Exhaust)]
        }
        // `EnergyCost.UpgradeBy(-cost)` takes the cost to nothing, which
        // later modifiers write over, then `AddKeyword(Eternal)`.
        __c0 if __c0 == V::Enchantment::TEZCATARAS_EMBER => &[
            Change::SetCost(sts2_core::CostPool::Energy, 0),
            Change::AddKeyword(CardKeyword::Eternal),
        ],
        __c0 if __c0 == V::Enchantment::GOOPY => &[Change::AddKeyword(CardKeyword::Exhaust)],
        __c0 if __c0 == V::Enchantment::ROYALLY_APPROVED => &[
            Change::AddKeyword(CardKeyword::Innate),
            Change::AddKeyword(CardKeyword::Retain),
        ],
        __c0 if __c0 == V::Enchantment::STEADY => &[Change::AddKeyword(CardKeyword::Retain)],
        _ => &[],
    }
}

/// Whether `card` carries `Eternal`: its table prints it, or its
/// enchantment's `OnEnchant` adds it.
pub(crate) fn card_eternal<V: crate::Build>(card: &CardIdent<V>) -> bool {
    use sts2_core::CardTable as _;

    card.card.eternal()
        || card.enchantment.is_some_and(|enchanted| {
            enchanted_changes::<V>(enchanted.enchantment)
                .contains(&Change::AddKeyword(CardKeyword::Eternal))
        })
}

/// `CardCmd.Enchant` by `amount`, and the enchantment's `OnEnchant`.
///
/// # Panics
///
/// An enchantment whose `OnEnchant` changes the card, which this port has
/// not reached.
pub(crate) fn enchant<V: crate::Build>(
    enchantment: V::Enchantment,
    amount: u8,
    card: &mut CardIdent<V>,
) {
    match enchantment {
        __c0 if __c0 == V::Enchantment::ADROIT
            || __c0 == V::Enchantment::MOMENTUM
            || __c0 == V::Enchantment::CORRUPTED
            || __c0 == V::Enchantment::GLAM
            || __c0 == V::Enchantment::INKY
            || __c0 == V::Enchantment::INSTINCT
            || __c0 == V::Enchantment::VIGOROUS
            || __c0 == V::Enchantment::NIMBLE
            || __c0 == V::Enchantment::PERFECT_FIT
            || __c0 == V::Enchantment::SHARP
            || __c0 == V::Enchantment::SLITHER
            || __c0 == V::Enchantment::SOULS_POWER
            || __c0 == V::Enchantment::SOWN
            || __c0 == V::Enchantment::SPIRAL
            || __c0 == V::Enchantment::SWIFT
            || __c0 == V::Enchantment::TEZCATARAS_EMBER
            || __c0 == V::Enchantment::GOOPY
            || __c0 == V::Enchantment::CLONE
            || __c0 == V::Enchantment::IMBUED
            || __c0 == V::Enchantment::ROYALLY_APPROVED
            || __c0 == V::Enchantment::STEADY =>
        {
            card.enchantment = Some(Enchanted {
                enchantment,
                amount,
            });
        }
        _ => unimplemented!("what enchanting a card with {enchantment:?} does to it"),
    }
}

/// `EnchantmentModel.ShouldStartAtBottomOfDrawPile`, which Imbued alone
/// overrides.
///
/// # Panics
///
/// An enchantment this port has not read.
pub(crate) fn should_start_at_bottom_of_draw_pile<V: crate::Build>(
    enchantment: V::Enchantment,
) -> sts2_core::HookResponse<bool> {
    use sts2_core::HookResponse;

    match enchantment {
        __c0 if __c0 == V::Enchantment::IMBUED => HookResponse::Decision(true),
        __c0 if __c0 == V::Enchantment::ADROIT
            || __c0 == V::Enchantment::CLONE
            || __c0 == V::Enchantment::CORRUPTED
            || __c0 == V::Enchantment::GLAM
            || __c0 == V::Enchantment::GOOPY
            || __c0 == V::Enchantment::INKY
            || __c0 == V::Enchantment::INSTINCT
            || __c0 == V::Enchantment::MOMENTUM
            || __c0 == V::Enchantment::NIMBLE
            || __c0 == V::Enchantment::PERFECT_FIT
            || __c0 == V::Enchantment::SHARP
            || __c0 == V::Enchantment::SLITHER
            || __c0 == V::Enchantment::SOULS_POWER
            || __c0 == V::Enchantment::SOWN
            || __c0 == V::Enchantment::SPIRAL
            || __c0 == V::Enchantment::SWIFT
            || __c0 == V::Enchantment::TEZCATARAS_EMBER
            || __c0 == V::Enchantment::VIGOROUS
            || __c0 == V::Enchantment::ROYALLY_APPROVED
            || __c0 == V::Enchantment::STEADY =>
        {
            HookResponse::DoesNotImplement
        }
        enchantment => unimplemented!("whether {enchantment:?} starts at the draw pile's bottom"),
    }
}

/// `Inky`'s `DamageVar(1, ValueProp.Move)`.
const INKY_DAMAGE: i32 = 1;

/// `TezcatarasEmber`'s `DamageVar(3)`.
const EMBER_DAMAGE: i32 = 3;

/// `Inky`'s `PowerVar<WeakPower>(1)`.
const INKY_WEAK: i32 = 1;

/// One enchantment on one copy: its `Status`, and the fields its own class
/// keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct State {
    status: EnchantmentStatus,
    own: Own,
}

/// The fields an enchantment's class adds to `EnchantmentModel`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Own {
    /// A class that adds none.
    Nothing,
    /// `Momentum.ExtraDamage`, which each play raises by its amount.
    Momentum { extra_damage: i16 },
}

impl<V: crate::Build> sts2_core::EnchantmentModel<V> for State {
    fn new(enchantment: Option<V::Enchantment>) -> Self {
        let own = match enchantment {
            Some(__c0) if __c0 == V::Enchantment::MOMENTUM => Own::Momentum { extra_damage: 0 },
            _ => Own::Nothing,
        };
        State {
            status: EnchantmentStatus::Normal,
            own,
        }
    }

    fn status(&self) -> EnchantmentStatus {
        self.status
    }

    fn set_status(&mut self, status: EnchantmentStatus) {
        self.status = status;
    }
}

/// An enchantment's `EnchantDamageAdditive`, `EnchantDamageMultiplicative`,
/// `EnchantBlockAdditive` or `EnchantPlayCount`, over the card it is on.
pub(crate) fn modify<V: crate::Build>(
    enchanted: Enchanted<V>,
    state: &State,
    hook: EnchantHook,
) -> Modify<i32> {
    let amount = i32::from(enchanted.amount);
    let status = state.status;
    match (enchanted.enchantment, hook) {
        // `Momentum`: what its plays have added, on a powered hit.
        (__c0, EnchantHook::DamageAdditive { powered: true })
            if __c0 == V::Enchantment::MOMENTUM =>
        {
            match state.own {
                Own::Momentum { extra_damage } => Modify::Added(i32::from(extra_damage)),
                Own::Nothing => unreachable!("a Momentum keeps its extra damage"),
            }
        }
        // Sharp's own check: a powered attack, and nothing for any other hit.
        (__c0, EnchantHook::DamageAdditive { powered: true }) if __c0 == V::Enchantment::SHARP => {
            Modify::Added(amount)
        }
        (__c0, EnchantHook::DamageMultiplicative { powered: true })
            if __c0 == V::Enchantment::CORRUPTED =>
        {
            Modify::Scaled(sts2_core::Fixed::new(15, 1))
        }
        // `Instinct`: double on a powered hit.
        (__c0, EnchantHook::DamageMultiplicative { powered: true })
            if __c0 == V::Enchantment::INSTINCT =>
        {
            Modify::Scaled(sts2_core::Fixed::new(2, 0))
        }
        // `Inky`: its `DamageVar(1)` on a powered hit, whatever its amount.
        (__c0, EnchantHook::DamageAdditive { powered: true }) if __c0 == V::Enchantment::INKY => {
            Modify::Added(INKY_DAMAGE)
        }
        (__c0, EnchantHook::BlockAdditive) if __c0 == V::Enchantment::NIMBLE => {
            Modify::Added(amount)
        }
        // `Goopy`: one less than its amount, which each play raises.
        (__c0, EnchantHook::BlockAdditive) if __c0 == V::Enchantment::GOOPY => {
            Modify::Added(amount - 1)
        }
        // Its amount on a powered hit, until its card is first played.
        (__c0, EnchantHook::DamageAdditive { powered: true })
            if __c0 == V::Enchantment::VIGOROUS && (status == EnchantmentStatus::Normal) =>
        {
            Modify::Added(amount)
        }
        // Its `Times`, which it prints as one.
        (__c0, EnchantHook::PlayCount) if __c0 == V::Enchantment::SPIRAL => Modify::Added(1),
        // `TezcatarasEmber`: its `DamageVar(3)` on a powered hit.
        (__c0, EnchantHook::DamageAdditive { powered: true })
            if __c0 == V::Enchantment::TEZCATARAS_EMBER =>
        {
            Modify::Added(EMBER_DAMAGE)
        }
        // Its `Times`, one, until its card is first played in the fight.
        (__c0, EnchantHook::PlayCount)
            if __c0 == V::Enchantment::GLAM && (status == EnchantmentStatus::Normal) =>
        {
            Modify::Added(1)
        }
        (__c0, _)
            if (__c0 == V::Enchantment::ADROIT
                || __c0 == V::Enchantment::CLONE
                || __c0 == V::Enchantment::CORRUPTED
                || __c0 == V::Enchantment::GLAM
                || __c0 == V::Enchantment::IMBUED
                || __c0 == V::Enchantment::GOOPY
                || __c0 == V::Enchantment::INKY
                || __c0 == V::Enchantment::INSTINCT
                || __c0 == V::Enchantment::MOMENTUM
                || __c0 == V::Enchantment::VIGOROUS
                || __c0 == V::Enchantment::NIMBLE
                || __c0 == V::Enchantment::PERFECT_FIT
                || __c0 == V::Enchantment::SHARP
                || __c0 == V::Enchantment::SLITHER
                || __c0 == V::Enchantment::SOULS_POWER
                || __c0 == V::Enchantment::SOWN
                || __c0 == V::Enchantment::SPIRAL
                || __c0 == V::Enchantment::SWIFT
                || __c0 == V::Enchantment::TEZCATARAS_EMBER
                || __c0 == V::Enchantment::ROYALLY_APPROVED
                || __c0 == V::Enchantment::STEADY) =>
        {
            Modify::Nothing
        }
        (enchantment, hook) => unimplemented!("what {enchantment:?} does for {hook:?}"),
    }
}

/// What an enchantment asks for when `hook` reaches the card `id` it is on.
/// The fight asks only an enchantment whose class overrides the hook.
pub(crate) fn event_hook<V: crate::Build>(
    enchanted: Enchanted<V>,
    id: CardId<V>,
    hook: EventHook<V>,
    asked: &mut HookEffects<V>,
) {
    match (enchanted.enchantment, hook) {
        // `CombatEnergyCosts.NextInt(4)` for its own card, which the fight
        // draws only while that card is in hand.
        (__c0, EventHook::CardDrawn { id: drawn, .. }) if __c0 == V::Enchantment::SLITHER => {
            if drawn == id {
                asked.push(HookEffect::RandomSelfCost { below: 4 });
            }
        }
        // `AfterCardPlayed` for its own card: spent once that play is over.
        (__c0, EventHook::CardPlayed { id: played, .. })
            if (__c0 == V::Enchantment::VIGOROUS || __c0 == V::Enchantment::GLAM) =>
        {
            if played == id {
                asked.push(HookEffect::DisableOwnEnchantment);
            }
        }
        // `AfterCardPlayed` for its own card: `Amount++`, on the copy and on
        // its deck version.
        (__c0, EventHook::CardPlayed { id: played, .. }) if __c0 == V::Enchantment::GOOPY => {
            if played == id {
                asked.push(HookEffect::RaiseOwnEnchantment {
                    by: 1,
                    write: crate::behaviour::DeckWrite::RaiseEnchantment(1),
                });
            }
        }
        // `AfterAutoPrePlayPhaseEntered` on its owner's first turn: its own
        // card auto-played, wherever it stands.
        (__c0, EventHook::AutoPrePlayPhase { turn }) if __c0 == V::Enchantment::IMBUED => {
            if turn <= 1 {
                asked.push(HookEffect::AutoPlay(id));
            }
        }
        // To the top of a shuffle, but not the one a fight opens on.
        (__c0, EventHook::ShuffleOrder { initial }) if __c0 == V::Enchantment::PERFECT_FIT => {
            if !initial {
                asked.push(HookEffect::ToTopOfDrawPile);
            }
        }
        (enchantment, hook) => unimplemented!("what {enchantment:?} does for {hook:?}"),
    }
}

/// `EnchantmentModel.OnPlay`, after each play of the card it is on.
pub(crate) fn on_play<V: crate::Build, F: Fight<V>>(
    enchanted: Enchanted<V>,
    state: &mut State,
    fight: &mut F,
) {
    let amount = i32::from(enchanted.amount);
    let status = &mut state.status;
    match enchanted.enchantment {
        // `ExtraDamage += Amount`.
        __c0 if __c0 == V::Enchantment::MOMENTUM => match &mut state.own {
            Own::Momentum { extra_damage } => {
                *extra_damage = extra_damage
                    .checked_add(i16::from(enchanted.amount))
                    .unwrap_or_else(|| unimplemented!("a Momentum past what a copy holds"));
            }
            Own::Nothing => unreachable!("a Momentum keeps its extra damage"),
        },
        __c0 if __c0 == V::Enchantment::SOWN => {
            if *status == EnchantmentStatus::Normal {
                fight.modify_energy(amount);
                *status = EnchantmentStatus::Disabled;
            }
        }
        __c0 if __c0 == V::Enchantment::SWIFT => {
            if *status == EnchantmentStatus::Normal {
                fight.draw(amount);
                *status = EnchantmentStatus::Disabled;
            }
        }
        // Two to its owner, unblockable and unpowered, on every play.
        __c0 if __c0 == V::Enchantment::CORRUPTED => {
            let owner = fight.actor();
            fight.damage_unblockable(owner, 2);
        }
        // Its Weak on the card's target, or on every enemy that can be hit
        // for a card aimed at all of them.
        __c0 if __c0 == V::Enchantment::INKY => {
            use sts2_core::{PowerAmount, TargetKinds as _};

            let weak = PowerAmount::Counted(INKY_WEAK);
            let every = fight.card_target_type(fight.playing_ident())
                == <V as sts2_core::ContentVersion>::TargetType::ALL_ENEMIES;
            let targets = match every {
                true => fight.hittable_opponents(),
                false => smallvec::smallvec![fight.target()],
            };
            for target in targets {
                fight.apply_power(target, V::Power::WEAK_POWER, weak);
            }
        }
        // Its block, which `RecalculateValues` sets to its amount, powered.
        __c0 if __c0 == V::Enchantment::ADROIT => {
            use sts2_core::{Amount, Effect, Scaling};

            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(amount),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
        }
        __c0 if __c0 == V::Enchantment::CLONE
            || __c0 == V::Enchantment::GLAM
            || __c0 == V::Enchantment::GOOPY
            || __c0 == V::Enchantment::IMBUED
            || __c0 == V::Enchantment::INSTINCT
            || __c0 == V::Enchantment::NIMBLE
            || __c0 == V::Enchantment::PERFECT_FIT
            || __c0 == V::Enchantment::SHARP
            || __c0 == V::Enchantment::SLITHER
            || __c0 == V::Enchantment::SOULS_POWER
            || __c0 == V::Enchantment::SPIRAL
            || __c0 == V::Enchantment::TEZCATARAS_EMBER
            || __c0 == V::Enchantment::VIGOROUS
            || __c0 == V::Enchantment::ROYALLY_APPROVED
            || __c0 == V::Enchantment::STEADY => {}
        enchantment => unimplemented!("what {enchantment:?} does when its card is played"),
    }
}
