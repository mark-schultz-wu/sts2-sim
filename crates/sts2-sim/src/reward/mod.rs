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
        /// How the pack was rolled, which a reroll rolls again. Nothing for
        /// a pack set by hand.
        rolled: Option<Rolled<V>>,
        /// `CanReroll`.
        reroll: bool,
        /// Whether `ModifyCardRewardAlternatives` adds `SACRIFICE`.
        sacrifice: bool,
        taken: bool,
    },
    /// `SpecialCardReward`: one card, and what its model saved. An event's
    /// has nothing saved. A card a thief hands back has what it saved in the
    /// deck, and is the deck card `returning` names, which joins it again.
    SpecialCard {
        card: sts2_core::CardIdent<V>,
        state: V::CardState,
        returning: Option<sts2_core::DeckCardId>,
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
        /// `IsWax`, which Toy Box sets on the relics it offers.
        wax: bool,
        taken: bool,
    },
    /// `CardRemovalReward`: one card of the deck removed, which the player
    /// picks once it is taken.
    CardRemoval {
        taken: bool,
    },
}

/// How a card reward's pack was rolled: `CardReward.Options`, which its
/// `RerollOptions` repeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rolled<V: Rules> {
    /// At the odds of a room of this kind.
    Room(combat::Room),
    /// As a custom reward of this kind, off this stream.
    Custom {
        reward: sts2_core::CustomReward<V>,
        stream: sts2_core::Stream<V>,
    },
}

impl<V: Rules> Offer<V> {
    /// `CardRewardAlternative.Generate` less `Skip`, in its order: `REROLL`
    /// where the reward can reroll, then what the hook adds. Nothing for any
    /// other kind of reward.
    pub(crate) fn alternatives(&self) -> impl Iterator<Item = sts2_core::RewardAlternative> {
        use sts2_core::RewardAlternative;

        let (reroll, sacrifice) = match self {
            Self::Card {
                reroll, sacrifice, ..
            } => (*reroll, *sacrifice),
            _ => (false, false),
        };
        [
            reroll.then_some(RewardAlternative::Reroll),
            sacrifice.then_some(RewardAlternative::Sacrifice),
        ]
        .into_iter()
        .flatten()
    }

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
