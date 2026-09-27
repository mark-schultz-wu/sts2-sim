//! The Future of Potions: a potion traded for an upgraded card of its rarity.

use nonmax::NonMaxU8;
use sts2_core::{
    CardKind, CustomReward, EventBehavior, EventView, HookEffect, HookEffects, RewardRarity, Room,
    Table as _,
};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::events::{Event, EventOption as Said};
use super::super::super::generated::potions::{Potion, PotionRarity};

/// The most potions it offers, which is `Mathf.Min(3, ...)`.
const OFFERED: usize = 3;

/// How many cards the reward offers.
const CARDS: usize = 3;

/// One reward for each rarity and card type, in the order `reward` reads.
static REWARDS: [[CustomReward<V>; 1]; 9] = {
    const fn reward(rarity: RewardRarity, kind: CardKind) -> [CustomReward<V>; 1] {
        [CustomReward::UpgradedCardsOf {
            rarity,
            kind,
            count: CARDS,
        }]
    }
    [
        reward(RewardRarity::Common, CardKind::Attack),
        reward(RewardRarity::Common, CardKind::Skill),
        reward(RewardRarity::Common, CardKind::Power),
        reward(RewardRarity::Uncommon, CardKind::Attack),
        reward(RewardRarity::Uncommon, CardKind::Skill),
        reward(RewardRarity::Uncommon, CardKind::Power),
        reward(RewardRarity::Rare, CardKind::Attack),
        reward(RewardRarity::Rare, CardKind::Skill),
        reward(RewardRarity::Rare, CardKind::Power),
    ]
};

#[derive(Debug, Clone)]
pub struct State {
    /// `PotionToCardType`, one for each potion in the belt, in slot order.
    offers: Vec<(Potion, CardKind)>,
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct TheFutureOfPotions;

/// A potion's `Rarity`, which a deprecated potion lacks.
fn rarity_of(potion: Potion) -> PotionRarity {
    potion
        .data()
        .rarity
        .unwrap_or_else(|| unimplemented!("trading {potion:?}, which has no rarity"))
}

/// `GetCardRarity`.
fn card_rarity(potion: Potion) -> RewardRarity {
    match rarity_of(potion) {
        PotionRarity::Rare | PotionRarity::Event => RewardRarity::Rare,
        PotionRarity::Uncommon => RewardRarity::Uncommon,
        PotionRarity::Common | PotionRarity::Token => RewardRarity::Common,
    }
}

impl EventBehavior<V> for TheFutureOfPotions {
    type State = State;

    /// Two potions or more, which the table of allowed events checks.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `PotionToCardType`: a card type for each potion off the event's
    /// stream, by `NextItem`. A common or token potion leaves Power out.
    fn enter<R: sts2_core::rng::GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> State {
        let offers = run
            .potions()
            .map(|potion| {
                let kinds: &[CardKind] = match rarity_of(potion) {
                    PotionRarity::Common | PotionRarity::Token => {
                        &[CardKind::Attack, CardKind::Skill]
                    }
                    _ => &[CardKind::Attack, CardKind::Skill, CardKind::Power],
                };
                let kind = *rng
                    .choose(kinds)
                    .unwrap_or_else(|_| unreachable!("a potion has a card type to draw"));
                (potion, kind)
            })
            .collect();
        State {
            offers,
            chosen: None,
        }
    }

    /// One option for each of the first three potions.
    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let options: Vec<(Said, bool)> = state
            .offers
            .iter()
            .take(OFFERED)
            .map(|_| (Said::TheFutureOfPotionsPotion, false))
            .collect();
        super::page(Event::TheFutureOfPotions, &options, state.chosen)
    }

    /// `Trade`: the potion discarded, then a reward of three upgraded cards of
    /// its rarity and the type drawn for it. That finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let at = usize::from(option);
        let (potion, kind) = state.offers[at];
        let rarity = match card_rarity(potion) {
            RewardRarity::Common => 0,
            RewardRarity::Uncommon => 1,
            RewardRarity::Rare => 2,
            RewardRarity::Other => unreachable!("a potion's card rarity is a reward rarity"),
        };
        let kind = match kind {
            CardKind::Attack => 0,
            CardKind::Skill => 1,
            CardKind::Power => 2,
            kind => unreachable!("a trade drew {kind:?}"),
        };
        smallvec::smallvec![
            HookEffect::DiscardPotion(at),
            HookEffect::OfferRewards(&REWARDS[rarity * 3 + kind]),
            HookEffect::FinishEvent,
        ]
    }
}
