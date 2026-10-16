//! Trial: a verdict on one of three defendants, each with a curse.

use crate::ids::{CardIds as _, EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::rng::{Bound, GameRng};
use sts2_core::{
    DeckAct, EventBehavior, EventView, HookEffect, HookEffects, RelicPull, Room, Stream,
};

/// `NobleGuilty`'s heal.
const HEAL: i32 = 10;

/// `NobleInnocent`'s gold.
const GOLD: i32 = 300;

/// The page the event stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Initial,
    Merchant,
    Noble,
    Nondescript,
    Reject,
}

#[derive(Debug, Clone)]
pub struct State {
    page: Page,
    /// The option taken on the page it stands on.
    chosen: Option<NonMaxU8>,
}

pub struct Trial;

impl<V: crate::Build> EventBehavior<V> for Trial {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State {
            page: Page::Initial,
            chosen: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let options: &[(V::EventOption, bool)] = match state.page {
            Page::Initial => &[
                (V::EventOption::TRIAL_ACCEPT, false),
                (V::EventOption::TRIAL_REJECT, false),
            ],
            Page::Merchant => &[
                (V::EventOption::TRIAL_MERCHANT_GUILTY, false),
                (V::EventOption::TRIAL_MERCHANT_INNOCENT, false),
            ],
            Page::Noble => &[
                (V::EventOption::TRIAL_NOBLE_GUILTY, false),
                (V::EventOption::TRIAL_NOBLE_INNOCENT, false),
            ],
            Page::Nondescript => &[
                (V::EventOption::TRIAL_NONDESCRIPT_GUILTY, false),
                (V::EventOption::TRIAL_NONDESCRIPT_INNOCENT, false),
            ],
            Page::Reject => &[
                (V::EventOption::TRIAL_REJECT_ACCEPT, false),
                (V::EventOption::TRIAL_REJECT_DOUBLE_DOWN, false),
            ],
        };
        let mut room = crate::behaviour::events::page::<V>(V::Event::TRIAL, options, state.chosen);
        // `DoubleDown` is written as a proceed option.
        if let (Page::Reject, Room::Event { options, .. }) = (state.page, &mut room) {
            options[1].proceed = true;
        }
        room
    }

    /// `Accept`: one `Rng.NextInt(3)` for the defendant's page. `Reject`:
    /// a page that accepts after all. Each verdict finishes the event.
    fn choose<R: GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        rng: &mut R,
    ) -> HookEffects<V> {
        let curse = |curse| HookEffect::AddToDeck(curse);
        let mut verdict: HookEffects<V> = match (state.page, option) {
            (Page::Initial | Page::Reject, 0) => {
                let three: R::Int = num_traits::cast(3).expect("a small bound");
                let drawn: u8 = num_traits::cast(
                    rng.next_int(Bound::try_new(three).expect("a positive bound")),
                )
                .expect("a draw below three");
                state.page = match drawn {
                    0 => Page::Merchant,
                    1 => Page::Noble,
                    _ => Page::Nondescript,
                };
                state.chosen = None;
                return smallvec::smallvec![];
            }
            (Page::Initial, _) => {
                state.page = Page::Reject;
                state.chosen = None;
                return smallvec::smallvec![];
            }
            // `DoubleDown`: the abandon-run popup, which the run asks. The
            // option shows chosen whichever way the popup is answered.
            (Page::Reject, _) => {
                state.chosen = NonMaxU8::new(option);
                return smallvec::smallvec![HookEffect::ConfirmAbandonRun];
            }
            // Regret, then two relics from the front of the bag.
            (Page::Merchant, 0) => smallvec::smallvec![
                curse(V::Card::REGRET),
                HookEffect::ObtainNextRelic(RelicPull::Rolled),
                HookEffect::ObtainNextRelic(RelicPull::Rolled),
            ],
            // Shame, then two cards upgraded.
            (Page::Merchant, _) => smallvec::smallvec![
                curse(V::Card::SHAME),
                HookEffect::SelectFromDeck {
                    act: DeckAct::Upgrade,
                    count: 2,
                },
            ],
            (Page::Noble, 0) => smallvec::smallvec![HookEffect::Heal(HEAL)],
            (Page::Noble, _) => {
                smallvec::smallvec![curse(V::Card::REGRET), HookEffect::GainGold(GOLD)]
            }
            (Page::Nondescript, 0) => {
                smallvec::smallvec![
                    curse(V::Card::DOUBT),
                    HookEffect::OfferRewards(V::TRIAL_REWARDS),
                ]
            }
            // Doubt, then two cards transformed off the event's own stream.
            (Page::Nondescript, _) => smallvec::smallvec![
                curse(V::Card::DOUBT),
                HookEffect::SelectFromDeck {
                    act: DeckAct::Transform(Stream::Event),
                    count: 2,
                },
            ],
        };
        state.chosen = NonMaxU8::new(option);
        verdict.push(HookEffect::FinishEvent);
        verdict
    }
}

/// `REWARDS`, written for one build's own types.
macro_rules! lists {
    ($build:ty) => {
        /// `NondescriptGuilty`: two card rewards of three, with no upgrade roll.
        const TRIAL_REWARDS: &'static [sts2_core::CustomReward<$build>] = &[
            sts2_core::CustomReward::CharacterCards {
                count: 3,
                upgrade_roll: false,
            },
            sts2_core::CustomReward::CharacterCards {
                count: 3,
                upgrade_roll: false,
            },
        ];
    };
}

pub(crate) use lists;
