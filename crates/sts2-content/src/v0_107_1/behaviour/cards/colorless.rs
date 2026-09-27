//! The colorless cards the generator leaves to a hand port.

use sts2_core::{EventHook, HookEffect};

use super::super::super::generated::cards::ColorlessCard;
use super::V;

/// Deliver a notification to one colorless card in the fight.
pub(super) fn event_hook(
    card: ColorlessCard,
    id: sts2_core::CardId<V>,
    history: sts2_core::History<'_, V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (card, hook) {
        // `Bolas.BeforeHandDraw` and `ThrummingHatchet.BeforeHandDraw`: back
        // into hand, from wherever it is, once this copy finished a play in
        // its owner's last turn. A fight here has one player.
        (
            ColorlessCard::Bolas | ColorlessCard::ThrummingHatchet,
            EventHook::BeforeHandDraw { turn, .. },
        ) => {
            if history
                .finished_last_player_turn(turn)
                .any(|finished| finished.id == id)
            {
                asked.push(HookEffect::ToHand);
            }
        }
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
}
