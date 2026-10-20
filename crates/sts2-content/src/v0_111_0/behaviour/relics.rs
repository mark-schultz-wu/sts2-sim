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
    matches!(relic, Relic::ToyBox)
}

/// A shared relic's `EventHook`, for a hook this build answers itself.
///
/// `ToyBox.AfterCombatEnd` returns at once here once `IsUsedUp`, where
/// v0.107.1's counts on. Every other hook it answers as the shared port does.
pub(crate) fn own_shared_relic_event_hook(
    state: &mut State<V>,
    hook: EventHook<V>,
    _asked: &mut sts2_core::HookEffects<V>,
) -> Option<()> {
    match (state, hook) {
        (State::ToyBox(state), EventHook::CombatEnd { .. })
            if crate::behaviour::relics::toy_box::used_up::<V>(state) =>
        {
            Some(())
        }
        _ => None,
    }
}
