//! `RainbowRing`: Strength and Dexterity once a turn, for an attack, a skill
//! and a power played in it.

use sts2_core::{CardKind, EventHook, HookEffect, PowerAmount};

use crate::behaviour::relics::Number;
use crate::generated::runstate::CombatSide;
use crate::ids::PowerIds as _;

/// `PowerVar<StrengthPower>(1)`.
const STRENGTH: PowerAmount<Number> = PowerAmount::Counted(1);

/// `PowerVar<DexterityPower>(1)`.
const DEXTERITY: PowerAmount<Number> = PowerAmount::Counted(1);

/// Whether an attack, a skill and a power have been played this turn, and
/// `ActivationCountThisTurn`, which is at most one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    attack: bool,
    skill: bool,
    power: bool,
    activated: bool,
}

/// `AfterCardPlayed` notes the kind of each card its owner plays, until the
/// turn has all three, which gives its Strength and then its Dexterity.
/// `BeforeSideTurnStart` on its owner's side and `AfterCombatEnd` start it
/// again.
pub(super) fn event_hook<V: crate::Build>(
    state: &mut State,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match hook {
        EventHook::CardPlayed { card, .. } if !state.activated => {
            match <V as sts2_core::Rules>::card_kind(card) {
                Some(CardKind::Attack) => state.attack = true,
                Some(CardKind::Skill) => state.skill = true,
                Some(CardKind::Power) => state.power = true,
                _ => {}
            }
            if state.attack && state.skill && state.power {
                asked.extend([
                    HookEffect::ApplyToOwner(V::Power::STRENGTH_POWER, STRENGTH),
                    HookEffect::ApplyToOwner(V::Power::DEXTERITY_POWER, DEXTERITY),
                ]);
                state.activated = true;
            }
        }
        EventHook::BeforeSideTurnStart {
            side: CombatSide::Player,
            ..
        }
        | EventHook::CombatEnd { .. } => *state = State::default(),
        _ => {}
    }
}
