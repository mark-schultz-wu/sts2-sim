//! What Neow offers on each of a range of seeds, for `cargo xtask neow`.
//!
//! ```text
//! neow_offers <character> <first> <count>
//! ```
//!
//! Seeds are the census's, `C` and nine digits, from `first` for `count` of
//! them. Each prints one line: the seed, then each relic Neow offers in the
//! order the options number them.

use sts2_content::v0_107_1::V0_107_1;
use sts2_core::{Play as _, Room, Seed, Start};
use sts2_sim::Sim;

type V = V0_107_1;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [character, first, count] = &args[..] else {
        eprintln!("usage: neow_offers <character> <first> <count>");
        std::process::exit(2);
    };
    let first: u64 = first.parse().expect("a first seed number");
    let count: u64 = count.parse().expect("a count of seeds");
    let character = serde_json::from_value(serde_json::Value::String(character.clone()))
        .expect("a character id, such as REGENT");
    for n in first..first + count {
        let seed = format!("C{n:09}");
        let start = Start::<V> {
            seed: Seed::new(&seed).expect("a seed the game accepts"),
            character,
            ascension: sts2_content::v0_107_1::ascension::AscensionLevel::None,
            modifiers: Vec::new(),
        };
        let mut sim = Sim::<V>::begin((), &start).expect("a run to begin");
        let seen = sim.observe().expect("an opening report");
        let Some(Room::Event { options, .. }) = seen.observation.room else {
            panic!("{seed} opens somewhere other than Neow");
        };
        let offered: Vec<String> = options
            .iter()
            .map(|option| match option.relic {
                Some(relic) => serde_json::to_value(relic)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_else(|| format!("{relic:?}")),
                None => "-".to_string(),
            })
            .collect();
        println!("{seed} {}", offered.join(" "));
    }
}
