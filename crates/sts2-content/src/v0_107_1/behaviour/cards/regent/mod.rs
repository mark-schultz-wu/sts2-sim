//! The Regent's cards the generator leaves to a hand port, filed by rarity.

use sts2_core::{Fight, Played};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::RegentCard;
use super::V;

mod common;
mod uncommon;

/// Where a Regent card stopped inside its own play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    /// `Begone`, after it selected the card to turn into a Minion Strike.
    BegoneTransform,
}

pub(super) fn on_play<F: Fight<V>>(card: RegentCard, fight: &mut F) -> Played<V> {
    match card {
        RegentCard::Glitterstream => common::glitterstream(fight),
        RegentCard::Begone => common::begone(),
        RegentCard::CollisionCourse => common::collision_course(fight),
        card => card_plays::regent::on_play(card, fight),
    }
}

/// Deliver a notification to one Regent card in the fight.
pub(super) fn event_hook<P: sts2_core::Printed<V> + ?Sized>(
    card: RegentCard,
    id: sts2_core::CardId<V>,
    printed: &P,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match card {
        RegentCard::KinglyKick => uncommon::kingly_kick(id, hook, asked),
        RegentCard::KinglyPunch => uncommon::kingly_punch(id, printed, hook, asked),
        card => unimplemented!("what {card:?} does for {hook:?}"),
    }
}

/// The rest of a play that stopped, once the fight has its answer.
pub(super) fn resume_play<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        Resume::BegoneTransform => common::begone_transform(fight, chosen),
    }
}
