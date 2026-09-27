//! The deck as a fight opens: shuffled, then dealt.

use sts2_core::rng::GameRng;

/// A deck known to have been shuffled.
///
/// Its representation is deliberately private. Callers know only that the
/// cards have been shuffled and can be dealt into combat piles.
#[derive(Debug)]
pub struct Shuffled<T> {
    cards: Vec<T>,
}

/// The most a hand holds, and so what the fight's first draw is capped at.
pub(super) use sts2_core::MAX_CARDS_IN_HAND;

impl<T> Shuffled<T> {
    /// The whole deck in shuffled order, which is the draw pile a fight opens
    /// on, top first.
    pub fn into_cards(self) -> Vec<T> {
        self.cards
    }
}

/// Shuffle owned deck storage and return it in the shuffled state.
pub fn shuffle<T, R: GameRng>(rng: &mut R, mut cards: Vec<T>) -> Shuffled<T> {
    rng.unstable_shuffle(&mut cards);
    Shuffled { cards }
}

#[cfg(test)]
mod tests {
    use crate::rng::MegaRng;
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_core::Seed;
    use sts2_core::rng::{GameRng, stream_seed};

    use super::shuffle;

    fn rng() -> MegaRng {
        let seed = stream_seed::<V0_107_1>(&Seed::new("0RACLE1").expect("a canonical seed"));
        sts2_core::rng::named::<V0_107_1, MegaRng>(seed, "shuffle")
    }

    #[test]
    fn a_deck_is_shuffled_whole() {
        let mut rng = rng();

        let cards = shuffle(&mut rng, (0..11).collect()).into_cards();

        assert_eq!(cards, [3, 8, 5, 9, 7, 2, 10, 6, 1, 0, 4]);
        assert_eq!(rng.counter(), 10);
    }
}
