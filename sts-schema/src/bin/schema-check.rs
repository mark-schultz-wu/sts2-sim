//! Validate that a producer's JSON conforms to `sts-schema`.
//!
//! These types are the source of truth for the wire format, so anything
//! emitting observations is held to them here rather than discovering a
//! mismatch inside a differential test, where it would look like a simulator
//! bug.
//!
//!     <producer> | cargo run -q -p sts-schema --bin schema-check
//!
//! Reads one observation per line, and stops at the first failure.
//!
//! TODO: no producer exists on this branch yet. Revisit when the adapters
//! land — the invocation above is a placeholder.

use std::io::{self, BufRead};

use sts_schema::provenance::GameVersion;
use sts_schema::{GameRng, Obs, Sts1, Sts2, diff};

#[derive(Default)]
struct Tally {
    n: usize,
    combat: usize,
    with_rng: usize,
    with_uid: usize,
    provenance: Option<String>,
}

/// Everything that does not depend on which game produced the line.
fn tally<G: GameRng + GameVersion>(obs: &Obs<G>, t: &mut Tally) -> Result<(), String> {
    t.n += 1;
    if obs.in_combat() {
        t.combat += 1;
    }
    if !obs.rng.is_empty() {
        t.with_rng += 1;
    }
    if obs.piles.values().flatten().any(|c| c.uid.is_some()) {
        t.with_uid += 1;
    }
    // Exercises the diff over real shapes, not the hand-built fixtures.
    let self_deltas = diff::compare(obs, obs);
    if !self_deltas.is_empty() {
        return Err(format!(
            "an observation differs from itself: {self_deltas:?}"
        ));
    }
    t.provenance = Some(format!("{:?}", obs.provenance));
    Ok(())
}

/// The game is not known until the line is read, and the [`Game`] marker is the
/// tag: exactly one of these parses, because each rejects the other's `game`.
///
/// [`Game`]: sts_schema::Game
fn check_line(line: &str, t: &mut Tally) -> Result<(), String> {
    let sts1 = serde_json::from_str::<Obs<Sts1>>(line);
    let sts2 = serde_json::from_str::<Obs<Sts2>>(line);
    match (sts1, sts2) {
        (Ok(o), _) => tally(&o, t),
        (_, Ok(o)) => tally(&o, t),
        // Both errors, since neither alone says which game was intended.
        (Err(a), Err(b)) => Err(format!("as sts1: {a}\n  as sts2: {b}")),
    }
}

fn main() {
    let mut t = Tally::default();
    for (i, line) in io::stdin().lock().lines().enumerate() {
        let line = line.expect("stdin");
        if line.trim().is_empty() {
            continue;
        }
        if let Err(e) = check_line(&line, &mut t) {
            eprintln!("line {}: does not conform: {e}", i + 1);
            eprintln!("  {}", &line[..line.len().min(300)]);
            std::process::exit(1);
        }
    }

    if t.n == 0 {
        eprintln!("no observations read");
        std::process::exit(1);
    }
    println!("conforms: {} observations", t.n);
    if let Some(p) = t.provenance {
        println!("  provenance    {p}");
    }
    println!("  combat        {}", t.combat);
    println!("  with rng      {}", t.with_rng);
    println!("  with card uid {}", t.with_uid);
}
