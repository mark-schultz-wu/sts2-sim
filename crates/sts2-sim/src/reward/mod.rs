//! Rewards offered by rooms, events, and relics.

pub(crate) mod card;
pub(crate) mod combat;
pub(crate) mod potion;

use sts2_core::rng::GameRng;
use sts2_core::{RarityKinds, Rules};

/// `RelicFactory.RollRarity`, which is one draw and three bands.
///
/// `NextFloat`, as the potion roll reads one. A fight's relic, a merchant's and
/// a treasure room's all roll it, each off its own stream.
pub(crate) fn relic_rarity<V: Rules, R: GameRng>(rng: &mut R) -> crate::run::Rarity<V> {
    let rolled = rng.next_double() as f32;
    match rolled {
        r if r < 0.5 => <crate::run::Rarity<V> as RarityKinds>::COMMON,
        r if r < 0.83 => <crate::run::Rarity<V> as RarityKinds>::UNCOMMON,
        _ => <crate::run::Rarity<V> as RarityKinds>::RARE,
    }
}

/// `GoldReward.Populate` for a reward made at one amount, which still draws
/// once, over the range from the amount to the amount.
pub(crate) fn fixed_gold<V: Rules, R: GameRng>(rng: &mut R, amount: V::Number) -> V::Number {
    use sts2_core::rng::{RngInteger as _, Span};

    let min: R::Int = num_traits::cast(amount).expect("a gold amount fits the RNG integer");
    let span = Span::new(min, min + R::Int::ONE).expect("a range one wide");
    num_traits::cast(rng.next_int_range(span)).expect("a reward's gold is a game number")
}

/// One thing a reward screen offers, and whether it is still on offer.
///
/// The indices never renumber as rewards are taken, so a taken reward stays in
/// the list with a flag rather than leaving it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Offer<V: Rules> {
    Gold {
        amount: V::Number,
        taken: bool,
    },
    /// The cards on offer. Taking one takes the whole reward.
    Card {
        cards: Vec<sts2_core::CardIdent<V>>,
        taken: bool,
    },
    /// One stolen deck card returned by the creature that took it.
    SpecialCard {
        card: sts2_core::CardIdent<V>,
        taken: bool,
    },
    Potion {
        potion: V::Potion,
        taken: bool,
    },
    /// A relic, which is what an elite gives. An elite's slot picks no
    /// rarity of its own, so what it reports is `RelicRarity.None`, whatever
    /// the relic in it is.
    Relic {
        relic: V::Relic,
        /// `RelicReward.Rarity`: the rarity the reward was asked for, and
        /// `Undrafted` for one that rolled its own.
        rarity: crate::run::Rarity<V>,
        taken: bool,
    },
    /// `CardRemovalReward`: one card of the deck removed, which the player
    /// picks once it is taken.
    CardRemoval {
        taken: bool,
    },
}

impl<V: Rules> Offer<V> {
    pub(crate) fn taken(&self) -> bool {
        match self {
            Self::Gold { taken, .. }
            | Self::Card { taken, .. }
            | Self::SpecialCard { taken, .. }
            | Self::Potion { taken, .. }
            | Self::Relic { taken, .. }
            | Self::CardRemoval { taken } => *taken,
        }
    }

    /// `Reward.RewardsSetIndex`, which a set is sorted by once it is filled
    /// in. The sort is stable, so rewards of one kind keep their order.
    pub(crate) fn set_index(&self) -> u8 {
        match self {
            Self::Gold { .. } => 1,
            Self::Potion { .. } => 2,
            Self::Relic { .. } => 3,
            Self::SpecialCard { .. } => 4,
            Self::Card { .. } => 5,
            Self::CardRemoval { .. } => 7,
        }
    }
}

/// `RewardsSet`'s sort, once every reward is filled in.
pub(crate) fn sort<V: Rules>(offered: &mut [Offer<V>]) {
    offered.sort_by_key(Offer::set_index);
}

/// The two pity counters a run carries between reward screens.
///
/// `AbstractOdds.CurrentValue`, which no observation reports. Both move on
/// every roll, so a run that has seen one reward screen rolls the next one
/// differently. See [`card::encounter`] and [`combat::potion`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Odds {
    /// `CardRarityOdds`, added to the odds of a rare.
    pub(crate) rarity: f32,
    /// `PotionRewardOdds`, the chance of a potion at all.
    pub(crate) potion: f32,
}

impl Default for Odds {
    /// What a run starts at, which is each class's own starting value.
    fn default() -> Self {
        Self {
            rarity: -0.05,
            potion: 0.4,
        }
    }
}
