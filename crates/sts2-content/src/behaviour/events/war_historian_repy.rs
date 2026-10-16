//! War Historian Repy: what a Lantern Key unlocks, a cage or a chest.

use crate::ids::{CardIds as _, EventIds as _, EventOptionIds as _, RelicIds as _};
use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

/// The page the event stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Initial,
    /// After the cage, with a second key: the chest alone.
    SecondChest,
    /// After the chest, with a second key: the cage alone.
    SecondCage,
}

#[derive(Debug, Clone)]
pub struct State {
    page: Page,
    /// The option taken on the page it stands on.
    chosen: Option<NonMaxU8>,
    /// The page the first unlock turns to once it is over.
    next: Option<Page>,
}

pub struct WarHistorianRepy;

/// The deck's Lantern Keys, as places in it, in deck order.
fn keys<V: crate::Build>(run: &impl EventView<V>) -> Vec<usize> {
    run.deck()
        .iter()
        .enumerate()
        .filter(|(_, card)| card.card == V::Card::LANTERN_KEY)
        .map(|(at, _)| at)
        .collect()
}

impl<V: crate::Build> EventBehavior<V> for WarHistorianRepy {
    type State = State;

    /// A Lantern Key's own room is the only way in.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        false
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State {
            page: Page::Initial,
            chosen: None,
            next: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let options: &[(V::EventOption, bool)] = match state.page {
            Page::Initial => &[
                (V::EventOption::WAR_HISTORIAN_REPY_UNLOCK_CAGE, false),
                (V::EventOption::WAR_HISTORIAN_REPY_UNLOCK_CHEST, false),
            ],
            Page::SecondChest => &[(V::EventOption::WAR_HISTORIAN_REPY_UNLOCK_CHEST, false)],
            Page::SecondCage => &[(V::EventOption::WAR_HISTORIAN_REPY_UNLOCK_CAGE, false)],
        };
        crate::behaviour::events::page::<V>(V::Event::WAR_HISTORIAN_REPY, options, state.chosen)
    }

    /// The first choice spends the first Lantern Key, and a run of one player
    /// with a key left goes on to the other unlock. The second spends every
    /// key left. `PlayerCmd.CompleteQuest` writes to the map history alone.
    ///
    /// `UnlockCage`: History Course. `UnlockChest`: `OfferCustom` of its
    /// rewards. The second choice finishes the event before it unlocks.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        let cage = match state.page {
            Page::Initial => option == 0,
            Page::SecondChest => false,
            Page::SecondCage => true,
        };
        let unlock = if cage {
            HookEffect::ObtainRelic(V::Relic::HISTORY_COURSE)
        } else {
            HookEffect::OfferRewards(V::WAR_HISTORIAN_REPY_CHEST)
        };
        let mut effects = HookEffects::new();
        if state.page == Page::Initial {
            let keys = keys::<V>(run);
            effects.extend(keys.first().copied().map(HookEffect::RemoveDeckCardAt));
            effects.push(unlock);
            state.chosen = NonMaxU8::new(option);
            if keys.len() > 1 {
                state.next = Some(if cage {
                    Page::SecondChest
                } else {
                    Page::SecondCage
                });
                effects.push(HookEffect::ResumeEvent);
            } else {
                effects.push(HookEffect::FinishEvent);
            }
        } else {
            state.chosen = NonMaxU8::new(option);
            effects.push(HookEffect::FinishEvent);
            // From the back, so each place still names its key.
            effects.extend(
                keys::<V>(run)
                    .into_iter()
                    .rev()
                    .map(HookEffect::RemoveDeckCardAt),
            );
            effects.push(unlock);
        }
        effects
    }

    /// `SetEventState` of the page for the other unlock, once the first is
    /// over.
    fn resume<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.page = state
            .next
            .take()
            .expect("only a first unlock with a key left resumes");
        state.chosen = None;
        HookEffects::new()
    }
}

/// `CHEST`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `UnlockChest`: two potions and two relics.
        const WAR_HISTORIAN_REPY_CHEST: &'static [sts2_core::CustomReward<$build>] = &[
            sts2_core::CustomReward::Potion,
            sts2_core::CustomReward::Potion,
            sts2_core::CustomReward::RolledRelic,
            sts2_core::CustomReward::RolledRelic,
        ];
    };
}

pub(crate) use lists;
