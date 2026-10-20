//! The relics whose rules this build ports for itself, which the shared
//! relics hand over through [`crate::build::OwnRules`].

use sts2_core::{EventHook, HookEffect, PowerAmount};

use super::super::V0_111_0 as V;
use super::super::generated::cards::{Cards, CurseCard, QuestCard};
use super::super::generated::potions::Potion;
use super::super::generated::powers::Power;
use super::super::generated::relics::Relic;
use crate::behaviour::relics::State;
use crate::generated::runstate::CombatSide;

/// `DiamondDiadem`'s `BlockVar(20m, ValueProp.Unpowered)`, which v0.107.1's
/// Diadem lacks.
const DIAMOND_DIADEM_BLOCK: i32 = 20;

/// A relic of this build's own, as the run carries it. Neither saves a
/// field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnRelic {
    DowsingRod,
    NeowsSacrifice,
}

/// The state a relic of this build's own starts with.
pub(crate) fn own_relic_state(relic: Relic) -> Option<OwnRelic> {
    match relic {
        Relic::DowsingRod => Some(OwnRelic::DowsingRod),
        Relic::NeowsSacrifice => Some(OwnRelic::NeowsSacrifice),
        _ => None,
    }
}

/// `EventHook` for a relic of this build's own.
pub(crate) fn own_relic_event_hook(
    state: &mut State<V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (state, hook) {
        // `DowsingRod.AfterObtained`: a Dowsing into the deck.
        (State::Own(OwnRelic::DowsingRod), EventHook::Obtained { .. }) => {
            asked.push(HookEffect::AddToDeck(Cards::Quest(QuestCard::Dowsing)));
        }
        // `NeowsSacrifice.AfterObtained`: `PotionCmd.TryToProcure` of an
        // Ambergris, then a Guilty into the deck.
        (State::Own(OwnRelic::NeowsSacrifice), EventHook::Obtained { .. }) => {
            asked.push(HookEffect::ProcurePotion(Potion::Ambergris));
            asked.push(HookEffect::AddToDeck(Cards::Curse(CurseCard::Guilty)));
        }
        // `DiamondDiadem.AfterSideTurnStart`: on its owner's first turn, its
        // block, unpowered, then one Blur on its owner.
        (State::DiamondDiadem(_), EventHook::SideTurnStart { side, turn, .. }) => {
            if side == CombatSide::Player && turn <= 1 {
                asked.extend([
                    HookEffect::GainBlock(DIAMOND_DIADEM_BLOCK),
                    HookEffect::ApplyToOwner(Power::BlurPower, PowerAmount::Counted(1)),
                ]);
            }
        }
        (State::DiamondDiadem(_), hook) => unimplemented!("what DiamondDiadem does for {hook:?}"),
        (state @ State::Own(_), hook) => unimplemented!("what {state:?} does for {hook:?}"),
        (state, _) => unreachable!("{state:?} is a shared relic"),
    }
}

/// Whether a shared relic's rule is this build's own for some hook.
pub(crate) fn own_relic(relic: Relic) -> bool {
    matches!(
        relic,
        Relic::ToyBox
            | Relic::DistinguishedCape
            | Relic::SereTalon
            | Relic::BeautifulBracelet
            | Relic::HandDrill
    )
}

/// A shared relic's `EventHook`, for a hook this build answers itself.
pub(crate) fn own_shared_relic_event_hook(
    state: &mut State<V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) -> Option<()> {
    use crate::build::Lists as _;
    use crate::ids::CardIds as _;
    use crate::vars::RelicVars as _;

    match (state, hook) {
        // `ToyBox.AfterCombatEnd` returns at once once `IsUsedUp`, where
        // v0.107.1's counts on.
        (State::ToyBox(state), EventHook::CombatEnd { .. })
            if crate::behaviour::relics::toy_box::used_up::<V>(state) =>
        {
            Some(())
        }
        // `BeautifulBracelet.AfterObtained` takes its cards at random off
        // `Niche`, of those Swift can enchant, where v0.107.1's asked.
        (State::BeautifulBracelet, EventHook::Obtained { .. }) => {
            use crate::ids::EnchantmentIds as _;

            asked.push(HookEffect::EnchantRandomDeckCards {
                enchantment: <V as sts2_core::ContentVersion>::Enchantment::SWIFT,
                amount: u8::try_from(Relic::BEAUTIFUL_BRACELET_SWIFT)
                    .expect("Swift's amount fits a byte"),
                count: Relic::BEAUTIFUL_BRACELET_CARDS as usize,
                stream: sts2_core::Stream::Run(crate::generated::runstate::RunRngType::Niche),
            });
            Some(())
        }
        // `HandDrill` gives its Vulnerable on `AfterBlockBroken`, where
        // v0.107.1's read `WasBlockBroken` off `AfterDamageGiven`.
        (State::HandDrill, EventHook::DamageGiven { .. }) => Some(()),
        (
            State::HandDrill,
            EventHook::BlockBroken {
                creature,
                breaker,
                pet_of,
                player,
            },
        ) => {
            use crate::ids::PowerIds as _;

            if (breaker == Some(player) || pet_of == Some(player)) && creature != player {
                asked.push(HookEffect::ApplyPower {
                    target: creature,
                    power: <V as sts2_core::ContentVersion>::Power::VULNERABLE_POWER,
                    amount: PowerAmount::Counted(Relic::HAND_DRILL_VULNERABLE_POWER),
                    applier: Some(player),
                });
            }
            Some(())
        }
        // `DistinguishedCape` and `SereTalon` traded what they cost. The
        // Cape adds two curses, each `NextItem` off `Niche` over those not
        // drawn already, then its Apparitions.
        (State::DistinguishedCape, EventHook::Obtained { .. }) => {
            asked.push(HookEffect::AddDistinctToDeck {
                cards: V::RELICS_MODIFIER_CURSES,
                count: DISTINGUISHED_CAPE_CURSES,
                stream: crate::generated::runstate::RunRngType::Niche,
            });
            for _ in 0..(Relic::DISTINGUISHED_CAPE_CARDS as usize) {
                asked.push(HookEffect::AddToDeck(
                    <V as sts2_core::ContentVersion>::Card::APPARITION,
                ));
            }
            Some(())
        }
        // The Talon takes nine maximum health, then its Wishes.
        (State::SereTalon, EventHook::Obtained { .. }) => {
            asked.push(HookEffect::LoseMaxHp(SERE_TALON_HP_LOSS));
            for _ in 0..(Relic::SERE_TALON_WISHES as usize) {
                asked.push(HookEffect::AddToDeck(
                    <V as sts2_core::ContentVersion>::Card::WISH,
                ));
            }
            Some(())
        }
        _ => None,
    }
}

/// `DistinguishedCape`'s `DynamicVar("Curses", 2m)`.
const DISTINGUISHED_CAPE_CURSES: usize = 2;

/// `SereTalon`'s `HpLossVar(9m)`.
const SERE_TALON_HP_LOSS: i32 = 9;
