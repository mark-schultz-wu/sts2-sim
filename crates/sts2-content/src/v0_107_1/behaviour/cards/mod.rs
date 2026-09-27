//! `CardModel.OnPlay`, and the rest of what a card does, ported.
//!
//! `generated/card_plays/` holds every play and upgrade whose statements the
//! generator matched. What is here is the rest: the plays it left to a hand
//! port, and every hook a card overrides.
//!
//! One module per pool, filed the way `generated/cards/` is, so a card's
//! behaviour sits under the same name as its row. A pool gets a module when
//! its first card is ported by hand.
//!
//! [`on_play`] panics for a card neither the generator nor a hand port
//! covers. A card outside both stops the run where it stands rather than
//! playing as if it did nothing.

use sts2_core::{CardVars, Fight, Played};

use super::super::V0_107_1;
use super::super::generated::card_plays;
use super::super::generated::cards::{CardVar, Cards, NecrobinderCard};

/// What the run holds a card's numbers as.
type Number = <V0_107_1 as sts2_core::ContentVersion>::Number;

mod defect;
mod necrobinder;
mod regent;
mod silent;
mod status;
mod token;

type V = V0_107_1;

/// What a card's first upgrade writes on the copy being upgraded.
///
/// `CardModel.OnUpgrade`, which each card overrides to raise the numbers it
/// prints. Every one the generator matched is generated, and nothing on this
/// build is ported by hand.
///
/// Declared rather than applied: an upgrade writes in the same vocabulary any
/// other effect does, and the fight owns the copy it writes on.
pub(crate) fn upgrade(card: Cards) -> &'static [Change] {
    match card {
        Cards::Ironclad(card) => card_plays::ironclad::upgrade(card),
        Cards::Silent(card) => card_plays::silent::upgrade(card),
        Cards::Regent(card) => card_plays::regent::upgrade(card),
        Cards::Necrobinder(card) => card_plays::necrobinder::upgrade(card),
        Cards::Defect(card) => card_plays::defect::upgrade(card),
        Cards::Colorless(card) => card_plays::colorless::upgrade(card),
        Cards::Curse(card) => card_plays::curse::upgrade(card),
        Cards::Event(card) => card_plays::event::upgrade(card),
        Cards::Quest(card) => card_plays::quest::upgrade(card),
        Cards::Status(card) => card_plays::status::upgrade(card),
        Cards::Token(card) => card_plays::token::upgrade(card),
        Cards::Deprecated(card) => card_plays::deprecated::upgrade(card),
    }
}

/// One thing an effect writes on a copy of a card, as this build spells it.
pub(crate) type Change = sts2_core::Change<V>;

/// What a card that works its own target out from the fight names.
pub(crate) fn computed_target(
    card: Cards,
    powers: &[sts2_core::Stack<V0_107_1>],
) -> super::super::generated::cards::TargetType {
    match card {
        Cards::Token(card) => token::computed_target(card, powers),
        card => unimplemented!("what {card:?} points at"),
    }
}

/// What a card works out from the fight for a number it prints.
pub(crate) fn calculated<P: sts2_core::Printed<V> + ?Sized>(
    card: Cards,
    key: CardVar,
    printed: &P,
    owner: sts2_core::CardOwner<'_, V0_107_1>,
) -> Number {
    match (card, key) {
        (Cards::Token(card), key) => token::calculated(card, key, printed, owner.powers),
        // `Unleash`: a base, and one more for each point of health a living
        // Osty has.
        (Cards::Necrobinder(NecrobinderCard::Unleash), CardVar::CalculatedDamage) => {
            printed.face(CardVar::CalculationBase)
                + printed.face(CardVar::ExtraDamage) * owner.osty.map_or(0, Number::from)
        }
        (card, key) => unimplemented!("the {} {card:?} works out", key.name()),
    }
}

/// Where a card's own play stops to ask something, under its card grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    Regent(regent::Resume),
    Silent(silent::Resume),
}

/// What a card does when it is played.
#[tracing::instrument(level = "trace", skip_all, fields(card = ?card))]
pub(crate) fn on_play<F: Fight<V>>(card: Cards, fight: &mut F) -> Played<V> {
    match card {
        Cards::Ironclad(card) => card_plays::ironclad::on_play(card, fight),
        Cards::Silent(card) => silent::on_play(card, fight),
        Cards::Regent(card) => regent::on_play(card, fight),
        Cards::Necrobinder(card) => necrobinder::on_play(card, fight),
        Cards::Defect(card) => defect::on_play(card, fight),
        Cards::Colorless(card) => card_plays::colorless::on_play(card, fight),
        Cards::Curse(card) => card_plays::curse::on_play(card, fight),
        Cards::Event(card) => card_plays::event::on_play(card, fight),
        Cards::Quest(card) => card_plays::quest::on_play(card, fight),
        Cards::Status(card) => card_plays::status::on_play(card, fight),
        Cards::Token(card) => token::on_play(card, fight),
        Cards::Deprecated(card) => card_plays::deprecated::on_play(card, fight),
    }
}

/// What a card still in the hand does when its owner's turn ends.
pub(crate) fn on_turn_end<F: Fight<V>>(card: Cards, fight: &mut F) {
    match card {
        Cards::Ironclad(card) => card_plays::ironclad::on_turn_end(card, fight),
        Cards::Silent(card) => card_plays::silent::on_turn_end(card, fight),
        Cards::Regent(card) => card_plays::regent::on_turn_end(card, fight),
        Cards::Necrobinder(card) => card_plays::necrobinder::on_turn_end(card, fight),
        Cards::Defect(card) => card_plays::defect::on_turn_end(card, fight),
        Cards::Colorless(card) => card_plays::colorless::on_turn_end(card, fight),
        Cards::Curse(card) => card_plays::curse::on_turn_end(card, fight),
        Cards::Event(card) => card_plays::event::on_turn_end(card, fight),
        Cards::Quest(card) => card_plays::quest::on_turn_end(card, fight),
        Cards::Status(card) => status::on_turn_end(card, fight),
        Cards::Token(card) => card_plays::token::on_turn_end(card, fight),
        Cards::Deprecated(card) => card_plays::deprecated::on_turn_end(card, fight),
    }
}

/// Deliver a notification to one card in the fight.
///
/// `printed` is what this copy of the card shows, which is what the model
/// reads its own numbers off.
pub(crate) fn event_hook<P: sts2_core::Printed<V> + ?Sized>(
    card: Cards,
    id: sts2_core::CardId<V>,
    printed: &P,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // The fight asks only a card whose class overrides the hook.
    match card {
        Cards::Status(card) => status::event_hook(card, id, printed, hook, asked),
        Cards::Necrobinder(card) => necrobinder::event_hook(card, id, printed, hook, asked),
        Cards::Regent(card) => regent::event_hook(card, id, printed, hook, asked),
        Cards::Silent(card) => silent::event_hook(card, id, hook, asked),
        card => unimplemented!("what {card:?} does for {hook:?}"),
    }
}

/// The rest of a play that stopped, once the fight has its answer.
pub(crate) fn resume_play<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        Resume::Regent(at) => regent::resume_play(fight, at, chosen),
        Resume::Silent(at) => silent::resume_play(fight, at, chosen),
    }
}
