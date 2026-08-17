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

mod cards;
mod characters;
mod enchantments;
mod encounters;
mod events;
mod monsters;
mod potions;
mod powers;
mod relics;

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

/// The content hash a dump's header names.
///
/// Every domain checks this against its version module, which is the one rule
/// the whole arrangement rests on: a dump from another build has another hash,
/// and a table regenerated against one is the failure the check prevents.
fn dump_hash(dump: &'static str) -> u32 {
    dump.lines()
        .next()
        .expect("the dump has a header")
        .rsplit_once(' ')
        .expect("the header names a content hash")
        .1
        .parse()
        .expect("the content hash is a number")
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
