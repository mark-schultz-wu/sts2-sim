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

/// A shuffled deck with its opening hand dealt off the front.
///
/// One list and where the hand ends in it, so dealing moves no card.
#[derive(Debug)]
pub struct Dealt<T> {
    cards: Vec<T>,
    hand: usize,
}

impl<T> Shuffled<T> {
    /// Deal up to `hand_size` cards from the front of the shuffled deck.
    pub fn deal(self, hand_size: usize) -> Dealt<T> {
        Dealt {
            hand: hand_size.min(self.cards.len()),
            cards: self.cards,
        }
    }
}

/// `CardPile.MaxCardsInHand`: the most a hand holds, and so what the fight's
/// first draw is capped at.
pub(super) const MAX_CARDS_IN_HAND: usize = 10;

impl<V: sts2_core::Rules> Shuffled<super::live::CardInstance<V>> {
    /// The same, for the one turn the innate cards are put on top first.
    ///
    /// `SetupPlayerTurn` on turn one. Each innate card moves to the top in
    /// pile order, so the last of them ends up first. The hand is drawn to at
    /// least their number, and at most a full hand.
    ///
    /// The game sinks an enchanted card whose `ShouldStartAtBottomOfDrawPile`
    /// says so first. `Rules::enchant` puts no such enchantment on a card.
    pub fn deal_opening(mut self, hand_size: usize) -> Dealt<super::live::CardInstance<V>> {
        let mut innate = 0;
        // `MoveToTopInternal` per card, in pile order. Each goes in front of
        // the last, which is what reverses them. The cards behind the one
        // moved keep their places, so the walk reaches each once.
        for at in 0..self.cards.len() {
            if self.cards[at].face.innate() {
                self.cards[..=at].rotate_right(1);
                innate += 1;
            }
        }
        self.deal(hand_size.max(innate).min(MAX_CARDS_IN_HAND))
    }
}

impl<T> Dealt<T> {
    /// Cards in hand, in hand order.
    pub fn hand(&self) -> &[T] {
        &self.cards[..self.hand]
    }

    /// Cards in the draw pile, in draw order.
    pub fn draw(&self) -> &[T] {
        &self.cards[self.hand..]
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

    use super::{Shuffled, shuffle};

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

    /// The fight's first draw puts the innate cards on top and draws them all.
    ///
    /// Two of them, so the order they end up in is visible. Each is moved to
    /// the top in turn, which leaves the later one in front.
    #[test]
    fn the_first_hand_opens_with_every_innate_card() {
        use crate::combat::live::{CardIds, CardInstance};
        use sts2_content::v0_107_1::cards::{Cards, RegentCard};
        use sts2_core::CardIdent;

        let mut ids = CardIds::<V0_107_1>::new();
        let plain = CardIdent::new(Cards::Regent(RegentCard::Strike));
        // `BigBang.OnUpgrade` is `AddKeyword(Innate)`, so an upgraded one
        // opens the fight in hand.
        let innate = CardIdent {
            upgrade: 1,
            ..CardIdent::new(Cards::Regent(RegentCard::BigBang))
        };
        let deck = [plain, innate, plain, plain, plain, innate, plain];
        let cards: Vec<_> = deck
            .iter()
            .map(|card| CardInstance::new(*card, ids.mint()))
            .collect();
        let named: Vec<_> = cards.iter().map(|card| card.id).collect();

        let piles = Shuffled { cards }.deal_opening(1);

        assert_eq!(
            piles.hand().iter().map(|card| card.id).collect::<Vec<_>>(),
            [named[5], named[1]],
            "both innate cards are drawn, the later one first"
        );
        assert_eq!(piles.draw().len(), 5);
    }
}
