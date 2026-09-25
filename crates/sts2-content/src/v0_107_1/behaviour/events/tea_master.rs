//! Tea Master eligibility, choices, and outcomes.

use sts2_core::{
    EventBehavior, EventContext, EventFlow, EventOption, EventOptionKey, EventView, Room,
};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::{events::Event, relics::Relic};
use super::super::super::{EventOptionId, TeaMasterOption};

#[derive(Debug, Clone, Default)]
pub struct State {
    finished: bool,
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

    fn observe(&self, state: &State, run: &impl EventView<V>) -> Room<V> {
        let options = [
            (TeaMasterOption::BoneTea, 50),
            (TeaMasterOption::EmberTea, 150),
            (TeaMasterOption::TeaOfDiscourtesy, 0),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (option, cost))| EventOption {
            index: index as u8,
            key: EventOptionKey {
                event: Event::TeaMaster,
                option: EventOptionId::TeaMaster(option),
            },
            title: None,
            text: None,
            locked: run.gold() < cost,
            proceed: false,
            chosen: false,
            relic: None,
        })
        .collect();
        Room::Event {
            event: Event::TeaMaster,
            finished: state.finished,
            title: None,
            text: None,
            options: if state.finished { Vec::new() } else { options },
        }
    }

    fn choose<R: sts2_core::rng::GameRng, C: EventContext<V>>(
        &self,
        state: &mut State,
        option: u8,
        run: &mut C,
        _rng: &mut R,
    ) -> EventFlow<V> {
        let (cost, relic) = match option {
            0 => (50, Relic::BoneTea),
            1 => (150, Relic::EmberTea),
            2 => (0, Relic::TeaOfDiscourtesy),
            _ => return EventFlow::Stay,
        };
        if !run.spend_gold(cost) {
            return EventFlow::Stay;
        }
        run.obtain_relic(relic);
        state.finished = true;
        EventFlow::Finished
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

    impl EventContext<V> for Run {
        fn gold(&self) -> i32 {
            self.gold
        }
        fn spend_gold(&mut self, amount: i32) -> bool {
            if self.gold < amount {
                return false;
            }
            self.gold -= amount;
            true
        }
        fn obtain_relic(&mut self, relic: Relic) {
            self.relics.push(relic);
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
        let mut run = Run {
            gold: 175,
            ..Run::default()
        };
        let mut rng = Rng;
        let mut state = TeaMaster.enter(&run, &mut rng);
        assert!(matches!(
            TeaMaster.choose(&mut state, 0, &mut run, &mut rng),
            EventFlow::Finished
        ));
        assert_eq!(run.gold, 125);
        assert_eq!(run.relics, [Relic::BoneTea]);
        let Room::Event {
            finished, options, ..
        } = TeaMaster.observe(&state, &run)
        else {
            panic!("Tea Master is an event room")
        };
        assert!(finished);
        assert!(options.is_empty());
    }
}
