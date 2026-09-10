use std::collections::HashMap;
use std::sync::Arc;

use smallvec::SmallVec;
use sts2_core::ActMap as _;
use sts2_core::rng::{GameRng, MegaRng, stream_seed};
use sts2_core::{
    ActTable, Answer, Applied, CardIdent, CardOption, Carried, Coord, Counters, EventOption, Map,
    Observation, Question, Reachable, Room, Rules, Seed, Table,
};

use crate::engine::Error;
use crate::event::Blessing;
use crate::map::{ActMap, Layout};
use crate::run::Opening;

/// The authoritative state of the run slice implemented so far.
///
/// Reports are built from this state. They are never stored and edited as if
/// they were the run itself.
#[derive(Debug, Clone)]
pub(crate) struct State<V: Rules> {
    shared: Arc<RunData<V>>,
    at: Coord,
    player: Player<V>,
    rngs: Rngs,
    phase: Phase<V>,
}

/// Data generated once and shared by every branch of this run.
///
/// Data generated once and shared by every branch of this run.
///
/// The act's map settles when it is generated. It does not change while the
/// act is played, so every branch and every report reads one copy.
#[derive(Debug)]
struct RunData<V: Rules> {
    _opening: Opening<V>,
    map: ActMap<V>,
}

/// The small, mutable part of one player's run state.
#[derive(Debug, Clone)]
struct Player<V: Rules> {
    relics: SmallVec<[V::Relic; 8]>,
    deck: Vec<CardIdent<V>>,
    hp: V::Number,
    gold: V::Number,
    potion_slots: V::Count,
}

/// Mutable random streams and their compact counters.
#[derive(Debug, Clone)]
struct Rngs {
    run: SmallVec<[u32; 16]>,
    player: SmallVec<[u32; 4]>,
    rewards: MegaRng,
}

/// Exactly one room or pending action can own the run at a time.
#[derive(Debug, Clone)]
enum Phase<V: Rules> {
    Neow {
        options: SmallVec<[V::Relic; 4]>,
        chosen: Option<u8>,
        finished: bool,
        selection: Option<SmallVec<[V::Card; 4]>>,
    },
}

impl<V: Rules> State<V> {
    pub(crate) fn start(
        seed: &Seed,
        character: V::Character,
        ascension: V::AscensionLevel,
    ) -> Self {
        let raw_seed = stream_seed(seed);
        let acts: Vec<V::Act> = V::Act::ALL
            .iter()
            .copied()
            .filter(|act| act.played())
            .collect();
        let mut up_front = MegaRng::named(raw_seed, "up_front");
        let opening = Opening::draw(&mut up_front, character, &acts);

        let mut map_rng = MegaRng::named(raw_seed, "act_1_map");
        let map = ActMap::<V>::generate(&mut map_rng, Layout::ACT_ONE, ascension)
            .expect("the game's act-one layout fits a packed map path");
        let at = map
            .ancient()
            .expect("a generated act map opens somewhere")
            .at;

        let mut neow_rng: MegaRng = Blessing::<V>::stream(raw_seed, 0);
        let neow_options = Blessing::<V>::roll(&mut neow_rng, 1).options;

        let mut run_counters: SmallVec<[u32; 16]> =
            smallvec::smallvec![0; V::run_rng_types().len()];
        let up_front_at = V::run_rng_types()
            .iter()
            .position(|stream| *stream == V::up_front_rng())
            .expect("up-front is a run RNG stream");
        run_counters[up_front_at] = up_front.counter() as u32;

        Self {
            shared: Arc::new(RunData {
                _opening: opening,
                map,
            }),
            at,
            player: Player {
                relics: V::starting_relics(character).into_iter().collect(),
                deck: V::starting_deck(character)
                    .into_iter()
                    .map(CardIdent::new)
                    .collect(),
                hp: num_traits::cast(V::starting_hp(character))
                    .expect("starting HP fits the build's game-number type"),
                gold: num_traits::cast(V::starting_gold(character))
                    .expect("starting gold fits the build's game-number type"),
                potion_slots: num_traits::cast(V::starting_potion_slots())
                    .expect("starting potion slots fit the build's count type"),
            },
            rngs: Rngs {
                run: run_counters,
                player: smallvec::smallvec![0; V::player_rng_types().len()],
                rewards: MegaRng::named(raw_seed, "rewards"),
            },
            phase: Phase::Neow {
                options: neow_options.into_iter().collect(),
                chosen: None,
                finished: false,
                selection: None,
            },
        }
    }

    pub(crate) fn choose(&mut self, index: u8) -> Result<Applied<V>, Error> {
        let Phase::Neow {
            options,
            chosen,
            selection,
            ..
        } = &mut self.phase;
        let option = options
            .get(index as usize)
            .ok_or(Error::NotImplemented("choose_option"))?;
        if *option != V::lead_paperweight() {
            return Err(Error::NotImplemented("this Neow option"));
        }

        *chosen = Some(index);
        self.player.relics.push(V::lead_paperweight());
        *selection = Some(
            crate::reward::card::regular::<V, _>(&mut self.rngs.rewards, V::colorless_cards(), 2)
                .into_iter()
                .collect(),
        );
        let mut applied = self.report();
        applied.resolved = false;
        Ok(applied)
    }

    pub(crate) fn answer(&mut self, answer: &Answer) -> Result<Applied<V>, Error> {
        let Phase::Neow {
            finished,
            selection,
            ..
        } = &mut self.phase;
        let cards = selection
            .as_ref()
            .ok_or(Error::NotImplemented("this answer"))?;
        let Answer::Cards(indices) = answer else {
            return Err(Error::NotImplemented("this answer"));
        };
        if indices.len() > 1 {
            return Err(Error::NotImplemented("an invalid card selection"));
        }
        let chosen: Option<Vec<_>> = indices
            .iter()
            .map(|&index| cards.get(index).copied().map(CardIdent::new))
            .collect();
        let Some(chosen) = chosen else {
            return Err(Error::NotImplemented("an invalid card selection"));
        };

        self.player.deck.extend(chosen);
        *selection = None;
        *finished = true;
        Ok(self.report())
    }

    /// Derive the comparison snapshot from the run's authoritative state.
    pub(crate) fn report(&self) -> Applied<V> {
        let map = &self.shared.map;
        let travelable = map
            .at(self.at)
            .expect("the run stands on its own map")
            .to
            .iter()
            .map(|&at| Reachable {
                at,
                kind: map.at(at).expect("a map edge leads to a node").kind,
            })
            .collect();
        let Phase::Neow {
            options: neow_options,
            chosen: neow_chosen,
            finished: neow_finished,
            selection: card_selection,
        } = &self.phase;
        let options = neow_options
            .iter()
            .copied()
            .enumerate()
            .map(|(index, relic)| {
                let value = serde_json::to_value(relic).expect("a relic id serializes");
                let id = value.as_str().expect("a relic id is a string");
                EventOption {
                    index: index as u8,
                    key: format!("NEOW.pages.INITIAL.options.{id}"),
                    title: None,
                    text: None,
                    locked: false,
                    proceed: false,
                    chosen: *neow_chosen == Some(index as u8),
                    relic: Some(relic),
                }
            })
            .collect();
        let asking = card_selection.as_ref().map(|cards| Question::SelectCards {
            min: 0,
            max: 1,
            asker: None,
            options: cards
                .iter()
                .copied()
                .enumerate()
                .map(|(index, card)| CardOption {
                    index,
                    card: CardIdent::new(card),
                    text: None,
                })
                .collect(),
        });
        let run: HashMap<_, _> = V::run_rng_types()
            .iter()
            .copied()
            .zip(self.rngs.run.iter().copied())
            .collect();
        let mut player: HashMap<_, _> = V::player_rng_types()
            .iter()
            .copied()
            .zip(self.rngs.player.iter().copied())
            .collect();
        player.insert(V::rewards_rng(), self.rngs.rewards.counter() as u32);

        Applied {
            observation: Observation {
                act: num_traits::zero(),
                floor: num_traits::one(),
                hp: self.player.hp,
                max_hp: self.player.hp,
                alive: true,
                run_over: false,
                victory: None,
                gold: self.player.gold,
                relics: self
                    .player
                    .relics
                    .iter()
                    .copied()
                    .map(|relic| Carried {
                        relic,
                        title: None,
                        text: None,
                    })
                    .collect(),
                potions: Vec::new(),
                potion_slots: self.player.potion_slots,
                deck: self.player.deck.clone(),
                in_combat: false,
                map: Map {
                    at: Some(self.at),
                    visited: vec![self.at],
                    travelable,
                    points: map.shared(),
                    proceed: false,
                },
                rng: Counters { run, player },
                room: Some(Room::Event {
                    event: V::neow_event(),
                    finished: *neow_finished,
                    title: None,
                    text: None,
                    options: if *neow_finished { Vec::new() } else { options },
                }),
                combat: None,
            },
            asking,
            resolved: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_content::v0_107_1::ascension::AscensionLevel;
    use sts2_content::v0_107_1::characters::Character;
    use sts2_core::Seed;

    use super::*;

    type V = V0_107_1;

    fn opened() -> State<V> {
        State::start(
            &Seed::new("0RACLE1").expect("the fixture's seed is a seed"),
            Character::Regent,
            AscensionLevel::None,
        )
    }

    /// A branch has to be able to cross a thread, so that searching a run in
    /// parallel is a matter of handing the branches out.
    ///
    /// Nothing needs it yet. One `Rc` or one `RefCell` on a field loses it,
    /// and this fails on that commit rather than years later.
    #[test]
    fn a_branch_can_cross_a_thread() {
        fn assert<T: Send + Sync>() {}
        assert::<State<V>>();
        assert::<crate::Sim<V>>();
    }

    #[test]
    fn branching_a_run_shares_the_setup_it_drew() {
        let run = opened();
        let branch = run.clone();
        assert!(Arc::ptr_eq(&run.shared, &branch.shared));
    }

    #[test]
    fn a_branch_keeps_its_own_play() {
        let run = opened();
        let mut branch = run.clone();
        branch
            .choose(0)
            .expect("the simulator implements this option");
        assert_eq!(run.player.relics.len() + 1, branch.player.relics.len());
    }
}
