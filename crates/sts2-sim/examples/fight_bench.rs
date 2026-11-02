//! Random-play fights per second, on spire-mentor's W1 workload.
//!
//! Ironclad, ascension 0, the starter deck and Burning Blood, against the 22
//! Act 1 Overgrowth encounters round-robin: seed `s` fights encounter
//! `s % 22`. Each seed's start state is built once, untimed. Every fight
//! clones it, timed, and plays uniformly random legal moves until the fight
//! is over. One thread.
//!
//! ```text
//! cargo run --release -p sts2-sim --example fight_bench -- --seeds 2200 --reps 50
//! ```

#![expect(clippy::disallowed_types, reason = "a bench spells its own seeds")]

use std::fmt::Write as _;
use std::time::Instant;

use clap::Parser;

use sts2_content::ids::CharacterIds as _;
use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Observed as _, Setup, Start};
use sts2_sim::Sim;

type V = V0_107_1;
type Encounter = <V as sts2_core::ContentVersion>::Encounter;

#[derive(Parser)]
struct Cli {
    /// How many start states, each a seed and an encounter.
    #[arg(long, default_value_t = 2200)]
    seeds: u64,
    /// How many fights each start state plays.
    #[arg(long, default_value_t = 50)]
    reps: u64,
}

/// spire-mentor's `w1_encounters.txt`, in its order.
fn encounters() -> Vec<Encounter> {
    use sts2_content::v0_107_1::encounters::Encounter as E;
    vec![
        E::ShrinkerBeetleWeak,
        E::FuzzyWurmCrawlerWeak,
        E::NibbitsWeak,
        E::SlimesWeak,
        E::CubexConstructNormal,
        E::FlyconidNormal,
        E::FogmogNormal,
        E::InkletsNormal,
        E::MawlerNormal,
        E::NibbitsNormal,
        E::OvergrowthCrawlers,
        E::RubyRaidersNormal,
        E::SlimesNormal,
        E::SlitheringStranglerNormal,
        E::SnappingJaxfruitNormal,
        E::VineShamblerNormal,
        E::BygoneEffigyElite,
        E::ByrdonisElite,
        E::PhrogParasiteElite,
        E::VantomBoss,
        E::CeremonialBeastBoss,
        E::TheKinBoss,
    ]
}

/// A small, fast generator for the random player.
struct SplitMix(u64);

impl SplitMix {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        ((u128::from(z) * n as u128) >> 64) as usize
    }
}

fn main() {
    let cli = Cli::parse();
    let encounters = encounters();
    let character = <V as sts2_core::ContentVersion>::Character::IRONCLAD;
    let starts: Vec<Sim<V>> = (0..cli.seeds)
        .map(|s| {
            let mut seed = String::from("B");
            write!(seed, "{s:07}").expect("a string takes the digits");
            let start = Start {
                seed: sts2_core::Seed::new(&seed).expect("a seed"),
                players: vec![sts2_core::Player { character, wins: 0 }],
                ascension: sts2_content::generated::ascension::AscensionLevel::None,
                modifiers: Vec::new(),
            };
            let mut sim = Sim::<V>::begin(&start).expect("a run starts");
            let encounter = encounters[(s % encounters.len() as u64) as usize];
            sim.construct(&Setup::Fight { encounter })
                .expect("no fault")
                .expect("the fight starts");
            sim
        })
        .collect();

    let mut rng = SplitMix(1);
    let (mut fights, mut actions, mut wins) = (0u64, 0u64, 0u64);
    let mut digest = 0xcbf2_9ce4_8422_2325u64;
    let mut sim = starts[0].clone();
    let mut legal = sts2_sim::Legal::new();
    let began = Instant::now();
    for _ in 0..cli.reps {
        for start in &starts {
            sim.clone_from(start);
            loop {
                let observed = sim.observed();
                if !observed.in_combat() || observed.run_over() {
                    break;
                }
                sim.legal_into(&mut legal);
                let pick = &legal[rng.below(legal.len())];
                sim.decide(pick).expect("no fault").expect("a legal move");
                actions += 1;
            }
            fights += 1;
            let observed = sim.observed();
            if observed.alive() {
                wins += 1;
            }
            digest = (digest ^ observed.hp() as u64).wrapping_mul(0x100_0000_01b3);
        }
    }
    let seconds = began.elapsed().as_secs_f64();
    println!(
        "{{\"fights\":{fights},\"actions\":{actions},\"wins\":{wins},\"seconds\":{seconds:.3},\
         \"fights_per_s\":{:.0},\"actions_per_s\":{:.0},\"ns_per_action\":{:.1},\
         \"actions_per_fight\":{:.2},\"digest\":\"{digest:016x}\"}}",
        fights as f64 / seconds,
        actions as f64 / seconds,
        seconds * 1e9 / actions as f64,
        actions as f64 / fights as f64,
    );
}
