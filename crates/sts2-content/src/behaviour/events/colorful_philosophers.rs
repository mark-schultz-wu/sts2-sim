//! Colorful Philosophers: three card rewards from another character's pool.

use crate::ids::EventIds as _;
use nonmax::NonMaxU8;
use sts2_core::rng::{Bound, GameRng};
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

/// How many options stay, which is `Mathf.Min(3, list.Count)`.
const KEPT: usize = 3;

/// `CardsVar(3)`.
pub(crate) const CARDS: usize = 3;

#[derive(Debug, Clone)]
pub struct State {
    /// The options kept, as places in `COLOR_ORDER`.
    offered: Vec<usize>,
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct ColorfulPhilosophers;

impl<V: crate::Build> EventBehavior<V> for ColorfulPhilosophers {
    type State = State;

    /// A run has every character's card pool unlocked, which the shared
    /// eligibility check answers.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `GenerateInitialOptions`: every pool but the character's own, in
    /// `CardPoolColorOrder`, then one `NextInt` removal at a time until three
    /// stay.
    fn enter<R: GameRng>(&self, run: &impl EventView<V>, rng: &mut R) -> State {
        let mut offered: Vec<usize> = (0..V::COLORFUL_PHILOSOPHERS_COLOR_ORDER.len())
            .filter(|&at| V::COLORFUL_PHILOSOPHERS_COLOR_ORDER[at].0 != run.character())
            .collect();
        while offered.len() > KEPT {
            let count: R::Int = num_traits::cast(offered.len()).expect("a few options");
            let at: usize =
                num_traits::cast(rng.next_int(Bound::try_new(count).expect("an option to remove")))
                    .expect("a draw below the count is an index");
            offered.remove(at);
        }
        State {
            offered,
            chosen: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let options: Vec<(V::EventOption, bool)> = state
            .offered
            .iter()
            .map(|&at| (V::COLORFUL_PHILOSOPHERS_COLOR_ORDER[at].1, false))
            .collect();
        crate::behaviour::events::page::<V>(V::Event::COLORFUL_PHILOSOPHERS, &options, state.chosen)
    }

    /// `OfferRewards` for the pool the option names, which finishes the
    /// event.
    fn choose<R: GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let pool = state.offered[usize::from(option)];
        smallvec::smallvec![
            HookEffect::OfferRewards(&V::COLORFUL_PHILOSOPHERS_REWARDS[pool]),
            HookEffect::FinishEvent
        ]
    }
}

/// `COLOR_ORDER`, `REWARDS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `CardPoolColorOrder`, each pool by its character and its option.
        const COLORFUL_PHILOSOPHERS_COLOR_ORDER: &'static [(<$build as sts2_core::ContentVersion>::Character, <$build as sts2_core::ContentVersion>::EventOption)] = &[
    (
        <$build as sts2_core::ContentVersion>::Character::NECROBINDER,
        <$build as sts2_core::ContentVersion>::EventOption::COLORFUL_PHILOSOPHERS_NECROBINDER,
    ),
    (<$build as sts2_core::ContentVersion>::Character::IRONCLAD, <$build as sts2_core::ContentVersion>::EventOption::COLORFUL_PHILOSOPHERS_IRONCLAD),
    (<$build as sts2_core::ContentVersion>::Character::REGENT, <$build as sts2_core::ContentVersion>::EventOption::COLORFUL_PHILOSOPHERS_REGENT),
    (<$build as sts2_core::ContentVersion>::Character::SILENT, <$build as sts2_core::ContentVersion>::EventOption::COLORFUL_PHILOSOPHERS_SILENT),
    (<$build as sts2_core::ContentVersion>::Character::DEFECT, <$build as sts2_core::ContentVersion>::EventOption::COLORFUL_PHILOSOPHERS_DEFECT),
];
        /// `OfferRewards` for each pool of `COLOR_ORDER`: a card reward of three at
        /// each of Common, Uncommon and Rare.
        const COLORFUL_PHILOSOPHERS_REWARDS: &'static [[sts2_core::CustomReward<$build>; 3]] = &{
    const fn reward(pool: <$build as sts2_core::ContentVersion>::Character) -> [sts2_core::CustomReward<$build>; 3] {
        const fn of(pool: <$build as sts2_core::ContentVersion>::Character, rarity: sts2_core::RewardRarity) -> sts2_core::CustomReward<$build> {
            sts2_core::CustomReward::CardsOf {
                pool: Some(pool),
                rarity,
                count: crate::behaviour::events::colorful_philosophers::CARDS,
                upgrade_roll: true,
                no_pool_modifications: true,
            }
        }
        [
            of(pool, sts2_core::RewardRarity::Common),
            of(pool, sts2_core::RewardRarity::Uncommon),
            of(pool, sts2_core::RewardRarity::Rare),
        ]
    }
    [
        reward(<$build as sts2_core::ContentVersion>::Character::NECROBINDER),
        reward(<$build as sts2_core::ContentVersion>::Character::IRONCLAD),
        reward(<$build as sts2_core::ContentVersion>::Character::REGENT),
        reward(<$build as sts2_core::ContentVersion>::Character::SILENT),
        reward(<$build as sts2_core::ContentVersion>::Character::DEFECT),
    ]
};
    };
}

pub(crate) use lists;
