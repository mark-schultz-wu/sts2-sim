//! The Future of Potions: a potion traded for an upgraded card of its rarity.

use nonmax::NonMaxU8;
use sts2_core::{
    CardKind, EventBehavior, EventView, HookEffect, HookEffects, RewardRarity, Room, Table as _,
};

use crate::generated::potions::PotionRarity;
use crate::ids::{EventIds as _, EventOptionIds as _};

/// The most potions it offers, which is `Mathf.Min(3, ...)`.
const OFFERED: usize = 3;

/// How many cards the reward offers.
pub(crate) const CARDS: usize = 3;

#[derive(Debug, Clone)]
pub struct State<V: crate::Build> {
    /// `PotionToCardType`, one for each potion in the belt, in slot order.
    offers: Vec<(V::Potion, CardKind)>,
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct TheFutureOfPotions;

/// A potion's `Rarity`, which a deprecated potion lacks.
fn rarity_of<V: crate::Build>(potion: V::Potion) -> PotionRarity {
    potion
        .data()
        .rarity
        .unwrap_or_else(|| unimplemented!("trading {potion:?}, which has no rarity"))
}

/// `GetCardRarity`.
fn card_rarity<V: crate::Build>(potion: V::Potion) -> RewardRarity {
    match rarity_of::<V>(potion) {
        PotionRarity::Rare | PotionRarity::Event => RewardRarity::Rare,
        PotionRarity::Uncommon => RewardRarity::Uncommon,
        PotionRarity::Common | PotionRarity::Token => RewardRarity::Common,
    }
}

impl<V: crate::Build> EventBehavior<V> for TheFutureOfPotions {
    type State = State<V>;

    /// Two potions or more, which the table of allowed events checks.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `PotionToCardType`: a card type for each potion off the event's
    /// stream, by `NextItem`. A common or token potion leaves Power out.
    fn enter<R: sts2_core::rng::GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> State<V> {
        let offers = run
            .potions()
            .map(|potion| {
                let kinds: &[CardKind] = match rarity_of::<V>(potion) {
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
    fn observe(&self, state: &State<V>, _run: &impl EventView<V>) -> Room<V> {
        let options: Vec<(V::EventOption, bool)> = state
            .offers
            .iter()
            .take(OFFERED)
            .map(|_| (V::EventOption::THE_FUTURE_OF_POTIONS_POTION, false))
            .collect();
        crate::behaviour::events::page::<V>(V::Event::THE_FUTURE_OF_POTIONS, &options, state.chosen)
    }

    /// `Trade`: the potion discarded, then a reward of three upgraded cards of
    /// its rarity and the type drawn for it. That finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State<V>,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let at = usize::from(option);
        let (potion, kind) = state.offers[at];
        let rarity = match card_rarity::<V>(potion) {
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
            HookEffect::OfferRewards(&V::THE_FUTURE_OF_POTIONS_REWARDS[rarity * 3 + kind]),
            HookEffect::FinishEvent,
        ]
    }
}

/// `REWARDS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// One reward for each rarity and card type, in the order `reward` reads.
        const THE_FUTURE_OF_POTIONS_REWARDS: &'static [[sts2_core::CustomReward<$build>; 1]] = &{
            const fn reward(
                rarity: sts2_core::RewardRarity,
                kind: sts2_core::CardKind,
            ) -> [sts2_core::CustomReward<$build>; 1] {
                [sts2_core::CustomReward::UpgradedCardsOf {
                    rarity,
                    kind,
                    count: crate::behaviour::events::the_future_of_potions::CARDS,
                }]
            }
            [
                reward(sts2_core::RewardRarity::Common, sts2_core::CardKind::Attack),
                reward(sts2_core::RewardRarity::Common, sts2_core::CardKind::Skill),
                reward(sts2_core::RewardRarity::Common, sts2_core::CardKind::Power),
                reward(
                    sts2_core::RewardRarity::Uncommon,
                    sts2_core::CardKind::Attack,
                ),
                reward(
                    sts2_core::RewardRarity::Uncommon,
                    sts2_core::CardKind::Skill,
                ),
                reward(
                    sts2_core::RewardRarity::Uncommon,
                    sts2_core::CardKind::Power,
                ),
                reward(sts2_core::RewardRarity::Rare, sts2_core::CardKind::Attack),
                reward(sts2_core::RewardRarity::Rare, sts2_core::CardKind::Skill),
                reward(sts2_core::RewardRarity::Rare, sts2_core::CardKind::Power),
            ]
        };
    };
}

pub(crate) use lists;
