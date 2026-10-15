//! What a run modifier does, as ported code. A modifier is asked only about
//! a hook its class overrides.

use sts2_core::{
    DecisionHook, EventHook, HookEffect, HookResponse, Modify, ModifyHook, PowerAmount,
    SideKinds as _,
};

use super::super::V0_107_1 as V;
use super::super::generated::encounters::RoomType;
use super::super::generated::modifiers::Modifier;
use super::super::generated::powers::Power;
use super::super::generated::relics::Relic;
use super::super::generated::restsite::RestSiteOption;
use super::super::generated::runstate::RunRngType;

/// `NightTerrors._maxHpLoss`.
const NIGHT_TERRORS_LOSS: i32 = 5;

/// What `DeadlyEvents.AfterRunCreated` sets an unknown room's elite odds to,
/// and their base.
const DEADLY_ELITE_ODDS: sts2_core::Fixed = sts2_core::Fixed::new(1, 1);

/// `BigGameHunter`'s elites, against the map's own.
const HUNTER_ELITES: sts2_core::Fixed = sts2_core::Fixed::new(25, 1);

/// `Murderous._strengthAmount`.
const MURDEROUS_STRENGTH: PowerAmount<i32> = PowerAmount::Counted(3);

/// What `Terminal` takes off the maximum in each room.
const TERMINAL_LOSS: i32 = 1;

/// `Terminal`'s Plating, for each fight.
const TERMINAL_PLATING: PowerAmount<i32> = PowerAmount::Counted(5);

/// `DeadlyEvents.ModifyOddsIncreaseForUnrolledRoomType`, on a treasure's.
const DEADLY_TREASURE_RISE: sts2_core::Fixed = sts2_core::Fixed::new(2, 0);

/// What `modifier` decides for `hook`.
///
/// # Panics
///
/// A modifier whose override of `hook` is not ported.
pub(crate) fn decide_hook(modifier: Modifier, hook: DecisionHook<V>) -> HookResponse<bool> {
    match (modifier, hook) {
        // `Flight.ShouldAllowFreeTravel`.
        (Modifier::Flight, DecisionHook::ShouldAllowFreeTravel) => HookResponse::Decision(true),
        (modifier, hook) => unimplemented!("what {modifier:?} decides for {hook:?}"),
    }
}

/// What `modifier` makes of the number `hook` asks about.
///
/// # Panics
///
/// A modifier whose override of `hook` is not ported.
pub(crate) fn modify_hook(modifier: Modifier, hook: ModifyHook<V>) -> Modify<i32> {
    match (modifier, hook) {
        // `NightTerrors.ModifyRestSiteHealAmount`: the whole of the maximum.
        (Modifier::NightTerrors, ModifyHook::RestSiteHealAmount { max_hp }) => {
            Modify::Replaced(max_hp)
        }
        // `DeadlyEvents.ModifyOddsIncreaseForUnrolledRoomType`: a treasure's
        // rise doubled.
        (Modifier::DeadlyEvents, ModifyHook::OddsIncreaseForUnrolledRoomType { room }) => {
            match room {
                RoomType::Treasure => Modify::Scaled(DEADLY_TREASURE_RISE),
                _ => Modify::Nothing,
            }
        }
        (modifier, hook) => unimplemented!("what {modifier:?} does for {hook:?}"),
    }
}

/// What `modifier` asks for once `hook` is raised.
///
/// # Panics
///
/// A modifier whose override of `hook` is not ported.
pub(crate) fn event_hook(
    modifier: Modifier,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (modifier, hook) {
        // `DeadlyEvents.AfterRunCreated`: no Juzu Bracelet in either bag, and
        // an unknown room can be an elite.
        (Modifier::DeadlyEvents, EventHook::RunCreated) => asked.extend([
            HookEffect::RemoveFromRelicBags(Relic::JuzuBracelet),
            HookEffect::SetUnknownRoomOdds {
                room: RoomType::Elite,
                odds: DEADLY_ELITE_ODDS,
                base: DEADLY_ELITE_ODDS,
            },
        ]),
        // `CursedRun.AfterActEntered`: a curse a modifier may give, by
        // `NextItem` off `RunState.Rng.Niche`, into the deck.
        (Modifier::CursedRun, EventHook::ActEntered) => {
            asked.push(HookEffect::AddDistinctToDeck {
                cards: &super::relics::MODIFIER_CURSES,
                count: 1,
                stream: RunRngType::Niche,
            });
        }
        // `Murderous.AfterRoomEntered`: three Strength on every creature in
        // a combat room, the player too.
        (Modifier::Murderous, EventHook::RoomEntered { room, .. }) => {
            if is_combat(room) {
                asked.push(HookEffect::ApplyToEveryCreature(
                    Power::StrengthPower,
                    MURDEROUS_STRENGTH,
                ));
            }
        }
        // `Murderous.AfterCreatureAddedToCombat`: three Strength on a
        // creature that joins anywhere but the player's side.
        (Modifier::Murderous, EventHook::CreatureAdded { creature, side, .. }) => {
            if side != <V as sts2_core::ContentVersion>::CombatSide::PLAYER {
                asked.push(HookEffect::ApplyPower {
                    target: creature,
                    power: Power::StrengthPower,
                    amount: MURDEROUS_STRENGTH,
                    applier: None,
                });
            }
        }
        // `Terminal.AfterRoomEntered`: one off the maximum in each room the
        // map opens, while there is more than one to lose. Then five Plating
        // in a combat room.
        (
            Modifier::Terminal,
            EventHook::RoomEntered {
                room, player, base, ..
            },
        ) => {
            if base && (player.hp > 1 || player.max_hp > 1) {
                asked.push(HookEffect::LoseMaxHp(TERMINAL_LOSS));
            }
            if is_combat(room) {
                asked.push(HookEffect::ApplyToPlayers(
                    Power::PlatingPower,
                    TERMINAL_PLATING,
                ));
            }
        }
        // `OnRunCreated` alone, which clears no deck for these.
        (_, EventHook::RunCreated) => {}
        // `BigGameHunter.ModifyGeneratedMap`: the map again, with two and a
        // half times its elites.
        (Modifier::BigGameHunter, EventHook::GeneratedMap { .. }) => {
            asked.push(HookEffect::ScaledElitesMap(HUNTER_ELITES));
        }
        // `BigGameHunter.ModifyCardRewardCreationOptions`: a won elite's
        // reward offers rares alone, at even odds. Its options carry neither
        // flag that would stop it.
        (
            Modifier::BigGameHunter,
            EventHook::CardRewardCreationOptions {
                elite_encounter: true,
            },
        ) => asked.push(HookEffect::RaresAtEvenOdds),
        (Modifier::BigGameHunter, EventHook::CardRewardCreationOptions { .. }) => {}
        // `Vintage.TryModifyRewardsLate`: a relic for each card reward, in a
        // monster's room.
        (Modifier::Vintage, EventHook::ModifyRewardsLate { room }) => {
            if room == Some(RoomType::Monster) {
                asked.push(HookEffect::RelicRewardsForCardRewards);
            }
        }
        // `Midas.TryModifyRewardsLate`: every gold reward at twice its amount.
        (Modifier::Midas, EventHook::ModifyRewardsLate { .. }) => {
            asked.push(HookEffect::DoubleGoldRewards);
        }
        // `Midas.TryModifyRestSiteOptions`: no smithing.
        (Modifier::Midas, EventHook::RestSiteOptions) => {
            asked.push(HookEffect::WithdrawRestOption(RestSiteOption::Smith));
        }
        // `NightTerrors.AfterRestSiteHeal`: five off the maximum, which
        // keeps one, after a rest site's own heal.
        (Modifier::NightTerrors, EventHook::RestSiteHeal { mimicked, max_hp }) => {
            if !mimicked {
                asked.push(HookEffect::LoseMaxHp(NIGHT_TERRORS_LOSS.min(max_hp - 1)));
            }
        }
        (modifier, hook) => unimplemented!("what {modifier:?} does for {hook:?}"),
    }
}

/// `room is CombatRoom`.
fn is_combat(room: RoomType) -> bool {
    matches!(room, RoomType::Monster | RoomType::Elite | RoomType::Boss)
}

/// Whether every rule of `modifier` is ported.
pub(crate) fn ported(modifier: Modifier) -> bool {
    matches!(
        modifier,
        Modifier::Flight
            | Modifier::Vintage
            | Modifier::Midas
            | Modifier::NightTerrors
            | Modifier::DeadlyEvents
            | Modifier::BigGameHunter
            | Modifier::CursedRun
            | Modifier::Murderous
            | Modifier::Terminal
    )
}
