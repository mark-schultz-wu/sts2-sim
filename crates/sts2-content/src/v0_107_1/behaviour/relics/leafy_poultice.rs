//! `LeafyPoultice`: less maximum health, and the first Basic Strike and Defend
//! transformed, as it is picked up.

use sts2_core::{DeckAct, DeckPick, EventHook, HookEffect, Stream};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::CardTag;
use super::super::super::generated::runstate::PlayerRngType;
use super::Number;

/// `MaxHpVar(12)`.
const MAX_HP: Number = 12;

/// `FirstOrDefault` over the deck's Basic cards, for each tag in turn.
const PICKS: &[DeckPick<V>] = &[
    DeckPick::FirstBasic(CardTag::Strike),
    DeckPick::FirstBasic(CardTag::Defend),
];

/// `AfterObtained`: `CreatureCmd.LoseMaxHp` on its owner, then
/// `CardCmd.Transform` on the cards it names, off
/// `PlayerRng.Transformations`.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { .. } = hook {
        asked.extend([
            HookEffect::LoseMaxHp(MAX_HP),
            HookEffect::OnDeckCards {
                act: DeckAct::Transform(Stream::Player(PlayerRngType::Transformations)),
                picks: PICKS,
            },
        ]);
    }
}
