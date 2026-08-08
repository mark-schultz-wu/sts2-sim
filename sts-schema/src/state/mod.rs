//! One decision point, as any backend reports it.
//!
//! [`Obs`] is the type a differential test compares; everything else here is a
//! field of it.

mod card;
mod location;
mod monster;
mod pile;
mod rng;

pub use card::{Card, CardRarity, CardType};
pub use location::{RoomType, ScreenType};
pub use monster::{Intent, IntentType, Monster};
pub use pile::{CombatPile, Pile};
pub use rng::{GameRng, Stream, Sts1Stream, Sts2Stream};

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::provenance::GameVersion;
use crate::{Input, Provenance};

/// One decision point.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound = "G: GameRng + GameVersion")]
pub struct Obs<G: GameRng + GameVersion> {
    pub provenance: Provenance<G>,
    pub screen: ScreenType,
    /// Which room, when [`screen`](Obs::screen) is [`ScreenType::Room`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<RoomType>,
    pub legal: Vec<Input>,

    /// Which player this observation is for. STS2 is co-op; STS1 pins it to 0.
    #[serde(default)]
    pub player_idx: u8,

    pub floor: i32,
    pub act: i32,
    pub hp: i32,
    pub max_hp: i32,
    pub gold: i32,

    // Combat-only. Absent outside a fight, so that "not in combat" and "zero"
    // stay distinguishable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub energy: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<i32>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub monsters: Vec<Monster>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub piles: BTreeMap<Pile, Vec<Card>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub powers: BTreeMap<String, i32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relics: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub potions: Vec<String>,

    /// Where each generator has got to. STS1 reports these only under
    /// `rngspy`. Compared only where both sides name the same stream.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rng: BTreeMap<G::StreamName, Stream<G>>,

    #[serde(default, skip_serializing_if = "is_false")]
    pub done: bool,
}

impl<G: GameRng + GameVersion> Obs<G> {
    /// In a room where something is fighting back.
    ///
    /// Needs both axes: `Room` also covers shops and rest sites, and a monster
    /// room stays a monster room once its rewards screen is up.
    pub fn in_combat(&self) -> bool {
        self.screen == ScreenType::Room
            && matches!(
                self.room,
                Some(RoomType::Monster | RoomType::Elite | RoomType::Boss)
            )
    }

    pub fn pile(&self, p: Pile) -> &[Card] {
        self.piles.get(&p).map(Vec::as_slice).unwrap_or(&[])
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;
    use crate::{Action, Sts1};

    fn obs() -> Obs<Sts1> {
        let mut piles = BTreeMap::new();
        piles.insert(
            Pile::Combat(CombatPile::Hand),
            vec![Card {
                id: "Strike_R".into(),
                uid: Some(4),
                cost: Some(1),
                ..Card::default()
            }],
        );
        Obs {
            provenance: Provenance {
                game: Sts1,
                game_version: crate::BuildId(10180494),
                adapter: "sts1-cli".into(),
                mods: vec!["determinism".into()],
            },
            screen: ScreenType::Room,
            room: Some(RoomType::Monster),
            legal: vec![Input::Action(Action::PlayCard {
                card: 1,
                target: nonmax::NonMaxU8::new(0),
            })],
            player_idx: 0,
            floor: 1,
            act: 1,
            hp: 80,
            max_hp: 80,
            gold: 99,
            energy: Some(3),
            turn: Some(1),
            block: Some(0),
            monsters: vec![Monster {
                id: "JawWorm".into(),
                hp: 44,
                max_hp: 44,
                intent: Some(Intent {
                    kind: IntentType::Attack,
                    repeats: Some(1),
                    damage: Some(11),
                }),
                ..Monster::default()
            }],
            piles,
            powers: BTreeMap::new(),
            relics: vec!["Burning Blood".into()],
            potions: vec![],
            rng: BTreeMap::from([(
                Sts1Stream::Shuffle,
                Stream {
                    seed: 7,
                    counter: 3,
                    state: None,
                },
            )]),
            done: false,
        }
    }

    /// Each pair differs in exactly one axis, and each difference is one a
    /// diff has to see.
    #[test]
    fn the_two_location_axes_are_independent() {
        let monster = obs();
        let elite = Obs::<Sts1> {
            room: Some(RoomType::Elite),
            ..obs()
        };
        assert_ne!(monster, elite, "elite and monster rooms must differ");
        assert!(monster.in_combat() && elite.in_combat());

        let shop = Obs::<Sts1> {
            room: Some(RoomType::Shop),
            ..obs()
        };
        assert!(!shop.in_combat(), "a shop is a Room but not a fight");

        // Same room, different screen: the fight is over.
        let rewards = Obs::<Sts1> {
            screen: ScreenType::Rewards,
            ..obs()
        };
        assert!(!rewards.in_combat(), "rewards are up, so combat has ended");
    }

    /// Holds the wire format to a payload this crate did not produce: an
    /// adapter, a recorded trace, or a hand-written fixture.
    #[test]
    fn accepts_a_minimal_hand_written_payload() {
        let json = r#"{
            "provenance": {"game": "sts2", "game_version": "v0.107.1",
                           "adapter": "sts2-cli", "mods": []},
            "screen": "map", "legal": [], "floor": 3, "act": 1,
            "hp": 60, "max_hp": 80, "gold": 120
        }"#;
        let o: Obs<crate::Sts2> = serde_json::from_str(json).unwrap();
        assert!(o.provenance.mods.is_empty());
        assert_eq!(o.energy, None, "absent combat fields stay None, not 0");
        assert!(!o.in_combat());
        assert!(o.pile(Pile::Combat(CombatPile::Hand)).is_empty());
    }

    /// An unmodelled field must fail at the seam, where the fix is to model
    /// it, rather than be dropped and resurface later as a causeless
    /// divergence.
    #[test]
    fn an_unmodelled_field_is_refused() {
        let json = r#"{
            "provenance": {"game": "sts2", "game_version": "v0.107.1",
                           "adapter": "sts2-cli", "mods": []},
            "screen": "map", "legal": [], "floor": 3, "act": 1,
            "hp": 60, "max_hp": 80, "gold": 120,
            "ascension": 20
        }"#;
        assert!(
            serde_json::from_str::<Obs<crate::Sts2>>(json).is_err(),
            "an unmodelled field must not be silently dropped"
        );
    }

    #[test]
    fn optional_combat_fields_are_omitted_not_zeroed() {
        let mut o = obs();
        o.energy = None;
        o.turn = None;
        o.block = None;
        let s = serde_json::to_string(&o).unwrap();
        assert!(
            !s.contains("energy"),
            "None must be omitted, not serialised as 0"
        );
    }

    #[test]
    fn provenance_is_mandatory() {
        // An observation that cannot say what produced it is not a result.
        let json = r#"{"screen":"map","legal":[],"floor":1,"act":1,
                       "hp":80,"max_hp":80,"gold":99}"#;
        assert!(serde_json::from_str::<Obs<Sts1>>(json).is_err());
    }
}
