//! Tea Master eligibility, choices, and outcomes.

use crate::ids::{EventIds as _, EventOptionIds as _, RelicIds as _};
use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct TeaMaster;

impl<V: crate::Build> EventBehavior<V> for TeaMaster {
    type State = State;

    fn allowed(&self, run: &impl EventView<V>) -> bool {
        run.act() < 2 && run.gold() >= 150
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    /// A tea the purse cannot pay for is locked.
    fn observe(&self, state: &State, run: &impl EventView<V>) -> Room<V> {
        crate::behaviour::events::page::<V>(
            V::Event::TEA_MASTER,
            &[
                (V::EventOption::TEA_MASTER_BONE_TEA, run.gold() < 50),
                (V::EventOption::TEA_MASTER_EMBER_TEA, run.gold() < 150),
                (V::EventOption::TEA_MASTER_TEA_OF_DISCOURTESY, false),
            ],
            state.chosen,
        )
    }

    /// `PlayerCmd.LoseGold` for the tea, where it costs any, then
    /// `RelicCmd.Obtain`, which finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        match option {
            0 => smallvec::smallvec![
                HookEffect::LoseGold(50),
                HookEffect::ObtainRelic(V::Relic::BONE_TEA),
                HookEffect::FinishEvent,
            ],
            1 => smallvec::smallvec![
                HookEffect::LoseGold(150),
                HookEffect::ObtainRelic(V::Relic::EMBER_TEA),
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::ObtainRelic(V::Relic::TEA_OF_DISCOURTESY),
                HookEffect::FinishEvent,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    type V = crate::v0_107_1::V0_107_1;
    use crate::behaviour::events::tea_master::*;
    use sts2_core::rng::GameRng;
    use sts2_core::{CardIdent, EventView};

    #[derive(Default)]
    struct Run {
        act: u32,
        gold: i32,
        relics: Vec<<V as sts2_core::ContentVersion>::Relic>,
    }

    impl EventView<V> for Run {
        fn act(&self) -> u32 {
            self.act
        }
        fn gold(&self) -> i32 {
            self.gold
        }
        fn floor(&self) -> u32 {
            unreachable!("Tea Master does not inspect the floor")
        }
        fn wins(&self) -> u32 {
            unreachable!("Tea Master does not inspect the profile")
        }
        fn local(&self) -> bool {
            unreachable!("Tea Master does not ask whose machine it is on")
        }
        fn room(&self) -> Option<<V as sts2_core::ContentVersion>::RoomType> {
            unreachable!("Tea Master does not inspect the room")
        }
        fn modifiers(&self) -> &[sts2_core::RunModifier<V>] {
            unreachable!("Tea Master does not read the run's modifiers")
        }
        fn hp(&self) -> i32 {
            unreachable!("Tea Master does not inspect HP")
        }
        fn max_hp(&self) -> i32 {
            unreachable!("Tea Master does not inspect HP")
        }
        fn potions(&self) -> impl Iterator<Item = <V as sts2_core::ContentVersion>::Potion> + '_ {
            std::iter::empty()
        }
        fn open_potion_slot(&self) -> bool {
            true
        }
        fn character(&self) -> <V as sts2_core::ContentVersion>::Character {
            unreachable!("Tea Master does not inspect the character")
        }
        fn multiplayer(&self) -> bool {
            unreachable!("Tea Master does not ask how many players there are")
        }
        fn relics(&self) -> impl Iterator<Item = sts2_core::CarriedRelic<V>> + '_ {
            self.relics.iter().map(|&relic| sts2_core::CarriedRelic {
                relic,
                used_up: false,
                melted: false,
            })
        }
        fn deck(&self) -> Vec<CardIdent<V>> {
            Vec::new()
        }
        fn relics_left(&self) -> bool {
            unreachable!("Tea Master does not inspect the relic bag")
        }
    }

    struct Rng;

    impl GameRng for Rng {
        type Int = i32;
        fn new(_: u64) -> Self {
            Self
        }
        fn counter(&self) -> i32 {
            0
        }
        fn next_u64(&mut self) -> u64 {
            unreachable!("Tea Master draws no random numbers")
        }
    }

    #[test]
    fn tea_master_requires_enough_gold_in_the_first_two_acts() {
        assert!(!TeaMaster.allowed(&Run {
            act: 0,
            gold: 149,
            ..Run::default()
        }));
        assert!(TeaMaster.allowed(&Run {
            act: 0,
            gold: 150,
            ..Run::default()
        }));
        assert!(!TeaMaster.allowed(&Run {
            act: 2,
            gold: 150,
            ..Run::default()
        }));
    }

    #[test]
    fn buying_bone_tea_finishes_the_event() {
        let run = Run {
            gold: 175,
            ..Run::default()
        };
        let mut rng = Rng;
        let mut state = TeaMaster.enter(&run, &mut rng);
        assert_eq!(
            TeaMaster.choose(&mut state, 0, &run, &mut rng).as_slice(),
            [
                HookEffect::LoseGold(50),
                HookEffect::ObtainRelic(<V as sts2_core::ContentVersion>::Relic::BONE_TEA),
                HookEffect::FinishEvent,
            ]
        );
    }
}
