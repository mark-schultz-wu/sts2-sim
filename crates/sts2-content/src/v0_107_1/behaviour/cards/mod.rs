//! `CardModel.OnPlay`, ported.
//!
//! One module per pool, filed the way `generated/cards/` is, so a card's
//! behaviour sits under the same name as its row. A pool gets a module when
//! its first card is ported.
//!
//! [`on_play`] panics for a card nobody has ported, so this tree is also the
//! list of what the simulator can play. A card outside it stops the run where
//! it stands rather than playing as if it did nothing.

use sts2_core::{CardTable as _, CardVars, Cost, Fight, Played};

use super::super::V0_107_1;
use super::super::generated::cards::{CardVar, Cards};

/// What the run holds a card's numbers as.
type Number = <V0_107_1 as sts2_core::ContentVersion>::Number;

/// One number on a card's face, as this copy of it prints one.
type CardNumber = sts2_core::CardNumber<CardVar, Number>;

mod colorless;
mod regent;
mod status;
mod token;

type V = V0_107_1;

/// Apply the printed-number changes from a card's first upgrade.
///
/// `CardModel.Upgrade`, which each card overrides to raise the numbers it
/// prints. Filed the way the play is: by pool here, then by the card's own
/// rarity. An upgrade then sits beside the play of the same card.
pub(crate) fn upgrade(card: Cards, printed: &mut [CardNumber]) {
    match card {
        Cards::Regent(card) => regent::upgrade(card, printed),
        Cards::Token(card) => token::upgrade(card, printed),
        card => unimplemented!("card upgrade: {card:?}"),
    }
}

/// What one copy of a card costs at the upgrade it is at.
///
/// `CardModel.EnergyCost` and `StarCost`. The table prints a fresh copy's, and
/// an `OnUpgrade` that calls `UpgradeBy` moves it by that much per upgrade. A
/// cost that is not a number stays what it is: `GetWithModifiers` returns
/// early for one, so an upgrade moves nothing.
pub(crate) fn cost(card: Cards, upgrade: u8) -> (Cost, Cost) {
    if upgrade == 0 {
        return (card.cost(), card.star_cost());
    }
    // A card upgraded in a fight went through `upgrade` to get here, and one
    // upgraded outside a fight went through it as the fight built its copy.
    // Both panic for a card nobody has ported, so a card reaching this is one
    // whose pool is ported and whose costs stand where the table put them.
    let (energy, star) = match card {
        Cards::Regent(card) => regent::upgrade_cost(card),
        Cards::Token(card) => token::upgrade_cost(card),
        card => unimplemented!("what an upgraded {card:?} costs"),
    };
    let moved = |cost: Cost, by: i8| match cost {
        // `GetWithModifiers` returns early for a cost that is not a number, so
        // an upgrade moves nothing.
        Cost::Fixed(fixed) => Cost::Fixed(fixed.saturating_add_signed(by)),
        cost => cost,
    };
    let each = |cost, by: i8| (0..upgrade).fold(cost, |cost, _| moved(cost, by));
    (each(card.cost(), energy), each(card.star_cost(), star))
}

/// Raise the numbers one card's upgrade raises.
///
/// A card that raises nothing it prints passes an empty list. A card nobody
/// has ported never reaches this.
fn raise(printed: &mut [CardNumber], changes: &[(CardVar, Number)]) {
    for &(key, amount) in changes {
        printed
            .iter_mut()
            .find(|var| var.id == key)
            .expect("an upgraded variable belongs to the card")
            .base += amount;
    }
}

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
pub(crate) fn calculated(
    card: Cards,
    key: CardVar,
    printed: &[CardNumber],
    powers: &[sts2_core::Stack<V0_107_1>],
) -> Number {
    match card {
        Cards::Token(card) => token::calculated(card, key, printed, powers),
        card => unimplemented!("the {} {card:?} works out", key.name()),
    }
}

/// One of the numbers on a card's own face.
///
/// A key the card does not print is a mistake in the port.
fn face(printed: &[CardNumber], key: CardVar) -> Number {
    printed
        .iter()
        .find(|var| var.id == key)
        .unwrap_or_else(|| unimplemented!("the {} this card does not print", key.name()))
        .base
}

/// Where a card's own play stops to ask something, under its card grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    Regent(regent::Resume),
}

/// What a card does when it is played.
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
pub(crate) fn event_hook(
    card: Cards,
    printed: &[CardNumber],
    hook: sts2_core::EventHook<V>,
) -> Vec<sts2_core::HookEffect<V>> {
    match card {
        Cards::Status(card) => status::event_hook(card, printed, hook),
        // A card that overrides no hook answers nothing, which is what
        // `AbstractModel` does for all but a handful of them.
        _ => Vec::new(),
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
