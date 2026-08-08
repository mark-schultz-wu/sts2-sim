//! Comparing two observations, and locating where a run first went wrong.
//!
//! Two rules shape this:
//!
//! **Compare only what both sides have.** An absent field means "this side
//! cannot say", not "this side says nothing". STS2 has no card `uid` and STS1
//! reports RNG counters only under `rngspy`, so treating absence as a
//! disagreement would bury the real signal.
//!
//! **Report the RNG counters alongside the deltas.** They localise a
//! divergence to a stream and a draw index.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::provenance::GameVersion;
use crate::{Card, GameRng, Obs, Pile, Stream};

/// One field-level disagreement. `path` is dotted for nesting, as in
/// `piles.hand[2].cost`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delta {
    pub path: String,
    pub left: String,
    pub right: String,
}

impl Delta {
    fn new(path: impl Into<String>, l: impl core::fmt::Debug, r: impl core::fmt::Debug) -> Self {
        Delta {
            path: path.into(),
            left: format!("{l:?}"),
            right: format!("{r:?}"),
        }
    }
}

impl core::fmt::Display for Delta {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {} != {}", self.path, self.left, self.right)
    }
}

/// Where two traces first disagree.
#[derive(Clone, Debug)]
pub struct Divergence<G: GameRng> {
    /// Index into both traces, and so also the number of steps that agreed.
    pub step: usize,
    pub deltas: Vec<Delta>,
    /// Streams whose counters disagree at this step, with both positions.
    pub rng: Vec<(G::StreamName, Stream<G>, Stream<G>)>,
}

impl<G: GameRng> core::fmt::Display for Divergence<G> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "diverged at step {}", self.step)?;
        for d in &self.deltas {
            writeln!(f, "  {d}")?;
        }
        for (name, l, r) in &self.rng {
            writeln!(f, "  rng[{name:?}]: counter {} != {}", l.counter, r.counter)?;
        }
        Ok(())
    }
}

/// Field-level comparison of one decision point.
///
/// `left` is conventionally the simulator and `right` the oracle; the order
/// only affects how a report reads.
pub fn compare<G: GameRng + GameVersion>(left: &Obs<G>, right: &Obs<G>) -> Vec<Delta> {
    let mut out = Vec::new();

    if left.screen != right.screen {
        out.push(Delta::new("screen", left.screen, right.screen));
    }
    // Both halves of "where am I". Comparing only the screen would let an
    // elite pass for a monster, which is the divergence this axis exists for.
    if left.room != right.room {
        out.push(Delta::new("room", left.room, right.room));
    }
    for (path, l, r) in [
        ("floor", left.floor, right.floor),
        ("act", left.act, right.act),
        ("hp", left.hp, right.hp),
        ("max_hp", left.max_hp, right.max_hp),
        ("gold", left.gold, right.gold),
    ] {
        if l != r {
            out.push(Delta::new(path, l, r));
        }
    }
    // Optional fields compare only when *both* sides have an opinion.
    for (path, l, r) in [
        ("energy", left.energy, right.energy),
        ("turn", left.turn, right.turn),
        ("block", left.block, right.block),
    ] {
        if let (Some(l), Some(r)) = (l, r)
            && l != r
        {
            out.push(Delta::new(path, l, r));
        }
    }

    if left.monsters.len() != right.monsters.len() {
        out.push(Delta::new(
            "monsters.len",
            left.monsters.len(),
            right.monsters.len(),
        ));
    } else {
        for (i, (a, b)) in left.monsters.iter().zip(&right.monsters).enumerate() {
            if a.id != b.id {
                out.push(Delta::new(format!("monsters[{i}].id"), &a.id, &b.id));
            }
            if a.hp != b.hp {
                out.push(Delta::new(format!("monsters[{i}].hp"), a.hp, b.hp));
            }
            if a.block != b.block {
                out.push(Delta::new(format!("monsters[{i}].block"), a.block, b.block));
            }
            if let (Some(x), Some(y)) = (&a.intent, &b.intent)
                && x != y
            {
                out.push(Delta::new(format!("monsters[{i}].intent"), x, y));
            }
            for k in keys(&a.powers, &b.powers) {
                let (x, y) = (a.powers.get(&k), b.powers.get(&k));
                if x != y {
                    out.push(Delta::new(format!("monsters[{i}].powers.{k}"), x, y));
                }
            }
        }
    }

    // The union of the piles either side reports. `Pile` is `Ord`, so this
    // iterates in declaration order every run.
    for p in keys(&left.piles, &right.piles) {
        compare_pile(p, left.pile(p), right.pile(p), &mut out);
    }

    for k in keys(&left.powers, &right.powers) {
        let (x, y) = (left.powers.get(&k), right.powers.get(&k));
        if x != y {
            out.push(Delta::new(format!("powers.{k}"), x, y));
        }
    }
    if left.relics != right.relics {
        out.push(Delta::new("relics", &left.relics, &right.relics));
    }
    if left.potions != right.potions {
        out.push(Delta::new("potions", &left.potions, &right.potions));
    }
    out
}

fn compare_pile(p: Pile, l: &[Card], r: &[Card], out: &mut Vec<Delta>) {
    let name = format!("{p:?}").to_lowercase();
    if l.len() != r.len() {
        out.push(Delta::new(format!("piles.{name}.len"), l.len(), r.len()));
        return;
    }
    for (i, (a, b)) in l.iter().zip(r).enumerate() {
        if a.id != b.id {
            out.push(Delta::new(format!("piles.{name}[{i}].id"), &a.id, &b.id));
            continue; // a different card makes its other fields moot
        }
        if a.upgrade_level != b.upgrade_level {
            out.push(Delta::new(
                format!("piles.{name}[{i}].upgrade_level"),
                a.upgrade_level,
                b.upgrade_level,
            ));
        }
        // Only when both sides can say.
        if let (Some(x), Some(y)) = (a.uid, b.uid)
            && x != y
        {
            out.push(Delta::new(format!("piles.{name}[{i}].uid"), x, y));
        }
        if let (Some(x), Some(y)) = (a.cost, b.cost)
            && x != y
        {
            out.push(Delta::new(format!("piles.{name}[{i}].cost"), x, y));
        }
    }
}

fn keys<K: Ord + Clone, V>(
    a: &alloc::collections::BTreeMap<K, V>,
    b: &alloc::collections::BTreeMap<K, V>,
) -> BTreeSet<K> {
    a.keys().chain(b.keys()).cloned().collect()
}

/// Streams whose counters disagree, considering only streams both sides emit.
pub fn rng_deltas<G: GameRng + GameVersion>(
    left: &Obs<G>,
    right: &Obs<G>,
) -> Vec<(G::StreamName, Stream<G>, Stream<G>)> {
    let mut out = Vec::new();
    for (name, l) in &left.rng {
        if let Some(r) = right.rng.get(name)
            && (l.counter != r.counter || l.seed != r.seed)
        {
            out.push((*name, *l, *r));
        }
    }
    out
}

/// First step at which two traces disagree.
///
/// Halts there rather than collecting every difference: past a divergence the
/// two runs are playing different games, so later deltas are consequences
/// rather than evidence.
pub fn first_divergence<G: GameRng + GameVersion>(
    left: &[Obs<G>],
    right: &[Obs<G>],
) -> Option<Divergence<G>> {
    for (step, (a, b)) in left.iter().zip(right).enumerate() {
        let deltas = compare(a, b);
        let rng = rng_deltas(a, b);
        if !deltas.is_empty() || !rng.is_empty() {
            return Some(Divergence { step, deltas, rng });
        }
    }
    if left.len() != right.len() {
        let step = left.len().min(right.len());
        return Some(Divergence {
            step,
            deltas: vec![Delta::new("trace.len", left.len(), right.len())],
            rng: vec![],
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BuildId, CombatPile, Provenance, RoomType, ScreenType, Sts1, Sts1Stream};
    use alloc::collections::BTreeMap;
    use alloc::string::ToString;

    fn base() -> Obs<Sts1> {
        Obs {
            provenance: Provenance {
                game: Sts1,
                game_version: BuildId(1),
                adapter: "sim".into(),
                mods: vec![],
            },
            screen: ScreenType::Room,
            room: Some(RoomType::Monster),
            legal: vec![],
            player_idx: 0,
            floor: 1,
            act: 1,
            hp: 80,
            max_hp: 80,
            gold: 99,
            energy: Some(3),
            turn: Some(1),
            block: Some(0),
            monsters: vec![],
            piles: BTreeMap::new(),
            powers: BTreeMap::new(),
            relics: vec![],
            potions: vec![],
            rng: BTreeMap::new(),
            done: false,
        }
    }

    #[test]
    fn identical_states_have_no_deltas() {
        assert!(compare(&base(), &base()).is_empty());
    }

    #[test]
    fn reports_the_field_and_both_values() {
        let (a, mut b) = (base(), base());
        b.hp = 74;
        let d = compare(&a, &b);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].path, "hp");
        assert_eq!(d[0].to_string(), "hp: 80 != 74");
    }

    /// The rule that keeps STS2 diffs usable: one side lacking a per-instance
    /// handle is not a disagreement.
    #[test]
    fn a_missing_field_is_not_a_disagreement() {
        let (mut a, mut b) = (base(), base());
        let with = Card {
            id: "Strike".into(),
            uid: Some(7),
            ..Card::default()
        };
        let without = Card {
            id: "Strike".into(),
            uid: None,
            ..Card::default()
        };
        a.piles.insert(Pile::Combat(CombatPile::Hand), vec![with]);
        b.piles
            .insert(Pile::Combat(CombatPile::Hand), vec![without]);
        assert!(compare(&a, &b).is_empty());

        // ...but two sides that *both* have one must agree.
        b.piles.insert(
            Pile::Combat(CombatPile::Hand),
            vec![Card {
                id: "Strike".into(),
                uid: Some(9),
                ..Card::default()
            }],
        );
        assert_eq!(compare(&a, &b).len(), 1);
    }

    #[test]
    fn optional_combat_fields_only_compare_when_both_present() {
        let (mut a, b) = (base(), base());
        a.energy = None;
        assert!(compare(&a, &b).is_empty());
    }

    #[test]
    fn rng_counters_localise_a_divergence() {
        let (mut a, mut b) = (base(), base());
        a.rng.insert(
            Sts1Stream::Shuffle,
            Stream {
                seed: 5,
                counter: 14,
                state: None,
            },
        );
        b.rng.insert(
            Sts1Stream::Shuffle,
            Stream {
                seed: 5,
                counter: 8,
                state: None,
            },
        );
        a.rng.insert(
            Sts1Stream::Ai,
            Stream {
                seed: 6,
                counter: 2,
                state: None,
            },
        );
        b.rng.insert(
            Sts1Stream::Ai,
            Stream {
                seed: 6,
                counter: 2,
                state: None,
            },
        );
        let r = rng_deltas(&a, &b);
        assert_eq!(r.len(), 1, "only the drifting stream is reported");
        assert_eq!(r[0].0, Sts1Stream::Shuffle);
    }

    #[test]
    fn first_divergence_halts_at_the_first_bad_step() {
        let good = base();
        let mut bad = base();
        bad.hp = 1;
        let left = vec![good.clone(), good.clone(), good.clone()];
        let right = vec![good.clone(), bad, good];
        let d = first_divergence(&left, &right).expect("should diverge");
        assert_eq!(d.step, 1);
    }

    #[test]
    fn differing_trace_lengths_are_a_divergence() {
        let g = base();
        let d = first_divergence(&[g.clone(), g.clone()], &[g]).expect("should diverge");
        assert_eq!(d.step, 1);
        assert_eq!(d.deltas[0].path, "trace.len");
    }

    #[test]
    fn a_different_card_suppresses_its_field_deltas() {
        let (mut a, mut b) = (base(), base());
        a.piles.insert(
            Pile::Combat(CombatPile::Hand),
            vec![Card {
                id: "Strike".into(),
                cost: Some(1),
                ..Card::default()
            }],
        );
        b.piles.insert(
            Pile::Combat(CombatPile::Hand),
            vec![Card {
                id: "Defend".into(),
                cost: Some(9),
                ..Card::default()
            }],
        );
        let d = compare(&a, &b);
        assert_eq!(d.len(), 1, "the id mismatch is the finding; cost is noise");
        assert!(d[0].path.ends_with(".id"));
    }
}
