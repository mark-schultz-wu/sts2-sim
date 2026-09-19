//! How long each list in a `Playable` gets, across the recorded runs.
//!
//! `legal::hand` builds one `Playable` per card in hand on every report, and
//! each carries three heap lists. A fixed cap needs a measurement, so this
//! replays every recording and prints what the lists reach.

use std::collections::BTreeMap;

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Construct, Play, Step};
use sts2_golden::bench::reached;
use sts2_golden::{every_run, recording};
use sts2_sim::Sim;

type V = V0_107_1;

fn main() {
    let mut targets: BTreeMap<usize, usize> = BTreeMap::new();
    let mut effect: BTreeMap<usize, usize> = BTreeMap::new();
    let mut reasons: BTreeMap<u32, usize> = BTreeMap::new();
    let mut creatures: BTreeMap<usize, usize> = BTreeMap::new();
    let mut hand: BTreeMap<usize, usize> = BTreeMap::new();
    let mut belt: BTreeMap<usize, usize> = BTreeMap::new();

    for name in every_run() {
        let recorded = recording::<V>(&name);
        let start = recorded.start().clone();
        let steps: Vec<Step<V>> = recorded.steps().to_vec();
        let supported = reached::<V, Sim<V>>(&start, &steps);
        let mut playing = Sim::<V>::begin((), &start).expect("a run to begin");
        for step in &steps[..supported] {
            let seen = playing.step(step).expect("a step to apply").expect("legal");
            for held in &seen.observation.potions {
                *belt.entry(held.targets.len()).or_default() += 1;
            }
            let Some(combat) = seen.observation.combat.as_ref() else {
                continue;
            };
            *creatures.entry(combat.creatures.len()).or_default() += 1;
            *hand.entry(combat.legal.len()).or_default() += 1;
            for card in &combat.legal {
                *targets.entry(card.targets.len()).or_default() += 1;
                *effect.entry(card.effect.len()).or_default() += 1;
                let set = card.reason.map_or(0, |reason| {
                    use sts2_core::FlagSet;
                    <<V as sts2_core::ContentVersion>::UnplayableReasons as FlagSet>::MEMBERS
                        .iter()
                        .filter(|&&member| reason.contains(member))
                        .count() as u32
                });
                *reasons.entry(set).or_default() += 1;
            }
        }
    }

    for (what, counts) in [
        ("creatures in a fight", &creatures),
        ("cards in hand", &hand),
        ("targets per card", &targets),
        ("previews per card", &effect),
        ("targets per potion", &belt),
    ] {
        let total: usize = counts.values().sum();
        let line: Vec<String> = counts
            .iter()
            .map(|(len, hits)| format!("{len}:{}%", hits * 100 / total.max(1)))
            .collect();
        println!("{what:22} {}", line.join("  "));
    }
    let total: usize = reasons.values().sum();
    let line: Vec<String> = reasons
        .iter()
        .map(|(len, hits)| format!("{len}:{}%", hits * 100 / total.max(1)))
        .collect();
    println!("{:22} {}", "reasons per card", line.join("  "));
}
