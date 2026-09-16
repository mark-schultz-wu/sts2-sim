//! Rewards offered by rooms, events, and relics.

pub(crate) mod card;
pub(crate) mod combat;
pub(crate) mod potion;

use sts2_core::Rules;

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
    /// An elite's relic. The slot picks no rarity of its own, so what it
    /// reports is `RelicRarity.None`, whatever the relic in it is.
    Relic {
        relic: V::Relic,
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
            | Self::Relic { taken, .. } => *taken,
        }
    }
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
