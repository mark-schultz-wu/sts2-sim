//! Battleworn Dummy: a fight against a dummy at one of three settings, each
//! its own encounter, and a reward for beating it in time.

use nonmax::NonMaxU8;
use sts2_core::{CustomReward, EventBehavior, EventView, HookEffect, HookEffects, Room, Stream};

use super::super::super::V0_111_0 as V;
use super::super::super::generated::encounters::Encounter;
use super::super::super::generated::events::{Event, EventOption as Said};
use super::super::super::generated::monsters::Monster;

/// Each setting's option, the encounter it enters and the dummy it fights.
const SETTINGS: [(Said, Encounter, &[Monster]); 3] = [
    (
        Said::BattlewornDummySetting1,
        Encounter::BattlewornDummyEventV1Encounter,
        &[Monster::BattleFriendV1],
    ),
    (
        Said::BattlewornDummySetting2,
        Encounter::BattlewornDummyEventV2Encounter,
        &[Monster::BattleFriendV2],
    ),
    (
        Said::BattlewornDummySetting3,
        Encounter::BattlewornDummyEventV3Encounter,
        &[Monster::BattleFriendV3],
    ),
];

/// The first setting's reward: one potion off the rewards stream.
const POTION: &[CustomReward<V>] = &[CustomReward::UnlockedPotion];

/// The third setting's reward: `RelicReward` for the next relic from the
/// front of the bag, at a rolled rarity.
const RELIC: &[CustomReward<V>] = &[CustomReward::RolledRelic];

/// The second setting's upgrades.
const UPGRADES: usize = 2;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken, which names the encounter fought.
    chosen: Option<NonMaxU8>,
}

pub struct BattlewornDummy;

impl EventBehavior<V> for BattlewornDummy {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            Event::BattlewornDummy,
            &SETTINGS.map(|(said, ..)| (said, false)),
            state.chosen,
        )
    }

    /// `EnterCombatWithoutExitingEvent` of the option's encounter, which
    /// comes back to the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let (_, encounter, roster) = SETTINGS[usize::from(option)];
        smallvec::smallvec![HookEffect::FightInEvent {
            encounter,
            rewards: &[],
            roster,
            resume: true,
        }]
    }

    /// `Resume`: finished either way, and a dummy beaten in time pays out by
    /// its encounter. The first offers a potion, the second upgrades two
    /// cards stably shuffled off the event's stream, and the third offers
    /// the next relic. The fight's `ExtraRewards` join the offer, and an
    /// offer of nothing is not made.
    fn resume_from_fight(&self, state: &mut State, escaped: bool) -> HookEffects<V> {
        let mut effects: HookEffects<V> = smallvec::smallvec![HookEffect::FinishEvent];
        if escaped {
            return effects;
        }
        let offered = match state.chosen.map(|chosen| chosen.get()) {
            Some(0) => POTION,
            Some(1) => {
                effects.push(HookEffect::UpgradeRandomDeckCards {
                    count: UPGRADES,
                    stream: Stream::Event,
                    kind: None,
                });
                &[]
            }
            Some(_) => RELIC,
            None => unreachable!("a dummy fought at no setting"),
        };
        effects.push(HookEffect::OfferRewardsWithRoomExtras(offered));
        effects
    }
}
