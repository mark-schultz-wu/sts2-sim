//! The generated tables, against the dumps they were generated from.
//!
//! One file per domain, each replaying its dump row by row. That is the whole
//! shape of these. If `cargo xtask codegen` was not re-run, or was run against a
//! dump from another build, the table describes a game nobody is talking to. The
//! replay is what notices.
//!
//! What a replay cannot notice is a claim the dump makes about the *game*. That
//! twenty-two powers work their type out, that fourteen encounters roll their
//! roster, that gold moves at Poverty and nowhere else. Those were measured by
//! the dump inside the running game and are invisible from here. Each file pins
//! them by name, so a build that changes one has to say so in a test rather than
//! sail through.
//!
//! This suite is a port of `v0_107_1`'s, not a copy of it. Twenty-five of its
//! assertions had to be read off this build instead. Every one of those is a
//! fact about the game that moved.

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
mod restsite;
mod runstate;
mod version;

/// A dump's rows of one kind, split on whitespace, comments dropped.
///
/// Shared rather than written four times, which is what it was: the dumps have
/// one format and this is it.
///
/// Row order is preserved because it is load-bearing in three of the four. A
/// monster's states are indexed by position, a branch's order decides which move
/// a roll produces, and a spawn's order is which monster it is.
fn rows(dump: &'static str, kind: &'static str) -> impl Iterator<Item = Vec<&'static str>> {
    dump.lines()
        .filter(|l| !l.starts_with('#'))
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .filter(move |f| f.first() == Some(&kind))
}

/// The build a dump's `# build` header names.
///
/// Every domain checks this against its version module, which is the one rule
/// the whole arrangement rests on. A table regenerated against another build's
/// dump is the failure this prevents.
///
/// Both hashes are compared, because the content one alone is not enough. The
/// xtask README says what each moves on.
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
/// There is no `id` accessor beside the rename, because two spellings of one
/// string is a way for them to disagree. This is how a test names a card, a
/// monster, an encounter or a power. It allocates, and it is only for tests. The
/// wire is what the simulator talks over, and the wire is this.
fn wire<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value)
        .unwrap()
        .trim_matches('"')
        .to_string()
}
