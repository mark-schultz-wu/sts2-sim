//! Trial: a verdict on one of three defendants, each with a curse.

use nonmax::NonMaxU8;
use sts2_core::rng::{Bound, GameRng};
use sts2_core::{
    CustomReward, DeckAct, EventBehavior, EventView, HookEffect, HookEffects, RelicPull, Room,
    Stream,
};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, CurseCard};
use super::super::super::generated::events::{Event, EventOption as Said};

/// `NobleGuilty`'s heal.
const HEAL: i32 = 10;

/// `NobleInnocent`'s gold.
const GOLD: i32 = 300;

/// `NondescriptGuilty`: two card rewards of three, with no upgrade roll.
const REWARDS: &[CustomReward<V>] = &[
    CustomReward::CharacterCards {
        count: 3,
        upgrade_roll: false,
    },
    CustomReward::CharacterCards {
        count: 3,
        upgrade_roll: false,
    },
];

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

impl EventBehavior<V> for Trial {
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
        let options: &[(Said, bool)] = match state.page {
            Page::Initial => &[(Said::TrialAccept, false), (Said::TrialReject, false)],
            Page::Merchant => &[
                (Said::TrialMerchantGuilty, false),
                (Said::TrialMerchantInnocent, false),
            ],
            Page::Noble => &[
                (Said::TrialNobleGuilty, false),
                (Said::TrialNobleInnocent, false),
            ],
            Page::Nondescript => &[
                (Said::TrialNondescriptGuilty, false),
                (Said::TrialNondescriptInnocent, false),
            ],
            Page::Reject => &[
                (Said::TrialRejectAccept, false),
                (Said::TrialRejectDoubleDown, false),
            ],
        };
        let mut room = super::page(Event::Trial, options, state.chosen);
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
        let curse = |curse| HookEffect::AddToDeck(Cards::Curse(curse));
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
                curse(CurseCard::Regret),
                HookEffect::ObtainNextRelic(RelicPull::Rolled),
                HookEffect::ObtainNextRelic(RelicPull::Rolled),
            ],
            // Shame, then two cards upgraded.
            (Page::Merchant, _) => smallvec::smallvec![
                curse(CurseCard::Shame),
                HookEffect::SelectFromDeck {
                    act: DeckAct::Upgrade,
                    count: 2,
                },
            ],
            (Page::Noble, 0) => smallvec::smallvec![HookEffect::Heal(HEAL)],
            (Page::Noble, _) => {
                smallvec::smallvec![curse(CurseCard::Regret), HookEffect::GainGold(GOLD)]
            }
            (Page::Nondescript, 0) => {
                smallvec::smallvec![curse(CurseCard::Doubt), HookEffect::OfferRewards(REWARDS),]
            }
            // Doubt, then two cards transformed off the event's own stream.
            (Page::Nondescript, _) => smallvec::smallvec![
                curse(CurseCard::Doubt),
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
