//! What winning a fight offers.
//!
//! `RewardsSet.GenerateRewardsFor` for a monster room: gold, then a potion
//! roll, then three cards. Every draw comes off the rewards stream, and the
//! order is what puts the stream where the next screen finds it.

use sts2_core::rng::{GameRng, Interval, RngInteger, Span};
use sts2_core::{AscensionKinds, Rules};

use super::{Odds, Offer, card};

/// How many cards a fight's card reward offers.
const CARDS_OFFERED: usize = 3;

/// The rewards a won monster fight offers.
///
/// The potion rolls while the set is built. The gold rolls while the set is
/// filled in. So the potion draws first, and the gold still sits ahead of it
/// on the screen.
pub(crate) fn won<V: Rules, R: GameRng>(
    rng: &mut R,
    odds: &mut Odds,
    ascension: V::AscensionLevel,
    character: V::Character,
    gold: Interval<u16>,
    act: usize,
) -> Vec<Offer<V>> {
    let pays_potion = potion(rng, odds);
    let mut offered = vec![Offer::Gold {
        amount: purse::<V, R>(rng, gold),
        taken: false,
    }];
    if pays_potion {
        offered.push(Offer::Potion {
            potion: super::potion::rolled::<V, R>(rng, character),
            taken: false,
        });
    }
    offered.push(Offer::Card {
        cards: card::encounter::<V, R>(rng, odds, ascension, character, CARDS_OFFERED, act),
        taken: false,
    });
    offered
}

/// `GoldReward.Populate`, which is one draw over the encounter's own range.
///
/// The room's `GoldProportion` is one for every fight the map deals. An event
/// that hands out a part of a fight's gold sets it, and none is ported.
fn purse<V: Rules, R: GameRng>(rng: &mut R, gold: Interval<u16>) -> V::Number {
    let min = num_traits::cast(gold.min()).expect("a gold minimum fits the RNG integer");
    let max: R::Int = num_traits::cast(gold.max()).expect("a gold maximum fits the RNG integer");
    let max = max + R::Int::ONE;
    let span = Span::new(min, max).expect("a gold range that ends above where it starts");
    num_traits::cast(rng.next_int_range(span)).expect("a fight's gold is a game number")
}

/// `PotionRewardOdds.Roll`, which draws whether or not a potion follows.
///
/// The chance moves either way by a tenth, so a run that has gone without one
/// is likelier to see the next. The elite bonus is not here: no elite is.
fn potion<R: GameRng>(rng: &mut R, odds: &mut Odds) -> bool {
    const STEP: f32 = 0.1;
    let rolled = rng.next_double() as f32;
    match rolled < odds.potion {
        true => {
            odds.potion -= STEP;
            true
        }
        false => {
            odds.potion += STEP;
            false
        }
    }
}

/// The rule the two odds share, for the ascension that moves both.
pub(crate) fn scarce<V: Rules>(ascension: V::AscensionLevel) -> bool {
    ascension.has(<V::AscensionLevel as AscensionKinds>::SCARCITY)
}
