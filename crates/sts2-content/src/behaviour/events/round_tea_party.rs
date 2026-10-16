//! Round Tea Party: Royal Poison and a full heal, or a fight for a relic.

use crate::ids::{EventIds as _, EventOptionIds as _, RelicIds as _};
use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

/// `DamageVar(11, Unblockable | Unpowered)`.
const DAMAGE: i32 = 11;

/// `IsAllowed`: every player stands at this much health or more.
const ALLOWED_FROM: i32 = 12;

/// The page the event stands on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Page {
    Initial,
    /// After picking the fight, which offers to go on with it alone.
    PickFight,
}

#[derive(Debug, Clone)]
pub struct State {
    page: Page,
    /// The option taken on the page it stands on.
    chosen: Option<NonMaxU8>,
}

pub struct RoundTeaParty;

impl<V: crate::Build> EventBehavior<V> for RoundTeaParty {
    type State = State;

    fn allowed(&self, run: &impl EventView<V>) -> bool {
        run.hp() >= ALLOWED_FROM
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State {
            page: Page::Initial,
            chosen: None,
        }
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let options: &[(V::EventOption, bool)] = match state.page {
            Page::Initial => &[
                (V::EventOption::ROUND_TEA_PARTY_ENJOY_TEA, false),
                (V::EventOption::ROUND_TEA_PARTY_PICK_FIGHT, false),
            ],
            Page::PickFight => &[(
                V::EventOption::ROUND_TEA_PARTY_PICK_FIGHT_CONTINUE_FIGHT,
                false,
            )],
        };
        crate::behaviour::events::page::<V>(V::Event::ROUND_TEA_PARTY, options, state.chosen)
    }

    /// `EnjoyTea`: Royal Poison, then a heal to full health, which finishes
    /// the event. `PickFight`: the page that goes on with it.
    /// `ContinueFight`: the damage, then the next relic from the front of the
    /// bag, which finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        match (state.page, option) {
            (Page::Initial, 0) => {
                state.chosen = NonMaxU8::new(option);
                let missing = run.max_hp() - run.hp();
                smallvec::smallvec![
                    HookEffect::ObtainRelic(V::Relic::ROYAL_POISON),
                    HookEffect::Heal(missing),
                    HookEffect::FinishEvent,
                ]
            }
            (Page::Initial, _) => {
                state.page = Page::PickFight;
                state.chosen = None;
                smallvec::smallvec![]
            }
            (Page::PickFight, _) => {
                state.chosen = NonMaxU8::new(option);
                smallvec::smallvec![
                    HookEffect::DamageOwner(DAMAGE),
                    HookEffect::ObtainNextRelic(sts2_core::RelicPull::Rolled),
                    HookEffect::FinishEvent,
                ]
            }
        }
    }
}
