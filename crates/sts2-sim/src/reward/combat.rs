//! What winning a fight offers.
//!
//! `RewardsSet.GenerateRewardsFor` for a monster room: gold, then a potion
//! roll, then three cards. Every draw comes off the rewards stream, and the
//! order is what puts the stream where the next screen finds it.

use sts2_core::rng::{GameRng, Interval, RngInteger, Span};
use sts2_core::{AscensionKinds, Rules};

use crate::run::Rarity;

use super::{Odds, Offer, card};

/// How many cards a fight's card reward offers.
pub(crate) const CARDS_OFFERED: usize = 3;

/// The kind of room a won fight was in, which `GenerateRewardsFor` switches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Room {
    Monster,
    Elite,
    Boss,
}

impl Room {
    /// `CardCreationOptions.ForRoom`, which is the odds its card reward rolls
    /// its rarities against.
    pub(crate) fn card_odds(self) -> card::RarityOdds {
        match self {
            Room::Monster => card::RarityOdds::Regular,
            Room::Elite => card::RarityOdds::Elite,
            Room::Boss => card::RarityOdds::Boss,
        }
    }
}

/// What the belt says of a won fight's rewards before they roll.
pub(crate) struct Modifiers<'a> {
    /// `Hook.ShouldForcePotionReward`, which pays the potion whatever the
    /// roll.
    pub(crate) forced: bool,
    /// `TryModifyCardRewardOptions`: the kind of each option added to the
    /// card reward, drawn once its own cards are.
    pub(crate) added: &'a [sts2_core::CardKind],
}

/// The rewards a won fight offers.
///
/// The potion rolls while the set is built. The gold rolls while the set is
/// filled in. So the potion draws first, and the gold sits ahead of it.
///
/// An elite adds a relic. `Populate` runs in build order and `RewardsSet`
/// sorts by `RewardsSetIndex` after, so the relic draws last and shows third.
#[allow(clippy::too_many_arguments)]
pub(crate) fn won<V: Rules, R: GameRng>(
    rng: &mut R,
    odds: &mut Odds,
    ascension: V::AscensionLevel,
    character: V::Character,
    gold: Option<Interval<u16>>,
    act: usize,
    room: Room,
    modifiers: Modifiers<'_>,
) -> (Vec<Offer<V>>, Option<Rarity<V>>) {
    let elite = room == Room::Elite;
    let pays_potion = potion(rng, odds, elite, modifiers.forced);
    let mut offered: Vec<Offer<V>> = gold
        .into_iter()
        .map(|gold| Offer::Gold {
            amount: purse::<V, R>(rng, gold),
            taken: false,
        })
        .collect();
    if pays_potion {
        offered.push(Offer::Potion {
            potion: super::potion::rolled::<V, R>(rng, character),
            taken: false,
        });
    }
    offered.push(card_reward::<V, R>(
        rng,
        odds,
        ascension,
        character,
        act,
        room,
        modifiers.added,
    ));
    // The rarity is rolled here and the relic is pulled by the caller, which
    // is the half that holds the run's bags.
    let rarity = elite.then(|| super::relic_rarity::<V, R>(rng));
    (offered, rarity)
}

/// `CardReward.Populate` for `CardCreationOptions.ForRoom`: three cards at
/// the room's odds, then an option of each kind in `added`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn card_reward<V: Rules, R: GameRng>(
    rng: &mut R,
    odds: &mut Odds,
    ascension: V::AscensionLevel,
    character: V::Character,
    act: usize,
    room: Room,
    added: &[sts2_core::CardKind],
) -> Offer<V> {
    let mut cards = card::encounter::<V, R>(
        rng,
        odds,
        ascension,
        character,
        CARDS_OFFERED,
        act,
        room.card_odds(),
    );
    for &kind in added {
        // The pool the reward drew, of the kind, less what it offers. Every
        // card of the kind when it offers them all, and nothing with none.
        let pool = V::reward_cards(character);
        let of_kind: Vec<V::Card> = [pool.common, pool.uncommon, pool.rare]
            .into_iter()
            .flatten()
            .copied()
            .filter(|&card| V::card_kind(card) == Some(kind))
            .collect();
        let unoffered: Vec<V::Card> = of_kind
            .iter()
            .copied()
            .filter(|&card| cards.iter().all(|offered| offered.card != card))
            .collect();
        let drawn = if unoffered.is_empty() {
            of_kind
        } else {
            unoffered
        };
        if !drawn.is_empty() {
            cards.push(card::added::<V, R>(rng, &drawn, room.card_odds(), act));
        }
    }
    Offer::Card {
        cards,
        rolled: Some(room),
        reroll: false,
        sacrifice: false,
        taken: false,
    }
}

/// `GoldReward.Populate`, which is one draw over the encounter's own range.
///
/// The range is the encounter's own, scaled by the room's `GoldProportion`.
fn purse<V: Rules, R: GameRng>(rng: &mut R, gold: Interval<u16>) -> V::Number {
    let min = num_traits::cast(gold.min()).expect("a gold minimum fits the RNG integer");
    let max: R::Int = num_traits::cast(gold.max()).expect("a gold maximum fits the RNG integer");
    let max = max + R::Int::ONE;
    let span = Span::new(min, max).expect("a gold range that ends above where it starts");
    num_traits::cast(rng.next_int_range(span)).expect("a fight's gold is a game number")
}

/// `PotionRewardOdds.Roll`, which draws whether or not a potion follows.
/// A `forced` roll pays one whatever it draws.
///
/// The chance moves either way by a tenth, so a run that has gone without one
/// is likelier to see the next. An elite adds a quarter of a bonus, halved,
/// to what the draw is held against, for this roll and no later one.
fn potion<R: GameRng>(rng: &mut R, odds: &mut Odds, elite: bool, forced: bool) -> bool {
    const STEP: f32 = 0.1;
    const ELITE_BONUS: f32 = 0.25;
    let against = match elite {
        true => odds.potion + ELITE_BONUS * 0.5,
        false => odds.potion,
    };
    let rolled = rng.next_double() as f32;
    match forced || rolled < against {
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
