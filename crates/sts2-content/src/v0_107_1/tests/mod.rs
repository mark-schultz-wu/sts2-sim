//! The generated tables, against the dumps they were generated from.
//!
//! One file per domain, each replaying its dump row by row. That is the whole
//! shape of these: if `cargo xtask codegen` was not re-run, or was run against
//! a dump from another build, the table describes a game nobody is talking to
//! and the replay is what notices.
//!
//! What a replay cannot notice is a claim the dump makes about the *game* — that
//! twenty powers work their type out, that fourteen encounters roll their
//! roster, that gold moves at Poverty and nowhere else. Those were measured by
//! the dump inside the running game and are invisible from here, so each file
//! pins them by name: a build that changes one has to say so in a test rather
//! than sail through.

mod acts;
mod afflictions;
mod ascension;
mod cards;
mod characters;
mod enchantments;
mod encounters;
mod events;
mod hooks;
mod modifiers;
mod monsters;
mod orbs;
mod potions;
mod powers;
mod relics;
mod version;

/// A dump's rows of one kind, split on whitespace, comments dropped.
///
/// Shared rather than written four times, which is what it was: the dumps have
/// one format and this is it. Row order is preserved because it is load-bearing
/// in three of the four — a monster's states are indexed by position, a
/// branch's order decides which move a roll produces, and a spawn's order is
/// which monster it is.
fn rows(dump: &'static str, kind: &'static str) -> impl Iterator<Item = Vec<&'static str>> {
    dump.lines()
        .filter(|l| !l.starts_with('#'))
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .filter(move |f| f.first() == Some(&kind))
}

/// The build a dump's `# build` header names.
///
/// Every domain checks this against its version module, which is the one rule
/// the whole arrangement rests on: a dump from another build describes a game
/// nobody is talking to, and a table regenerated against one is the failure the
/// check prevents.
///
/// Both hashes are compared, because the content one alone is not enough. It is
/// XxHash32 over the *set of model ids*, so it moves when content is added,
/// removed or renamed — and it stays put when a card's damage goes from 6 to 7.
/// A rebalance patch would slip past it with every generated number wrong. The
/// assembly hash is shipped in the game's `release_info.json` and moves on any
/// code change, which is what closes that.
fn build(dump: &'static str) -> super::Version {
    let header = dump
        .lines()
        .find_map(|l| l.strip_prefix("# build "))
        .expect("the dump has a `# build` header");

    let words: Vec<&str> = header.split_whitespace().collect();
    let field = |key: &str| -> &'static str {
        let at = words
            .iter()
            .position(|w| *w == key)
            .unwrap_or_else(|| panic!("the build header names no {key}: {header}"));
        // The slice is borrowed from a `&'static str`, so the word is too.
        header
            .split_whitespace()
            .nth(at + 1)
            .unwrap_or_else(|| panic!("the build header's {key} has no value: {header}"))
    };

    super::Version {
        name: header.split_whitespace().next().expect("a version string"),
        commit: field("commit"),
        assembly_hash: field("assembly").parse().expect("the assembly hash"),
        content_hash: field("content").parse().expect("the content hash"),
    }
}

/// The game's own spelling of a value: the serde rename.
///
/// There is no `id` accessor beside the rename — two spellings of one string is
/// a way for them to disagree — so this is how a test names a card, a monster,
/// an encounter or a power. It allocates, and it is only for tests; the wire is
/// what the simulator will talk over, and the wire is this.
fn wire<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value)
        .unwrap()
        .trim_matches('"')
        .to_string()
}
