//! One run of the simulator at random, for `cargo xtask census`.
//!
//! ```text
//! random_run <seed> <character> [--avoid <step>:<move>]...
//! ```
//!
//! Every decision is a legal move, picked by a generator seeded from the seed,
//! so a run repeats exactly. Ending the turn is picked one time in ten while
//! anything else is legal. The run carries 25 Gorgets and a Mercury
//! Hourglass, so random play reaches content instead of dying early.
//! `--avoid` takes one move out of one step, by its place in the legal list.
//!
//! It prints `at <act> <floor>` as the floor changes, and `step <n> <move>
//! <moves>` before each decision and `done` after. The last line is `over
//! <won|lost>` or `capped`. An unported path panics, and the census reads the
//! panic off stderr, so nothing here catches one.

use std::io::Write as _;

use sts2_content::v0_107_1::V0_107_1;
use sts2_content::v0_107_1::relics::Relic;
use sts2_core::{Decision, Gift, Observed as _, Play as _, Seed, Setup, Start};
use sts2_sim::Sim;

type V = V0_107_1;

/// The relics every run is granted before it starts, and how many of each.
const CARRIED: [(Relic, usize); 2] = [(Relic::Gorget, 25), (Relic::MercuryHourglass, 1)];

/// Decisions a run takes before this gives up on it.
const STEPS: usize = 20_000;

/// SplitMix64, which is enough for picking among a handful of moves.
struct Pick(u64);

impl Pick {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [seed, character, rest @ ..] = &args[..] else {
        eprintln!("usage: random_run <seed> <character> [--avoid <step>:<move>]...");
        std::process::exit(2);
    };
    let mut avoid: Vec<(usize, usize)> = Vec::new();
    let mut rest = rest.iter();
    while let Some(flag) = rest.next() {
        let value = rest.next().filter(|_| flag == "--avoid");
        let parsed = value
            .and_then(|value| value.split_once(':'))
            .and_then(|(step, at)| Some((step.parse().ok()?, at.parse().ok()?)));
        match parsed {
            Some(pair) => avoid.push(pair),
            None => {
                eprintln!("expected --avoid <step>:<move>, not {flag}");
                std::process::exit(2);
            }
        }
    }
    let start = Start::<V> {
        seed: Seed::new(seed).expect("a seed the game accepts"),
        character: serde_json::from_value(serde_json::Value::String(character.clone()))
            .expect("a character id, such as REGENT"),
        ascension: sts2_content::v0_107_1::ascension::AscensionLevel::None,
        modifiers: Vec::new(),
    };
    let mut pick = Pick(
        seed.bytes()
            .fold(0, |at, b| at.wrapping_mul(31) ^ u64::from(b)),
    );
    let mut sim = Sim::<V>::begin((), &start).expect("a run to begin");
    for (relic, count) in CARRIED {
        for _ in 0..count {
            let granted = sim.construct(&Setup::Grant(Gift::Relic(relic)));
            if let Err(refused) = granted.expect("a relic to be granted") {
                panic!("the run refused {relic:?}: {refused:?}");
            }
        }
    }
    let mut out = std::io::stdout().lock();
    let mut last = None;

    for step in 0..STEPS {
        let run = sim.observed();
        let here = (run.act(), run.floor());
        if last != Some(here) {
            writeln!(out, "at {} {}", here.0, here.1).expect("stdout");
            last = Some(here);
        }
        if run.run_over() {
            let how = match run.victory() {
                Some(true) => "won",
                _ => "lost",
            };
            writeln!(out, "over {how}").expect("stdout");
            return;
        }
        let legal = sim.legal();
        if legal.is_empty() {
            panic!("a run with nothing legal to do");
        }
        // The moves this step picks from, by their place in the legal list.
        let open: Vec<usize> = (0..legal.len())
            .filter(|at| !avoid.contains(&(step, *at)))
            .collect();
        let others: Vec<usize> = open
            .iter()
            .copied()
            .filter(|&at| !matches!(legal[at], Decision::EndTurn))
            .collect();
        let at = match open
            .iter()
            .find(|&&at| matches!(legal[at], Decision::EndTurn))
        {
            Some(&end) if others.is_empty() || pick.below(10) == 0 => end,
            _ if others.is_empty() => panic!("every move of step {step} was avoided"),
            _ => others[pick.below(others.len())],
        };
        writeln!(out, "step {step} {at} {}", legal.len()).expect("stdout");
        out.flush().expect("stdout");
        let decision = &legal[at];
        if let Err(refused) = sim.decide(decision).expect("a legal move to apply") {
            panic!("the run refused its own legal move {decision:?}: {refused:?}");
        }
        writeln!(out, "done").expect("stdout");
    }
    writeln!(out, "capped").expect("stdout");
}
