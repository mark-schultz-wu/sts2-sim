//! Drinking one potion into the fight.
//!
//! `PotionModel.OnUseWrapper`: the potion leaves the belt, then its own effect
//! runs through the board a card acts through. Whether it can be drunk at all
//! is the belt's question, and the run asks it before this.

use sts2_core::{CombatId, Rules};

use super::Error;
use super::board::Board;
use super::live::Live;
use crate::stream::Streams;

/// Drink one potion, and give back the fight it leaves behind.
///
/// Nothing is committed until this returns, as playing a card is.
pub(crate) fn drink<V: Rules>(
    live: &Live<V>,
    potion: V::Potion,
    target: Option<CombatId>,
    streams: &Streams<V>,
    relics: &[crate::run::HeldRelic<V>],
) -> Result<super::Fought<V>, Error> {
    let mut next = live.clone();
    let mut relics = relics.to_vec();
    let mut streams = streams.clone();

    let mut board = Board::for_card(
        &mut next,
        streams.combat(),
        &mut relics,
        live.ascension,
        target,
        None,
    );
    let used = V::on_use(potion, &mut board);
    let won = board.won;
    let skipped = std::mem::take(&mut board.skipped);

    next.skip_tick = super::play::merged::<V>(&next.skip_tick, skipped);
    super::play::carry_on(
        next,
        streams,
        relics,
        live.ascension,
        super::live::Resuming {
            // A drink runs once. `GeneratePlayCount` belongs to a card.
            plays: super::live::Plays {
                done: num_traits::zero(),
                count: num_traits::one(),
            },
            asker: super::live::Asker::Potion(potion),
            target,
            won,
        },
        used,
    )
}
