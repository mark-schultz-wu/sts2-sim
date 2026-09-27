//! The colorless cards the generator leaves to a hand port.

use sts2_core::{EventHook, Fight, HookEffect, Pile, Played};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardVar, ColorlessCard};
use super::V;

pub(super) fn on_play<F: Fight<V>>(card: ColorlessCard, fight: &mut F) -> Played<V> {
    match card {
        ColorlessCard::Restlessness => restlessness(fight),
        card => card_plays::colorless::on_play(card, fight),
    }
}

/// `Restlessness`: with no other card in hand, its `Cards` drawn one at a
/// time, then its `Energy`. It stands in the play pile as it runs.
fn restlessness<F: Fight<V>>(fight: &mut F) -> Played<V> {
    if fight.cards_in(Pile::Hand).is_empty() {
        for _ in 0..fight.var(CardVar::Cards) {
            fight.draw(1);
        }
        fight.modify_energy(fight.var(CardVar::Energy));
    }
    Played::Done
}

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
