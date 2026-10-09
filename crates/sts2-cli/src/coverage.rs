//! What play has reached, and what it never has.
//!
//! A crash-free fuzzer can still miss the state containing a bug. Coverage says
//! which encounters, events, cards, and other content a batch of runs touched.
//!
//! [`Coverage::saw`] records reported state. [`Coverage::did`] records actions,
//! distinguishing a card merely drawn from one whose effect ran. Reports group
//! axes by table properties such as act, room, pool, and rarity. This exposes
//! unreachable content that a bare fraction hides.
//!
//! Ids remain strings so coverage can retain names absent from this build.s
//! tables. Such a name is itself evidence that the table and game disagree.

use crate::id::spelling;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use sts2_core::ActMap as _;
use sts2_core::{
    CardIdent, CardTable, CharacterTable, Combat, Decision, EncounterTable, EventTable, Map,
    ModelId, MonsterTable, PotionTable, Question, RelicTable, RewardKind, Room, Snapshot, Stock,
    Table, Take,
};

/// Every id play has touched, by kind.
///
/// Sets, so folding a second run into a first is a union and the same run
/// counted twice changes nothing.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Coverage {
    /// Fights entered, by encounter id.
    pub encounters: BTreeSet<String>,
    /// Monsters that stood in one.
    pub monsters: BTreeSet<String>,
    /// Events walked into.
    pub events: BTreeSet<String>,
    /// Cards seen anywhere at all: a deck, a pile, a reward, a shop shelf.
    pub cards_seen: BTreeSet<String>,
    /// Cards actually played, which is the set whose effects have run.
    pub cards_played: BTreeSet<String>,
    /// Relics held.
    pub relics: BTreeSet<String>,
    /// Relics offered, which is much wider than what a run walks away with: a
    /// chest shows three and gives one.
    pub relics_offered: BTreeSet<String>,
    /// Potions held or offered.
    pub potions: BTreeSet<String>,
    /// Powers applied to any creature, the player included.
    pub powers: BTreeSet<String>,
    /// Characters played.
    pub characters: BTreeSet<String>,
    /// Acts reached, by index.
    pub acts: BTreeSet<String>,
    /// Map point types stood on.
    pub rooms: BTreeSet<String>,
}

impl Coverage {
    /// Folds in everything one reply reported.
    pub fn saw<V: sts2_content_wire::WireContentVersion>(&mut self, snapshot: &Snapshot<V>) {
        if let Some(question) = &snapshot.asking {
            self.question(question);
        }

        let observed = &snapshot.observation;

        self.acts.insert(observed.act.to_string());
        let relics: Vec<_> = observed.relics.iter().map(|r| r.relic).collect();
        let potions: Vec<_> = observed.potions.iter().map(|p| p.potion).collect();
        self.relics.extend(spellings(&relics));
        self.potions.extend(spellings(&potions));
        self.cards_seen.extend(cards(&observed.deck));

        self.map(&observed.map);
        if let Some(room) = &observed.room {
            self.room(room);
        }
        if let Some(combat) = &observed.combat {
            self.combat(combat);
        }
    }

    /// Folds in the move taken, given the state it was taken in.
    ///
    /// The command carries an index and not an id. `play_card 3` says nothing
    /// about which card that was, so the state it was chosen from is what turns
    /// it back into content.
    pub fn did<V: sts2_content_wire::WireContentVersion>(
        &mut self,
        command: &Decision<V>,
        from: &Snapshot<V>,
    ) {
        let Decision::PlayCard { index, .. } = command else {
            return;
        };
        let Some(card) = from
            .observation
            .combat
            .as_ref()
            .and_then(|combat| combat.hand.get(usize::from(index.get())))
            .and_then(|ident| spelling(&ident.card))
        else {
            return;
        };
        self.cards_played.insert(card.clone());
        self.cards_seen.insert(card);
    }

    /// The character a run was started as, which no observation reports.
    pub fn started<V: sts2_content_wire::WireContentVersion>(&mut self, character: V::Character) {
        if let Some(id) = spelling(&character) {
            self.characters.insert(id);
        }
    }

    /// Unions another run's coverage into this one.
    pub fn absorb(&mut self, other: Coverage) {
        let Coverage {
            encounters,
            monsters,
            events,
            cards_seen,
            cards_played,
            relics,
            relics_offered,
            potions,
            powers,
            characters,
            acts,
            rooms,
        } = other;
        self.encounters.extend(encounters);
        self.monsters.extend(monsters);
        self.events.extend(events);
        self.cards_seen.extend(cards_seen);
        self.cards_played.extend(cards_played);
        self.relics.extend(relics);
        self.relics_offered.extend(relics_offered);
        self.potions.extend(potions);
        self.powers.extend(powers);
        self.characters.extend(characters);
        self.acts.extend(acts);
        self.rooms.extend(rooms);
    }

    fn question<V: sts2_content_wire::WireContentVersion>(&mut self, question: &Question<V>) {
        match question {
            Question::SelectCards { options, .. } => {
                self.cards_seen
                    .extend(cards(options.iter().map(|o| &o.card)));
            }
            // Cells and tools, and no content id among them. The rewards the
            // minigame pays out arrive later, as an ordinary reward screen,
            // and are counted there.
            Question::Divine { .. } | Question::Confirm => {}
            Question::ChooseBundle { bundles } => {
                for bundle in bundles {
                    self.cards_seen
                        .extend(cards(bundle.cards.iter().map(|c| &c.card)));
                }
            }
            Question::Rewards { rewards, .. } => {
                for detail in rewards {
                    match &detail.reward {
                        RewardKind::Card { cards: offer, .. } => {
                            self.cards_seen.extend(cards(offer.iter().map(|c| &c.card)));
                        }
                        RewardKind::SpecialCard { card: Some(one) } => {
                            self.cards_seen.extend(cards(std::iter::once(&one.card)));
                        }
                        RewardKind::Relic {
                            relic: Some(id), ..
                        } => {
                            self.relics_offered.extend(spelling(id));
                        }
                        RewardKind::Potion { potion: Some(id) } => {
                            self.potions.extend(spelling(id));
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// The type of the point stood on, which the map graph names and the
    /// position does not.
    fn map<V: sts2_content_wire::WireContentVersion>(&mut self, map: &Map<V>) {
        let Some(here) = map.at else {
            return;
        };
        if let Ok(node) = map.points.at(here)
            && let Some(kind) = spelling(&node.kind)
        {
            self.rooms.insert(kind);
        }
    }

    fn room<V: sts2_content_wire::WireContentVersion>(&mut self, room: &Room<V>) {
        match room {
            Room::Event { event, options, .. } => {
                self.events.extend(spelling(event));
                // An event that hands over a relic names it before it is taken.
                self.relics_offered.extend(
                    options
                        .iter()
                        .filter_map(|o| o.relic.as_ref())
                        .filter_map(spelling),
                );
            }

            Room::Treasure { options, .. } => {
                self.relics_offered
                    .extend(options.iter().filter_map(|o| match &o.take {
                        Take::Relic(relic) => spelling(relic),
                        Take::Skip => None,
                    }));
            }

            // A shelf names cards, relics and potions, and what an entry sells
            // is what says which. The removal entry sells no thing at all.
            Room::Shop { entries } => {
                for entry in entries {
                    match &entry.stock {
                        Stock::Card(Some(on_sale)) => {
                            self.cards_seen
                                .extend(cards(std::iter::once(&on_sale.card)));
                        }
                        Stock::Relic(Some(relic)) => self.relics_offered.extend(spelling(relic)),
                        Stock::Potion(Some(potion)) => self.potions.extend(spelling(potion)),
                        _ => {}
                    }
                }
            }

            Room::RestSite { .. } => {}
        }
    }

    fn combat<V: sts2_content_wire::WireContentVersion>(&mut self, combat: &Combat<V>) {
        self.encounters
            .extend(combat.encounter.as_ref().and_then(spelling));

        for pile in [&combat.hand, &combat.draw, &combat.discard, &combat.exhaust] {
            self.cards_seen.extend(cards(pile));
        }

        for creature in &combat.creatures {
            // The monster itself, not the packed `MONSTER/BOWLBUG` the wire
            // spells it as: the table this is counted against holds the entry
            // alone. A creature on the enemy side that is not a monster — the
            // category says so — is not one either.
            if let Some(ModelId::Monster(monster)) = &creature.model
                && spelling(&creature.side).as_deref() == Some("Enemy")
            {
                self.monsters.extend(spelling(monster));
            }
            self.powers
                .extend(creature.powers.iter().filter_map(|p| spelling(&p.power)));
        }
    }
}

/// The cards a pile holds, by id.
///
/// The id alone, without the upgrade level or the affliction the identity
/// carries. An upgraded Bash is a Bash for the purpose of "has a run ever seen
/// this card". And `BASH+1` is not a card any table lists.
fn cards<'a, V: sts2_content_wire::WireContentVersion + 'a>(
    pile: impl IntoIterator<Item = &'a CardIdent<V>>,
) -> Vec<String> {
    pile.into_iter()
        .filter_map(|ident| spelling(&ident.card))
        .collect()
}

/// One measured axis: what was reached, out of what exists.
#[derive(Debug, Clone)]
pub struct Axis {
    /// What the axis counts.
    pub name: &'static str,
    /// Ids reached.
    pub seen: usize,
    /// Ids this build has.
    pub total: usize,
    /// Reached and reachable, out of reachable, for the axes where the table
    /// says what a run can get to at all. `None` where it does not, which is
    /// not the same as everything being reachable.
    pub reachable: Option<(usize, usize)>,
    /// Ids never reached, in the table's order.
    pub missing: Vec<String>,
    /// Ids reached that the table does not list, which is a disagreement
    /// between the tables and the game rather than a gap in play.
    pub unlisted: Vec<String>,
    /// The same count, cut by the keys the table carries.
    pub groupings: Vec<Grouping>,
}

impl Axis {
    /// Reached, as a percentage of what is reachable if that is known and of
    /// the whole table otherwise. Zero out of zero is complete.
    pub fn percent(&self) -> f64 {
        let (seen, total) = self.reachable.unwrap_or((self.seen, self.total));
        match total {
            0 => 100.0,
            total => 100.0 * seen as f64 / total as f64,
        }
    }
}

/// One way of cutting an axis: by act, by rarity, by pool.
#[derive(Debug, Clone)]
pub struct Grouping {
    /// The key the slices are of.
    pub by: &'static str,
    /// One per distinct key, in the order the table first names them.
    pub slices: Vec<Slice>,
}

/// One key's share of an axis.
#[derive(Debug, Clone)]
pub struct Slice {
    /// The key, as the game spells it.
    pub key: String,
    /// Ids reached under it.
    pub seen: usize,
    /// Ids it holds.
    pub total: usize,
}

/// A key extractor: zero or more keys per value, since an encounter can belong
/// to several acts and a card to no pool at all.
type Keys<T> = Box<dyn Fn(T) -> Vec<String>>;

/// Every axis with a table behind it, measured.
pub fn report<V: sts2_content_wire::WireContentVersion>(coverage: &Coverage) -> Vec<Axis> {
    let acts_of = |e: V::Encounter| spellings(EncounterTable::acts(e));
    let room_of = |e: V::Encounter| spellings(&[e.room()]);
    let card_pool = |c: V::Card| spellings(&c.pool().into_iter().collect::<Vec<_>>());
    let card_rarity = |c: V::Card| spellings(&[c.rarity()]);
    let relic_rarity = |r: V::Relic| spellings(&[r.rarity()]);
    let relic_pools = |r: V::Relic| spellings(RelicTable::pools(r));

    vec![
        axis::<V::Encounter>(
            "encounters",
            &coverage.encounters,
            // An encounter no act lists is one only an event or a debug
            // command starts, so no amount of playing will reach it.
            Some(Box::new(|e| !EncounterTable::acts(e).is_empty())),
            vec![
                ("act", Box::new(acts_of)),
                ("room", Box::new(room_of) as Keys<V::Encounter>),
            ],
        ),
        axis::<V::Event>(
            "events",
            &coverage.events,
            Some(Box::new(|e: V::Event| {
                !EventTable::acts(e).is_empty() || e.any_act()
            })),
            vec![(
                "act",
                Box::new(|e: V::Event| {
                    // The any-act pool is offered everywhere and lists no act,
                    // so it is its own key rather than nothing.
                    match e.any_act() {
                        true => vec!["ANY_ACT".to_string()],
                        false => spellings(EventTable::acts(e)),
                    }
                }) as Keys<V::Event>,
            )],
        ),
        axis::<V::Monster>(
            "monsters",
            &coverage.monsters,
            // A monster with no act is a summon: reachable, but only through
            // whatever summons it, which no map rolls directly.
            None,
            vec![(
                "act",
                Box::new(|m: V::Monster| spellings(MonsterTable::acts(m))) as Keys<V::Monster>,
            )],
        ),
        axis::<V::Card>(
            "cards seen",
            &coverage.cards_seen,
            None,
            vec![
                ("pool", Box::new(card_pool)),
                ("rarity", Box::new(card_rarity) as Keys<V::Card>),
            ],
        ),
        axis::<V::Card>(
            "cards played",
            &coverage.cards_played,
            None,
            vec![
                ("pool", Box::new(card_pool)),
                ("rarity", Box::new(card_rarity) as Keys<V::Card>),
            ],
        ),
        axis::<V::Relic>(
            "relics offered",
            &coverage.relics_offered,
            None,
            vec![
                ("rarity", Box::new(relic_rarity)),
                ("pool", Box::new(relic_pools) as Keys<V::Relic>),
            ],
        ),
        axis::<V::Relic>(
            "relics held",
            &coverage.relics,
            None,
            vec![("rarity", Box::new(relic_rarity) as Keys<V::Relic>)],
        ),
        axis::<V::Potion>(
            "potions",
            &coverage.potions,
            None,
            vec![(
                "pool",
                Box::new(|p: V::Potion| spellings(&[p.pool()])) as Keys<V::Potion>,
            )],
        ),
        axis::<V::Power>("powers", &coverage.powers, None, vec![]),
        axis::<V::Character>(
            "characters",
            &coverage.characters,
            Some(Box::new(CharacterTable::playable)),
            vec![],
        ),
    ]
}

/// One kind's table against one kind's sightings.
fn axis<T: Table + Copy + serde::Serialize>(
    name: &'static str,
    seen: &BTreeSet<String>,
    reachable: Option<Box<dyn Fn(T) -> bool>>,
    groupings: Vec<(&'static str, Keys<T>)>,
) -> Axis {
    let every: Vec<(T, String)> = T::ALL
        .iter()
        .filter_map(|value| spelling(value).map(|id| (*value, id)))
        .collect();
    let known: BTreeSet<&str> = every.iter().map(|(_, id)| id.as_str()).collect();
    let reached = |id: &String| seen.contains(id);

    Axis {
        name,
        seen: every.iter().filter(|(_, id)| reached(id)).count(),
        total: every.len(),
        reachable: reachable.map(|can| {
            let within: Vec<&(T, String)> = every.iter().filter(|(v, _)| can(*v)).collect();
            (
                within.iter().filter(|(_, id)| reached(id)).count(),
                within.len(),
            )
        }),
        missing: every
            .iter()
            .filter(|(_, id)| !reached(id))
            .map(|(_, id)| id.clone())
            .collect(),
        unlisted: seen
            .iter()
            .filter(|id| !known.contains(id.as_str()))
            .cloned()
            .collect(),
        groupings: groupings
            .into_iter()
            .map(|(by, keys)| Grouping {
                by,
                slices: slice(&every, &keys, seen),
            })
            .collect(),
    }
}

/// One grouping's slices, in the order the table first names each key.
fn slice<T: Copy>(every: &[(T, String)], keys: &Keys<T>, seen: &BTreeSet<String>) -> Vec<Slice> {
    let mut slices: Vec<Slice> = Vec::new();
    for (value, id) in every {
        for key in keys(*value) {
            let at = match slices.iter().position(|s| s.key == key) {
                Some(at) => at,
                None => {
                    slices.push(Slice {
                        key,
                        seen: 0,
                        total: 0,
                    });
                    slices.len() - 1
                }
            };
            slices[at].total += 1;
            slices[at].seen += usize::from(seen.contains(id));
        }
    }
    slices
}

/// Ids as the game spells them, which is their serialisation and nothing else.
fn spellings<T: serde::Serialize>(values: &[T]) -> Vec<String> {
    values.iter().filter_map(crate::id::spelling).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture::{self, creature};
    use serde_json::json;
    use sts2_core::fixture::hand;

    type V = sts2_content::v0_107_1::V0_107_1;

    fn axis_named<'a>(report: &'a [Axis], name: &str) -> &'a Axis {
        report.iter().find(|a| a.name == name).expect("axis")
    }

    #[test]
    fn a_fight_covers_its_encounter_and_its_monsters() {
        let mut coverage = Coverage::default();
        coverage.saw(&fixture::snapshot(json!({
            "inCombat": true,
            "combat": fixture::combat(json!({
                "encounter": "BOWLBUGS_WEAK",
                "hand": ["STRIKE_IRONCLAD"],
                "creatures": [
                    creature(1, "Player", Some("CHARACTER/IRONCLAD"), &[("VULNERABLE_POWER", 2)]),
                    creature(2, "Enemy", Some("MONSTER/BOWLBUG_ROCK"), &[]),
                ]
            }))
        })));

        assert!(coverage.encounters.contains("BOWLBUGS_WEAK"));
        assert_eq!(coverage.monsters, ["BOWLBUG_ROCK".to_string()].into());
        assert!(coverage.powers.contains("VULNERABLE_POWER"));
        // Seen, because it is in hand; not played, because nothing played it.
        assert!(coverage.cards_seen.contains("STRIKE_IRONCLAD"));
        assert!(coverage.cards_played.is_empty());
    }

    #[test]
    fn playing_a_card_is_what_covers_it() {
        let before = fixture::snapshot(json!({
            "inCombat": true,
            "combat": fixture::combat(json!({ "hand": ["STRIKE_IRONCLAD", "BASH"] }))
        }));
        let mut coverage = Coverage::default();
        coverage.did::<V>(
            &Decision::PlayCard {
                index: hand(1),
                target: None,
            },
            &before,
        );

        assert_eq!(coverage.cards_played, ["BASH".to_string()].into());
    }

    #[test]
    fn a_chest_covers_what_it_offered_not_what_was_taken() {
        let mut coverage = Coverage::default();
        coverage.saw(&fixture::snapshot(json!({
            "relics": ["BURNING_BLOOD"],
            "room": {
                "type": "Treasure", "finished": false,
                "options": [
                    { "index": 0, "id": "AKABEKO", "relic": "AKABEKO", "chosen": false },
                    { "index": 1, "id": "ANCHOR", "relic": "ANCHOR", "chosen": false },
                    { "index": 2, "id": "SKIP", "skip": true, "chosen": false },
                ],
            }
        })));

        assert_eq!(
            coverage.relics_offered,
            ["AKABEKO".to_string(), "ANCHOR".to_string()].into()
        );
        assert_eq!(coverage.relics, ["BURNING_BLOOD".to_string()].into());
    }

    #[test]
    fn the_report_counts_against_the_build_s_own_tables() {
        let mut coverage = Coverage::default();
        coverage.encounters.insert("BOWLBUGS_WEAK".into());
        coverage.encounters.insert("NOT_AN_ENCOUNTER".into());

        let report = report::<V>(&coverage);
        let encounters = axis_named(&report, "encounters");

        assert_eq!(encounters.total, 96);
        assert_eq!(encounters.seen, 1);
        assert_eq!(encounters.unlisted, ["NOT_AN_ENCOUNTER".to_string()]);
        assert!(encounters.missing.contains(&"AEONGLASS_BOSS".to_string()));
    }

    #[test]
    fn the_unreachable_is_counted_apart_from_the_unreached() {
        let report = report::<V>(&Coverage::default());
        let encounters = axis_named(&report, "encounters");
        let (_, reachable) = encounters.reachable.expect("encounters know their acts");

        // The mocks and the event-started fights are in the table and no map
        // rolls them, so they are not play's to reach.
        assert!(reachable < encounters.total);
        assert_eq!(
            encounters.total - reachable,
            <<V as sts2_core::ContentVersion>::Encounter as Table>::ALL
                .iter()
                .filter(|e| EncounterTable::acts(**e).is_empty())
                .count()
        );
    }

    #[test]
    fn an_axis_is_cut_by_the_keys_its_table_carries() {
        let mut coverage = Coverage::default();
        // Both Hive bosses, so the Hive slice moves and no other does.
        coverage.encounters.insert("KAISER_CRAB_BOSS".into());
        coverage.encounters.insert("THE_INSATIABLE_BOSS".into());

        let report = report::<V>(&coverage);
        let by_act = axis_named(&report, "encounters")
            .groupings
            .iter()
            .find(|g| g.by == "act")
            .expect("encounters are grouped by act");

        let hive = by_act.slices.iter().find(|s| s.key == "HIVE").unwrap();
        assert_eq!(hive.seen, 2);
        for other in by_act.slices.iter().filter(|s| s.key != "HIVE") {
            assert_eq!(other.seen, 0, "{} moved", other.key);
        }
    }

    #[test]
    fn absorbing_a_run_is_a_union() {
        let mut first = Coverage::default();
        first.events.insert("ABYSSAL_BATHS".into());
        let mut second = Coverage::default();
        second.events.insert("ABYSSAL_BATHS".into());
        second.events.insert("BYRDONIS_NEST".into());

        first.absorb(second);
        assert_eq!(first.events.len(), 2);
    }
    /// A monster is counted by its entry, not by the packed id it arrives as.
    ///
    /// The observation writes `CATEGORY/ENTRY`. Coverage once stored that whole
    /// string although the monster table lists only entries. Every fought
    /// monster then counted as unlisted. Fixtures hid the bug by omitting the
    /// category. A typed `ModelId<V>` keeps both fields separate, and this
    /// counts only `ModelId::Monster`.
    #[test]
    fn a_monster_is_counted_by_its_entry_and_not_its_category() {
        let mut coverage = Coverage::default();
        coverage.saw(&fixture::snapshot(json!({
            "inCombat": true,
            "combat": fixture::combat(json!({
                "creatures": [creature(1, "Enemy", Some("MONSTER/BOWLBUG_ROCK"), &[])]
            }))
        })));

        assert_eq!(coverage.monsters, ["BOWLBUG_ROCK".to_string()].into());

        let report = report::<V>(&coverage);
        let axis = axis_named(&report, "monsters");
        assert_eq!(axis.seen, 1, "a fought monster counts as seen");
        assert!(
            axis.unlisted.is_empty(),
            "and not as a name the table has never heard of: {:?}",
            axis.unlisted
        );
    }

    /// An upgraded card is that card, for the purpose of having seen it.
    ///
    /// The same shape of bug in the other direction. A deck holding `BASH+1`
    /// put that string into `cards_seen`, and it is not a card any table lists.
    /// So an upgraded card counted as unlisted, and a card only ever met
    /// upgraded never counted as seen at all.
    #[test]
    fn an_upgraded_card_is_counted_as_the_card_it_is() {
        let mut coverage = Coverage::default();
        coverage.saw(&fixture::snapshot(json!({
            "deck": ["BASH+1", "STRIKE_IRONCLAD/SMOG", "DEFEND_IRONCLAD"]
        })));

        assert_eq!(
            coverage.cards_seen,
            [
                "BASH".to_string(),
                "STRIKE_IRONCLAD".to_string(),
                "DEFEND_IRONCLAD".to_string()
            ]
            .into()
        );

        let report = report::<V>(&coverage);
        assert!(axis_named(&report, "cards seen").unlisted.is_empty());
    }

    /// Playing an upgraded card counts the card, for the same reason.
    #[test]
    fn playing_an_upgraded_card_counts_the_card() {
        let before = fixture::snapshot(json!({
            "inCombat": true,
            "combat": fixture::combat(json!({ "hand": ["BASH+1"] }))
        }));

        let mut coverage = Coverage::default();
        coverage.did::<V>(
            &Decision::PlayCard {
                index: hand(0),
                target: None,
            },
            &before,
        );

        assert_eq!(coverage.cards_played, ["BASH".to_string()].into());
    }

    /// A shop's stock is counted by what the entry sells.
    #[test]
    fn a_shop_covers_the_three_kinds_of_thing_it_sells() {
        let entry = |index: u8, kind: &str, id: &str| {
            json!({"index": index, "kind": kind, "id": id, "cost": 50,
                   "stocked": true, "affordable": true, "blocked": null})
        };
        let mut coverage = Coverage::default();
        coverage.saw(&fixture::snapshot(json!({
            "room": {"type": "Shop", "entries": [
                entry(0, "MerchantCardEntry", "BASH+1"),
                entry(1, "MerchantRelicEntry", "AKABEKO"),
                entry(2, "MerchantPotionEntry", "ASHWATER"),
                json!({"index": 3, "kind": "MerchantCardRemovalEntry", "id": null,
                       "cost": 75, "stocked": true, "affordable": true, "blocked": null})
            ]}
        })));

        assert!(coverage.cards_seen.contains("BASH"));
        assert!(coverage.relics_offered.contains("AKABEKO"));
        assert!(coverage.potions.contains("ASHWATER"));
    }
}
