//! The relics whose rules this build ports for itself, which the shared
//! relics hand over through [`crate::build::OwnRules`].

use sts2_core::{EventHook, HookEffect};

use super::super::V0_111_0 as V;
use super::super::generated::cards::{Cards, CurseCard, QuestCard};
use super::super::generated::potions::Potion;
use super::super::generated::relics::Relic;
use crate::behaviour::relics::State;

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
        // `DiamondDiadem` blocks and blurs on its owner's first turn here.
        (State::DiamondDiadem(_), _) => unimplemented!("Diamond Diadem in v0.111.0"),
        (state @ State::Own(_), hook) => unimplemented!("what {state:?} does for {hook:?}"),
        (state, _) => unreachable!("{state:?} is a shared relic"),
    }
}
