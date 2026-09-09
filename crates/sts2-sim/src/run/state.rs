use std::collections::HashMap;

use sts2_core::rng::{GameRng, MegaRng, stream_seed};
use sts2_core::{
    ActTable, Answer, Applied, CardIdent, CardOption, Carried, Counters, EventOption, Map,
    MapPoints, Node, Observation, Question, Reachable, Room, Rules, Seed, Table,
};

use crate::engine::Error;
use crate::event::Blessing;
use crate::map::{ActMap, Layout};
use crate::run::Opening;

/// The authoritative state of the run slice implemented so far.
///
/// Reports are built from this state. They are never stored and edited as if
/// they were the run itself.
#[derive(Debug)]
pub(crate) struct State<V: Rules> {
    _opening: Opening<V>,
    nodes: Vec<Node<V>>,
    at: sts2_core::Coord,
    relics: Vec<V::Relic>,
    deck: Vec<CardIdent<V>>,
    hp: i32,
    gold: i32,
    potion_slots: usize,
    run_counters: HashMap<V::RunRngType, u32>,
    player_counters: HashMap<V::PlayerRngType, u32>,
    rewards_rng: MegaRng,
    neow_options: Vec<V::Relic>,
    neow_chosen: Option<u8>,
    neow_finished: bool,
    card_selection: Option<Vec<V::Card>>,
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
        let act_map = ActMap::<V>::generate(&mut map_rng, Layout::ACT_ONE, ascension)
            .expect("the game's act-one layout fits a packed map path");
        let nodes: Vec<Node<V>> = act_map
            .nodes()
            .into_iter()
            .map(|(at, kind, to)| Node { at, kind, to })
            .collect();
        let at = nodes
            .iter()
            .find(|node| node.kind == V::MapPointType::ANCIENT)
            .expect("an act map opens on an ancient")
            .at;

        let mut neow_rng: MegaRng = Blessing::<V>::stream(raw_seed, 0);
        let neow_options = Blessing::<V>::roll(&mut neow_rng, 1).options;

        let mut run_counters: HashMap<V::RunRngType, u32> = V::run_rng_types()
            .iter()
            .copied()
            .map(|stream| (stream, 0))
            .collect();
        run_counters.insert(V::up_front_rng(), up_front.counter() as u32);
        let player_counters = V::player_rng_types()
            .iter()
            .copied()
            .map(|stream| (stream, 0))
            .collect();

        Self {
            _opening: opening,
            nodes,
            at,
            relics: V::starting_relics(character),
            deck: V::starting_deck(character)
                .into_iter()
                .map(CardIdent::new)
                .collect(),
            hp: i32::from(V::starting_hp(character)),
            gold: i32::from(V::starting_gold(character)),
            potion_slots: V::starting_potion_slots(),
            run_counters,
            player_counters,
            rewards_rng: MegaRng::named(raw_seed, "rewards"),
            neow_options,
            neow_chosen: None,
            neow_finished: false,
            card_selection: None,
        }
    }

    pub(crate) fn choose(&mut self, index: u8) -> Result<Applied<V>, Error> {
        let option = self
            .neow_options
            .get(index as usize)
            .ok_or(Error::NotImplemented("choose_option"))?;
        if *option != V::lead_paperweight() {
            return Err(Error::NotImplemented("this Neow option"));
        }

        self.neow_chosen = Some(index);
        self.relics.push(V::lead_paperweight());
        self.card_selection = Some(crate::reward::card::regular::<V, _>(
            &mut self.rewards_rng,
            &V::colorless_cards(),
            2,
        ));
        Ok(self.report())
    }

    pub(crate) fn answer(&mut self, answer: &Answer) -> Result<Applied<V>, Error> {
        let cards = self
            .card_selection
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

        self.deck.extend(chosen);
        self.card_selection = None;
        self.neow_finished = true;
        Ok(self.report())
    }

    /// Derive the comparison snapshot from the run's authoritative state.
    pub(crate) fn report(&self) -> Applied<V> {
        let travelable = self
            .nodes
            .iter()
            .find(|node| node.at == self.at)
            .expect("the run stands on its map")
            .to
            .iter()
            .map(|&at| {
                let kind = self
                    .nodes
                    .iter()
                    .find(|node| node.at == at)
                    .expect("a map edge leads to a node")
                    .kind;
                Reachable { at, kind }
            })
            .collect();
        let options = self
            .neow_options
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
                    chosen: self.neow_chosen == Some(index as u8),
                    relic: Some(relic),
                }
            })
            .collect();
        let asking = self
            .card_selection
            .as_ref()
            .map(|cards| Question::SelectCards {
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
        let mut player = self.player_counters.clone();
        player.insert(V::rewards_rng(), self.rewards_rng.counter() as u32);

        Applied {
            observation: Observation {
                act: 0,
                floor: 1,
                hp: self.hp,
                max_hp: self.hp,
                alive: true,
                run_over: false,
                victory: None,
                gold: self.gold,
                relics: self
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
                potion_slots: self.potion_slots,
                deck: self.deck.clone(),
                in_combat: false,
                map: Map {
                    at: Some(self.at),
                    visited: vec![self.at],
                    travelable,
                    points: self.nodes.clone(),
                    proceed: false,
                },
                rng: Counters {
                    run: self.run_counters.clone(),
                    player,
                },
                room: Some(Room::Event {
                    event: V::neow_event(),
                    finished: self.neow_finished,
                    title: None,
                    text: None,
                    options: if self.neow_finished {
                        Vec::new()
                    } else {
                        options
                    },
                }),
                combat: None,
            },
            asking,
            resolved: true,
        }
    }
}
