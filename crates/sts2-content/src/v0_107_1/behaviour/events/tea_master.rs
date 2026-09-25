//! Tea Master eligibility, choices, and outcomes.

use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::events::{Event, EventOption as Said};
use super::super::super::generated::relics::Relic;

#[derive(Debug, Clone, Default)]
pub struct State {
    /// The option taken.
    chosen: Option<u8>,
}

pub struct TeaMaster;

impl EventBehavior<V> for TeaMaster {
    type State = State;

    fn allowed(&self, run: &impl EventView<V>) -> bool {
        run.act() < 2 && run.gold() >= 150
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State::default()
    }

    /// A tea the purse cannot pay for is locked.
    fn observe(&self, state: &State, run: &impl EventView<V>) -> Room<V> {
        super::page(
            Event::TeaMaster,
            &[
                (Said::TeaMasterBoneTea, run.gold() < 50),
                (Said::TeaMasterEmberTea, run.gold() < 150),
                (Said::TeaMasterTeaOfDiscourtesy, false),
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
        state.chosen = Some(option);
        match option {
            0 => smallvec::smallvec![
                HookEffect::LoseGold(50),
                HookEffect::ObtainRelic(Relic::BoneTea),
                HookEffect::FinishEvent,
            ],
            1 => smallvec::smallvec![
                HookEffect::LoseGold(150),
                HookEffect::ObtainRelic(Relic::EmberTea),
                HookEffect::FinishEvent,
            ],
            _ => smallvec::smallvec![
                HookEffect::ObtainRelic(Relic::TeaOfDiscourtesy),
                HookEffect::FinishEvent,
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_core::rng::GameRng;
    use sts2_core::{CardIdent, EventView};

    #[derive(Default)]
    struct Run {
        act: u32,
        gold: i32,
        relics: Vec<Relic>,
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
        fn hp(&self) -> i32 {
            unreachable!("Tea Master does not inspect HP")
        }
        fn max_hp(&self) -> i32 {
            unreachable!("Tea Master does not inspect HP")
        }
        fn potions(&self) -> impl Iterator<Item = <V as sts2_core::ContentVersion>::Potion> + '_ {
            std::iter::empty()
        }
        fn character(&self) -> <V as sts2_core::ContentVersion>::Character {
            unreachable!("Tea Master does not inspect the character")
        }
        fn relics(&self) -> impl Iterator<Item = Relic> + '_ {
            self.relics.iter().copied()
        }
        fn deck(&self) -> &[CardIdent<V>] {
            &[]
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
                HookEffect::ObtainRelic(Relic::BoneTea),
                HookEffect::FinishEvent,
            ]
        );
    }
}
