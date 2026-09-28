//! Battleworn Dummy: a fight against a dummy at one of three settings, and a
//! reward for beating it in time.

use nonmax::NonMaxU8;
use sts2_core::{
    CustomReward, EventBehavior, EventView, HookEffect, HookEffects, RelicPull, Room, Stream,
};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::encounters::Encounter;
use super::super::super::generated::events::{Event, EventOption as Said};
use super::super::super::generated::monsters::Monster;

/// `DummySetting`, by option: the dummy each fights.
const SETTINGS: [(Said, Monster); 3] = [
    (Said::BattlewornDummySetting1, Monster::BattleFriendV1),
    (Said::BattlewornDummySetting2, Monster::BattleFriendV2),
    (Said::BattlewornDummySetting3, Monster::BattleFriendV3),
];

/// `Setting1`'s reward: one potion off the rewards stream.
const POTION: &[CustomReward<V>] = &[CustomReward::UnlockedPotion];

/// `Setting2`'s upgrades.
const UPGRADES: usize = 2;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken, which is the encounter's `Setting`.
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
        super::page(
            Event::BattlewornDummy,
            &SETTINGS.map(|(said, _)| (said, false)),
            state.chosen,
        )
    }

    /// `StartCombat`: the encounter at the option's setting, which comes back
    /// to the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let (_, dummy) = SETTINGS[usize::from(option)];
        let roster: &'static [Monster] = match dummy {
            Monster::BattleFriendV1 => &[Monster::BattleFriendV1],
            Monster::BattleFriendV2 => &[Monster::BattleFriendV2],
            _ => &[Monster::BattleFriendV3],
        };
        smallvec::smallvec![HookEffect::FightInEvent {
            encounter: Encounter::BattlewornDummyEventEncounter,
            reward: None,
            roster,
            resume: true,
        }]
    }

    /// `Resume`: finished either way, and a dummy beaten in time pays out
    /// by its setting. `Setting1` offers a potion. `Setting2` upgrades two
    /// cards stably shuffled off the event's stream. `Setting3` gives the
    /// next relic from the front of the bag.
    fn resume_from_fight(&self, state: &mut State, escaped: bool) -> HookEffects<V> {
        let mut effects: HookEffects<V> = smallvec::smallvec![HookEffect::FinishEvent];
        if escaped {
            return effects;
        }
        effects.push(match state.chosen.map(|chosen| chosen.get()) {
            Some(0) => HookEffect::OfferRewards(POTION),
            Some(1) => HookEffect::UpgradeRandomDeckCards {
                count: UPGRADES,
                stream: Stream::Event,
                kind: None,
            },
            Some(_) => HookEffect::ObtainNextRelic(RelicPull::Rolled),
            None => unreachable!("a dummy fought at no setting"),
        });
        effects
    }
}
