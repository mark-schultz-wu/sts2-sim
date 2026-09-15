//! Combat card piles.

use sts2_core::rng::GameRng;

/// A deck known to have been shuffled.
///
/// Its representation is deliberately private. Callers know only that the
/// cards have been shuffled and can be dealt into combat piles.
#[derive(Debug)]
pub struct Shuffled<T> {
    cards: Vec<T>,
}

/// A combat's card piles.
///
/// Slice access keeps ownership and representation inside this module. The
/// storage can change without changing combat rules that inspect the piles.
#[derive(Debug)]
pub struct Piles<T> {
    hand: Vec<T>,
    draw: Vec<T>,
    discard: Vec<T>,
    exhaust: Vec<T>,
}

impl<T> Shuffled<T> {
    /// Deal up to `hand_size` cards from the front of the shuffled deck.
    pub fn deal(mut self, hand_size: usize) -> Piles<T> {
        let draw = self.cards.split_off(hand_size.min(self.cards.len()));
        Piles {
            hand: self.cards,
            draw,
            discard: Vec::new(),
            exhaust: Vec::new(),
        }
    }
}

impl<T> Piles<T> {
    /// Cards in hand, in hand order.
    pub fn hand(&self) -> &[T] {
        &self.hand
    }

    /// Cards in the draw pile, in draw order.
    pub fn draw(&self) -> &[T] {
        &self.draw
    }

    /// Cards in the discard pile, in the order they entered it.
    pub fn discard(&self) -> &[T] {
        &self.discard
    }

    /// Cards exhausted this combat, in the order they entered the pile.
    pub fn exhaust(&self) -> &[T] {
        &self.exhaust
    }

    /// Move one card from the hand to the discard pile.
    ///
    /// An index outside the current hand moves no card and says so.
    pub fn discard_from_hand(&mut self, index: usize) -> Result<(), NotInHand> {
        move_from_hand(&mut self.hand, &mut self.discard, index)
    }

    /// Move one card from the hand to the exhaust pile.
    ///
    /// An index outside the current hand moves no card and says so.
    pub fn exhaust_from_hand(&mut self, index: usize) -> Result<(), NotInHand> {
        move_from_hand(&mut self.hand, &mut self.exhaust, index)
    }
}

/// An index that names no card in hand.
///
/// A decision is applied to the hand it was made against. So this is a caller
/// holding a stale index, not a move the rules refuse. The rules refuse a card
/// with a [`sts2_core::Playable`] reason, before the choice. That never
/// reaches these piles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("no card at {index} in a hand of {held}")]
pub struct NotInHand {
    pub index: usize,
    pub held: usize,
}

fn move_from_hand<T>(
    hand: &mut Vec<T>,
    destination: &mut Vec<T>,
    index: usize,
) -> Result<(), NotInHand> {
    if index >= hand.len() {
        return Err(NotInHand {
            index,
            held: hand.len(),
        });
    }
    destination.push(hand.remove(index));
    Ok(())
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

    use super::{NotInHand, shuffle};

    fn rng() -> MegaRng {
        let seed = stream_seed::<V0_107_1>(&Seed::new("0RACLE1").expect("a canonical seed"));
        sts2_core::rng::named::<V0_107_1, MegaRng>(seed, "shuffle")
    }

    #[test]
    fn a_shuffled_deck_is_dealt_from_the_front() {
        let mut rng = rng();

        let piles = shuffle(&mut rng, (0..11).collect()).deal(5);

        assert_eq!(piles.hand(), [3, 8, 5, 9, 7]);
        assert_eq!(piles.draw(), [2, 10, 6, 1, 0, 4]);
        assert_eq!(rng.counter(), 10);
    }

    #[test]
    fn a_small_deck_deals_every_card() {
        let mut rng = rng();

        let piles = shuffle(&mut rng, vec![1, 2]).deal(5);

        assert_eq!(piles.hand().len(), 2);
        assert!(piles.draw().is_empty());
    }

    #[test]
    fn cards_move_from_hand_without_leaving_the_piles() {
        let mut rng = rng();
        let mut piles = shuffle(&mut rng, vec![1, 2, 3]).deal(3);

        let discarded = piles.hand()[1];
        let exhausted = piles.hand()[0];
        let kept = piles.hand()[2];

        piles
            .discard_from_hand(1)
            .expect("the second card is in hand");
        piles
            .exhaust_from_hand(0)
            .expect("the first card is in hand");

        assert_eq!(piles.hand(), [kept]);
        assert_eq!(piles.discard(), [discarded]);
        assert_eq!(piles.exhaust(), [exhausted]);
    }

    #[test]
    fn an_invalid_hand_index_changes_nothing() {
        let mut rng = rng();
        let mut piles = shuffle(&mut rng, vec![1, 2]).deal(2);

        assert_eq!(
            piles.discard_from_hand(2),
            Err(NotInHand { index: 2, held: 2 })
        );
        assert_eq!(
            piles.exhaust_from_hand(usize::MAX),
            Err(NotInHand {
                index: usize::MAX,
                held: 2
            })
        );

        assert_eq!(piles.hand().len(), 2);
        assert!(piles.discard().is_empty());
        assert!(piles.exhaust().is_empty());
    }
}
