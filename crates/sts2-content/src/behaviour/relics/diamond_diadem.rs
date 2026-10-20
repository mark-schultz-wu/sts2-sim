//! `DiamondDiadem`: half damage from attacks after a turn of few cards.
//!
//! The power it applies is each build's own, so each build ports the hook.

use crate::behaviour::relics::Number;

/// `CardThreshold`, the most cards a turn can play and still count.
pub(crate) const THRESHOLD: u32 = 2;

/// `CardsPlayedThisTurn`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct State {
    pub(crate) cards_played: u32,
}

/// `DisplayAmount`: the cards played this turn.
pub(super) fn display_amount(state: &State) -> Number {
    Number::try_from(state.cards_played).expect("a turn's plays fit a game number")
}
