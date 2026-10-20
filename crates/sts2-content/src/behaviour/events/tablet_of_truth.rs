//! Tablet of Truth: Max HP for upgrades, five times at most, or a heal.

use crate::ids::{EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room, Stream};

/// `DynamicVar("SmashHPGain", 20)`.
const SMASH_HEAL: i32 = 20;

/// `GetDecipherCost` for the deciphers before the last, the first being
/// `DynamicVar("DecipherMaxHpLoss", 3)`.
const DECIPHER_COSTS: [i32; 4] = [3, 6, 12, 24];

/// `DecipherCount` once the event finishes.
const DECIPHERS: u8 = 5;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// `DecipherCount`, which is the page the event stands on.
    deciphered: u8,
    /// The option taken on the page it stands on.
    chosen: Option<NonMaxU8>,
}

pub struct TabletOfTruth;

impl<V: crate::Build> EventBehavior<V> for TabletOfTruth {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let decipher = match state.deciphered {
            0 => V::EventOption::TABLET_OF_TRUTH_DECIPHER1,
            1 => V::EventOption::TABLET_OF_TRUTH_DECIPHER1_DECIPHER,
            2 => V::EventOption::TABLET_OF_TRUTH_DECIPHER2_DECIPHER,
            3 => V::EventOption::TABLET_OF_TRUTH_DECIPHER3_DECIPHER,
            _ => V::EventOption::TABLET_OF_TRUTH_DECIPHER4_DECIPHER,
        };
        let other = match state.deciphered {
            0 => V::EventOption::TABLET_OF_TRUTH_SMASH,
            _ => V::EventOption::TABLET_OF_TRUTH_DECIPHER_GIVE_UP,
        };
        crate::behaviour::events::page::<V>(
            V::Event::TABLET_OF_TRUTH,
            &[(decipher, false), (other, false)],
            state.chosen,
        )
    }

    /// `Decipher`: `CreatureCmd.LoseMaxHp` by its cost, then one upgradable
    /// deck card upgraded, drawn off the event's stream. The fifth upgrades
    /// every one, costs all but one Max HP, and finishes the event. The rest
    /// turn to the next page. `Smash` heals, and `GiveUp` leaves.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        if option != 0 {
            state.chosen = NonMaxU8::new(option);
            return match state.deciphered {
                0 => smallvec::smallvec![HookEffect::Heal(SMASH_HEAL), HookEffect::FinishEvent],
                _ => smallvec::smallvec![HookEffect::FinishEvent],
            };
        }
        let last = state.deciphered + 1 == DECIPHERS;
        let cost = match DECIPHER_COSTS.get(usize::from(state.deciphered)) {
            Some(&cost) => cost,
            None => run.max_hp() - 1,
        };
        // `LoseMaxHpAndUpgrade` kills a player whose maximum the cost reaches.
        if cost >= run.max_hp() {
            unimplemented!("a player killed by Tablet of Truth");
        }
        let upgrade = match last {
            true => HookEffect::UpgradeDeck,
            false => HookEffect::UpgradeRandomDeckCard {
                stream: Stream::Event,
            },
        };
        state.deciphered += 1;
        let mut effects: HookEffects<V> = smallvec::smallvec![HookEffect::LoseMaxHp(cost), upgrade];
        if last {
            effects.push(HookEffect::FinishEvent);
        }
        effects
    }
}
