//! Which cards an enchantment takes, and what it does on them in a fight.

use sts2_core::{
    CardId, CardIdent, CardKind, CardTable as _, Change, Cost, EnchantHook, Enchanted,
    EnchantmentStatus, EventHook, Fight, HookEffect, HookEffects, Modify, Rules as _,
};

use super::super::V0_107_1 as V;
use super::super::generated::cards::{CardKeyword, CardRarity, CardTag};
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
        // Its own check adds an X cost to the base's refusals.
        Enchantment::Slither => base && card.card.cost() != Cost::X,
        Enchantment::Glam
        | Enchantment::Sown
        | Enchantment::Swift
        | Enchantment::Adroit
        | Enchantment::Inky
        | Enchantment::TezcatarasEmber
        | Enchantment::Clone => base,
        // Exhaust among the card's own keywords, which its upgrade can take
        // off.
        Enchantment::SoulsPower => base && card_exhausts(card),
        // `CanEnchantCardType`, which takes an attack alone.
        Enchantment::Corrupted
        | Enchantment::Instinct
        | Enchantment::Sharp
        | Enchantment::Vigorous
        | Enchantment::Momentum => base && V::card_kind(card.card) == Some(CardKind::Attack),
        Enchantment::Nimble => base && card.card.gains_block(),
        // `CanEnchantCardType`, which takes a skill alone.
        Enchantment::Imbued => base && V::card_kind(card.card) == Some(CardKind::Skill),
        // `CanEnchantCardType`, which takes an attack or a skill.
        // `Steady` overrides no check, so the base answers.
        Enchantment::Steady => base,
        Enchantment::RoyallyApproved => {
            base && matches!(
                V::card_kind(card.card),
                Some(CardKind::Attack | CardKind::Skill)
            )
        }
        Enchantment::Goopy => base && card.card.tagged(CardTag::Defend),
        _ => unimplemented!("which cards {enchantment:?} can enchant"),
    }
}

/// `GetKeywordsWithSources(Local).Contains(Exhaust)`: what the card prints,
/// then what its upgrade writes where it has one.
fn card_exhausts(card: &CardIdent<V>) -> bool {
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
pub(crate) fn enchanted_changes(enchantment: Enchantment) -> &'static [Change<V>] {
    match enchantment {
        Enchantment::SoulsPower => &[Change::RemoveKeyword(CardKeyword::Exhaust)],
        // `EnergyCost.UpgradeBy(-cost)` takes the cost to nothing, which
        // later modifiers write over, then `AddKeyword(Eternal)`.
        Enchantment::TezcatarasEmber => &[
            Change::SetCost(sts2_core::CostPool::Energy, 0),
            Change::AddKeyword(CardKeyword::Eternal),
        ],
        Enchantment::Goopy => &[Change::AddKeyword(CardKeyword::Exhaust)],
        Enchantment::RoyallyApproved => &[
            Change::AddKeyword(CardKeyword::Innate),
            Change::AddKeyword(CardKeyword::Retain),
        ],
        Enchantment::Steady => &[Change::AddKeyword(CardKeyword::Retain)],
        _ => &[],
    }
}

/// Whether `card` carries `Eternal`: its table prints it, or its
/// enchantment's `OnEnchant` adds it.
pub(crate) fn card_eternal(card: &CardIdent<V>) -> bool {
    use sts2_core::CardTable as _;

    card.card.eternal()
        || card.enchantment.is_some_and(|enchanted| {
            enchanted_changes(enchanted.enchantment)
                .contains(&Change::AddKeyword(CardKeyword::Eternal))
        })
}

/// `CardCmd.Enchant` by `amount`, and the enchantment's `OnEnchant`.
///
/// # Panics
///
/// An enchantment whose `OnEnchant` changes the card, which this port has
/// not reached.
pub(crate) fn enchant(enchantment: Enchantment, amount: u8, card: &mut CardIdent<V>) {
    match enchantment {
        Enchantment::Adroit
        | Enchantment::Momentum
        | Enchantment::Corrupted
        | Enchantment::Glam
        | Enchantment::Inky
        | Enchantment::Instinct
        | Enchantment::Vigorous
        | Enchantment::Nimble
        | Enchantment::PerfectFit
        | Enchantment::Sharp
        | Enchantment::Slither
        | Enchantment::SoulsPower
        | Enchantment::Sown
        | Enchantment::Spiral
        | Enchantment::Swift
        | Enchantment::TezcatarasEmber
        | Enchantment::Goopy
        | Enchantment::Clone
        | Enchantment::Imbued
        | Enchantment::RoyallyApproved
        | Enchantment::Steady => {
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
pub(crate) fn should_start_at_bottom_of_draw_pile(
    enchantment: Enchantment,
) -> sts2_core::HookResponse<bool> {
    use sts2_core::HookResponse;

    match enchantment {
        Enchantment::Imbued => HookResponse::Decision(true),
        Enchantment::Adroit
        | Enchantment::Clone
        | Enchantment::Corrupted
        | Enchantment::Glam
        | Enchantment::Goopy
        | Enchantment::Inky
        | Enchantment::Instinct
        | Enchantment::Momentum
        | Enchantment::Nimble
        | Enchantment::PerfectFit
        | Enchantment::Sharp
        | Enchantment::Slither
        | Enchantment::SoulsPower
        | Enchantment::Sown
        | Enchantment::Spiral
        | Enchantment::Swift
        | Enchantment::TezcatarasEmber
        | Enchantment::Vigorous
        | Enchantment::RoyallyApproved
        | Enchantment::Steady => HookResponse::DoesNotImplement,
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

impl sts2_core::EnchantmentModel<V> for State {
    fn new(enchantment: Option<Enchantment>) -> Self {
        let own = match enchantment {
            Some(Enchantment::Momentum) => Own::Momentum { extra_damage: 0 },
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
pub(crate) fn modify(enchanted: Enchanted<V>, state: &State, hook: EnchantHook) -> Modify<i32> {
    let amount = i32::from(enchanted.amount);
    let status = state.status;
    match (enchanted.enchantment, hook) {
        // `Momentum`: what its plays have added, on a powered hit.
        (Enchantment::Momentum, EnchantHook::DamageAdditive { powered: true }) => match state.own {
            Own::Momentum { extra_damage } => Modify::Added(i32::from(extra_damage)),
            Own::Nothing => unreachable!("a Momentum keeps its extra damage"),
        },
        // Sharp's own check: a powered attack, and nothing for any other hit.
        (Enchantment::Sharp, EnchantHook::DamageAdditive { powered: true }) => {
            Modify::Added(amount)
        }
        (Enchantment::Corrupted, EnchantHook::DamageMultiplicative { powered: true }) => {
            Modify::Scaled(sts2_core::Fixed::new(15, 1))
        }
        // `Instinct`: double on a powered hit.
        (Enchantment::Instinct, EnchantHook::DamageMultiplicative { powered: true }) => {
            Modify::Scaled(sts2_core::Fixed::new(2, 0))
        }
        // `Inky`: its `DamageVar(1)` on a powered hit, whatever its amount.
        (Enchantment::Inky, EnchantHook::DamageAdditive { powered: true }) => {
            Modify::Added(INKY_DAMAGE)
        }
        (Enchantment::Nimble, EnchantHook::BlockAdditive) => Modify::Added(amount),
        // `Goopy`: one less than its amount, which each play raises.
        (Enchantment::Goopy, EnchantHook::BlockAdditive) => Modify::Added(amount - 1),
        // Its amount on a powered hit, until its card is first played.
        (Enchantment::Vigorous, EnchantHook::DamageAdditive { powered: true })
            if status == EnchantmentStatus::Normal =>
        {
            Modify::Added(amount)
        }
        // Its `Times`, which it prints as one.
        (Enchantment::Spiral, EnchantHook::PlayCount) => Modify::Added(1),
        // `TezcatarasEmber`: its `DamageVar(3)` on a powered hit.
        (Enchantment::TezcatarasEmber, EnchantHook::DamageAdditive { powered: true }) => {
            Modify::Added(EMBER_DAMAGE)
        }
        // Its `Times`, one, until its card is first played in the fight.
        (Enchantment::Glam, EnchantHook::PlayCount) if status == EnchantmentStatus::Normal => {
            Modify::Added(1)
        }
        (
            Enchantment::Adroit
            | Enchantment::Clone
            | Enchantment::Corrupted
            | Enchantment::Glam
            | Enchantment::Imbued
            | Enchantment::Goopy
            | Enchantment::Inky
            | Enchantment::Instinct
            | Enchantment::Momentum
            | Enchantment::Vigorous
            | Enchantment::Nimble
            | Enchantment::PerfectFit
            | Enchantment::Sharp
            | Enchantment::Slither
            | Enchantment::SoulsPower
            | Enchantment::Sown
            | Enchantment::Spiral
            | Enchantment::Swift
            | Enchantment::TezcatarasEmber
            | Enchantment::RoyallyApproved
            | Enchantment::Steady,
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
        (Enchantment::Vigorous | Enchantment::Glam, EventHook::CardPlayed { id: played, .. }) => {
            if played == id {
                asked.push(HookEffect::DisableOwnEnchantment);
            }
        }
        // `AfterCardPlayed` for its own card: `Amount++`, on the copy and on
        // its deck version.
        (Enchantment::Goopy, EventHook::CardPlayed { id: played, .. }) => {
            if played == id {
                asked.push(HookEffect::RaiseOwnEnchantment {
                    by: 1,
                    write: super::cards::DeckWrite::RaiseEnchantment(1),
                });
            }
        }
        // `AfterAutoPrePlayPhaseEntered` on its owner's first turn: its own
        // card auto-played, wherever it stands.
        (Enchantment::Imbued, EventHook::AutoPrePlayPhase { turn }) => {
            if turn <= 1 {
                asked.push(HookEffect::AutoPlay(id));
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
pub(crate) fn on_play<F: Fight<V>>(enchanted: Enchanted<V>, state: &mut State, fight: &mut F) {
    let amount = i32::from(enchanted.amount);
    let status = &mut state.status;
    match enchanted.enchantment {
        // `ExtraDamage += Amount`.
        Enchantment::Momentum => match &mut state.own {
            Own::Momentum { extra_damage } => {
                *extra_damage = extra_damage
                    .checked_add(i16::from(enchanted.amount))
                    .unwrap_or_else(|| unimplemented!("a Momentum past what a copy holds"));
            }
            Own::Nothing => unreachable!("a Momentum keeps its extra damage"),
        },
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
        // Its Weak on the card's target, or on every enemy that can be hit
        // for a card aimed at all of them.
        Enchantment::Inky => {
            use super::super::generated::powers::Power;
            use sts2_core::{PowerAmount, TargetKinds as _};

            let weak = PowerAmount::Counted(INKY_WEAK);
            let every = fight.card_target_type(fight.playing_ident())
                == <V as sts2_core::ContentVersion>::TargetType::ALL_ENEMIES;
            let targets = match every {
                true => fight.hittable_opponents(),
                false => smallvec::smallvec![fight.target()],
            };
            for target in targets {
                fight.apply_power(target, Power::WeakPower, weak);
            }
        }
        // Its block, which `RecalculateValues` sets to its amount, powered.
        Enchantment::Adroit => {
            use sts2_core::{Amount, Effect, Scaling};

            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(amount),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
        }
        Enchantment::Clone
        | Enchantment::Glam
        | Enchantment::Goopy
        | Enchantment::Imbued
        | Enchantment::Instinct
        | Enchantment::Nimble
        | Enchantment::PerfectFit
        | Enchantment::Sharp
        | Enchantment::Slither
        | Enchantment::SoulsPower
        | Enchantment::Spiral
        | Enchantment::TezcatarasEmber
        | Enchantment::Vigorous
        | Enchantment::RoyallyApproved
        | Enchantment::Steady => {}
        enchantment => unimplemented!("what {enchantment:?} does when its card is played"),
    }
}
