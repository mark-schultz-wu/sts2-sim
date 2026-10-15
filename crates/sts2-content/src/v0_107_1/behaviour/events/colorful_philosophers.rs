//! Colorful Philosophers: three card rewards from another character's pool.

use nonmax::NonMaxU8;
use sts2_core::rng::{Bound, GameRng};
use sts2_core::{
    CustomReward, EventBehavior, EventView, HookEffect, HookEffects, RewardRarity, Room,
};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::characters::Character;
use super::super::super::generated::events::{Event, EventOption as Said};

/// `CardPoolColorOrder`, each pool by its character and its option.
const COLOR_ORDER: [(Character, Said); 5] = [
    (
        Character::Necrobinder,
        Said::ColorfulPhilosophersNecrobinder,
    ),
    (Character::Ironclad, Said::ColorfulPhilosophersIronclad),
    (Character::Regent, Said::ColorfulPhilosophersRegent),
    (Character::Silent, Said::ColorfulPhilosophersSilent),
    (Character::Defect, Said::ColorfulPhilosophersDefect),
];

/// How many options stay, which is `Mathf.Min(3, list.Count)`.
const KEPT: usize = 3;

/// `CardsVar(3)`.
const CARDS: usize = 3;

/// `OfferRewards` for each pool of `COLOR_ORDER`: a card reward of three at
/// each of Common, Uncommon and Rare.
static REWARDS: [[CustomReward<V>; 3]; 5] = {
    const fn reward(pool: Character) -> [CustomReward<V>; 3] {
        const fn of(pool: Character, rarity: RewardRarity) -> CustomReward<V> {
            CustomReward::CardsOf {
                pool: Some(pool),
                rarity,
                count: CARDS,
                upgrade_roll: true,
                no_pool_modifications: true,
            }
        }
        [
            of(pool, RewardRarity::Common),
            of(pool, RewardRarity::Uncommon),
            of(pool, RewardRarity::Rare),
        ]
    }
    [
        reward(Character::Necrobinder),
        reward(Character::Ironclad),
        reward(Character::Regent),
        reward(Character::Silent),
        reward(Character::Defect),
    ]
};

#[derive(Debug, Clone)]
pub struct State {
    /// The options kept, as places in `COLOR_ORDER`.
    offered: Vec<usize>,
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct ColorfulPhilosophers;

impl EventBehavior<V> for ColorfulPhilosophers {
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
        let mut offered: Vec<usize> = (0..COLOR_ORDER.len())
            .filter(|&at| COLOR_ORDER[at].0 != run.character())
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
        let options: Vec<(Said, bool)> = state
            .offered
            .iter()
            .map(|&at| (COLOR_ORDER[at].1, false))
            .collect();
        super::page(Event::ColorfulPhilosophers, &options, state.chosen)
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
            HookEffect::OfferRewards(&REWARDS[pool]),
            HookEffect::FinishEvent
        ]
    }
}
