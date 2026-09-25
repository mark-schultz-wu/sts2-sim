//! What each seed's first unknown room rolls, for `cargo xtask unknown`.
//!
//! ```text
//! unknown_rolls <character> <first> <count>
//! ```
//!
//! Seeds are the census's, `C` and nine digits. Each prints the seed, the
//! first draw of the `UnknownMapPoint` stream, and every unknown point of the
//! act's first three rows. The draw is read off a copy of the stream, so the
//! run it came from is not moved.

use sts2_content::v0_107_1::V0_107_1;
use sts2_content::v0_107_1::runstate::RunRngType;
use sts2_core::rng::GameRng as _;
use sts2_core::{Play as _, Seed, Start};
use sts2_sim::Sim;
use sts2_sim::rng::MegaRng;

type V = V0_107_1;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [character, first, count] = &args[..] else {
        eprintln!("usage: unknown_rolls <character> <first> <count>");
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
        let position = seen.observation.rng.run[&RunRngType::UnknownMapPoint];
        let words = position.words.expect("the simulator reports its words");
        let counter = i32::try_from(position.counter).expect("a count fits the generator");
        let roll = MegaRng::from_words(words, counter).next_double() as f32;
        let unknown: Vec<String> = seen
            .observation
            .map
            .points
            .iter()
            .filter(|node| {
                node.at.row <= 3 && node.kind == <_ as sts2_core::MapPointKinds>::UNKNOWN
            })
            .map(|node| format!("{},{}", node.at.col, node.at.row))
            .collect();
        println!("{seed} {roll:.4} {}", unknown.join(" "));
    }
}
