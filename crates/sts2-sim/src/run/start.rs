use std::collections::HashMap;

use sts2_core::rng::{GameRng, MegaRng, stream_seed};
use sts2_core::{
    ActTable, Applied, CardIdent, Carried, Counters, EventOption, Map, MapPoints, Node,
    Observation, Reachable, Room, Rules, Seed, Table,
};

use crate::engine::Sim;
use crate::event::Blessing;
use crate::map::{ActMap, Layout};
use crate::run::Opening;
impl<V: Rules> Sim<V> {
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
        let _opening = Opening::<V>::draw(&mut up_front, character, &acts);

        let mut map_rng = MegaRng::named(raw_seed, "act_1_map");
        let act_map = ActMap::<V>::generate(&mut map_rng, Layout::ACT_ONE, ascension)
            .expect("the game's act-one layout fits a packed map path");
        let nodes: Vec<Node<V>> = act_map
            .nodes()
            .into_iter()
            .map(|(at, kind, to)| Node { at, kind, to })
            .collect();
        let first = nodes
            .iter()
            .find(|node| node.kind == V::MapPointType::ANCIENT)
            .expect("an act map opens on an ancient");
        let first_at = first.at;
        let travelable = first
            .to
            .iter()
            .map(|&at| {
                let kind = nodes
                    .iter()
                    .find(|node| node.at == at)
                    .expect("a map edge leads to a node")
                    .kind;
                Reachable { at, kind }
            })
            .collect();

        let mut neow_rng: MegaRng = Blessing::<V>::stream(raw_seed, 0);
        let options = Blessing::<V>::roll(&mut neow_rng, 1)
            .options
            .into_iter()
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
                    chosen: false,
                    relic: Some(relic),
                }
            })
            .collect();

        let mut run: HashMap<V::RunRngType, u32> = V::run_rng_types()
            .iter()
            .copied()
            .map(|stream| (stream, 0))
            .collect();
        run.insert(V::up_front_rng(), up_front.counter() as u32);
        let player = V::player_rng_types()
            .iter()
            .copied()
            .map(|stream| (stream, 0))
            .collect();
        let hp = i32::from(V::starting_hp(character));
        let current = Applied {
            observation: Observation {
                act: 0,
                floor: 1,
                hp,
                max_hp: hp,
                alive: true,
                run_over: false,
                victory: None,
                gold: i32::from(V::starting_gold(character)),
                relics: V::starting_relics(character)
                    .iter()
                    .copied()
                    .map(|relic| Carried {
                        relic,
                        title: None,
                        text: None,
                    })
                    .collect(),
                potions: Vec::new(),
                potion_slots: V::starting_potion_slots(),
                deck: V::starting_deck(character)
                    .iter()
                    .copied()
                    .map(CardIdent::new)
                    .collect(),
                in_combat: false,
                map: Map {
                    at: Some(first_at),
                    visited: vec![first_at],
                    travelable,
                    points: nodes,
                    proceed: false,
                },
                rng: Counters { run, player },
                room: Some(Room::Event {
                    event: V::neow_event(),
                    finished: false,
                    title: None,
                    text: None,
                    options,
                }),
                combat: None,
            },
            asking: None,
            resolved: true,
        };
        Self {
            current,
            reward_seed: raw_seed,
        }
    }
}
