//! `CardModel.OnPlay`, ported.
//!
//! One module per pool, filed the way `generated/cards/` is, so a card's
//! behaviour sits under the same name as its row. A pool gets a module when
//! its first card is ported.
//!
//! [`on_play`] panics for a card nobody has ported, so this tree is also the
//! list of what the simulator can play. A card outside it stops the run where
//! it stands rather than playing as if it did nothing.

use sts2_core::{CardVars, Fight, Played};

use super::super::V0_107_1;
use super::super::generated::cards::{CardVar, Cards};

/// What the run holds a card's numbers as.
type Number = <V0_107_1 as sts2_core::ContentVersion>::Number;

mod colorless;
mod regent;
mod status;
mod token;

type V = V0_107_1;

/// What a card's first upgrade writes on the copy being upgraded.
///
/// `CardModel.Upgrade`, which each card overrides to raise the numbers it
/// prints. Filed the way the play is: by pool here, then by the card's own
/// rarity.
///
/// Declared rather than applied: an upgrade writes in the same vocabulary any
/// other effect does, and the fight owns the copy it writes on.
pub(crate) fn upgrade(card: Cards) -> &'static [Change] {
    match card {
        Cards::Regent(card) => regent::upgrade(card),
        Cards::Token(card) => token::upgrade(card),
        card => unimplemented!("card upgrade: {card:?}"),
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
    powers: &[sts2_core::Stack<V0_107_1>],
) -> Number {
    match card {
        Cards::Token(card) => token::calculated(card, key, printed, powers),
        card => unimplemented!("the {} {card:?} works out", key.name()),
    }
}

/// Where a card's own play stops to ask something, under its card grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    Regent(regent::Resume),
}

/// What a card does when it is played.
#[tracing::instrument(level = "trace", skip_all, fields(card = ?card))]
pub(crate) fn on_play<F: Fight<V>>(card: Cards, fight: &mut F) -> Played<V> {
    match card {
        Cards::Colorless(card) => colorless::on_play(card, fight),
        Cards::Regent(card) => regent::on_play(card, fight),
        Cards::Status(card) => status::on_play(card, fight),
        Cards::Token(card) => token::on_play(card, fight),
        // Nothing from these pools is ported. Named rather than left to a
        // wildcard, so a build that adds a pool stops here.
        Cards::Ironclad(_)
        | Cards::Silent(_)
        | Cards::Necrobinder(_)
        | Cards::Defect(_)
        | Cards::Curse(_)
        | Cards::Event(_)
        | Cards::Quest(_)
        | Cards::Deprecated(_) => unimplemented!("what {card:?} does when it is played"),
    }
}

/// What a card still in the hand does when its owner's turn ends.
pub(crate) fn on_turn_end<F: Fight<V>>(card: Cards, fight: &mut F) {
    if let Cards::Status(card) = card {
        status::on_turn_end(card, fight);
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
    }
}
