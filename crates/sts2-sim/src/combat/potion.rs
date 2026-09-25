//! Drinking one potion into the fight.
//!
//! `PotionModel.OnUseWrapper`: the potion leaves the belt, then its own effect
//! runs through the board a card acts through. Whether it can be drunk at all
//! is the belt's question, and the run asks it before this.

use sts2_core::{CombatId, Rules};

use super::board::Board;
use super::{Error, Fighting};

/// Drink one potion into the fight where it stands.
///
/// Whatever the drink ended, if it ended the fight.
pub(crate) fn drink<V: Rules>(
    fighting: Fighting<'_, V>,
    potion: V::Potion,
    target: Option<CombatId>,
) -> Result<Option<super::Ended>, Error> {
    let Fighting {
        live,
        streams,
        relics,
    } = fighting;
    let ascension = live.ascension;
    let mut board = Board::for_card(live, streams.combat(), relics, ascension, target, None);
    let used = V::on_use(potion, &mut board);
    let ended = board.ended;
    let skipped = std::mem::take(&mut board.skipped);

    super::play::merge::<V>(&mut live.skip_tick, skipped);
    super::play::carry_on(
        Fighting {
            live,
            streams,
            relics,
        },
        ascension,
        super::live::Resuming {
            // A drink runs once. `GeneratePlayCount` belongs to a card.
            plays: super::live::Plays {
                done: num_traits::zero(),
                count: num_traits::one(),
            },
            asker: super::live::Asker::Potion(potion),
            target,
            ended,
        },
        used,
    )
}
