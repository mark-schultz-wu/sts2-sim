//! Each model the game's own code can pause in, held to a scenario.
//!
//! `cargo xtask pause-sites --write` lists, from the game's assembly, each
//! call that can pause on a choice, as `pause-sites-<build>.txt`. A model
//! named there needs a resume point in the simulator, and a scenario that
//! names the model is where the game shows it. So a scenario names each
//! one, in a command or in its file name. Hefty Tablet comes from Neow, so
//! its scenario names it in the file name alone.
//!
//! Scenarios are played on v0.107.1 alone. A model that pauses only in
//! v0.111.0 is listed in [`V0_111_0_ONLY`], which the hunts cover instead.

use std::collections::BTreeSet;
use std::path::Path;

/// The models a single-player run never holds. Massive Scroll drops only
/// where `runState.Players.Count > 1`.
const UNREACHABLE: &[&str] = &["MASSIVE_SCROLL"];

/// The models that pause in v0.111.0 and not in v0.107.1.
const V0_111_0_ONLY: &[&str] = &["ABUNDANCE", "TUTOR", "TOASTY_MITTENS"];

/// The models one build's list names as pausing by themselves, by kind and
/// id: the class name in capitals, its words parted by underscores. A base
/// model class is not content, and neither is a command.
fn pausing(build: &str) -> BTreeSet<(String, String)> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join(format!("pause-sites-{build}.txt"));
    let list = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    list.lines()
        .filter_map(|line| line.strip_prefix("pauses Models."))
        .filter_map(|line| {
            let owner = line.split(' ').next()?;
            let mut parts = owner.rsplitn(3, '.');
            let _method = parts.next()?;
            let class = parts.next()?;
            let kind = parts.next()?;
            (!class.ends_with("Model")).then(|| (kind.to_owned(), id(class)))
        })
        .collect()
}

/// `SculptingStrike` as `SCULPTING_STRIKE`.
fn id(class: &str) -> String {
    let mut id = String::new();
    let mut last: Option<char> = None;
    for c in class.chars() {
        if c.is_ascii_uppercase()
            && last.is_some_and(|last| last.is_ascii_lowercase() || last.is_ascii_digit())
        {
            id.push('_');
        }
        id.push(c.to_ascii_uppercase());
        last = Some(c);
    }
    id
}

/// Every scenario's file name and text, joined.
fn scenarios() -> String {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/scenarios");
    let mut text = String::new();
    for entry in
        std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
    {
        let path = entry.expect("a directory entry").path();
        if path.extension().is_some_and(|ext| ext == "txt") {
            let name = path.file_stem().expect("a scenario has a name");
            text.push_str(&format!("-{}-\n", name.to_string_lossy()));
            text.push_str(&std::fs::read_to_string(&path).expect("a scenario reads"));
        }
    }
    text
}

#[test]
fn every_model_that_pauses_is_named_in_a_scenario() {
    let text = scenarios();
    // A monster is named by its encounter, whose id starts with its own. A
    // file name names a model in lower case, with hyphens, and Wish is
    // plural in Sere Talon's.
    let named = |(kind, id): &(String, String)| {
        let file = id.to_lowercase().replace('_', "-");
        text.contains(&format!("\"{id}\""))
            || (kind == "Monsters" && text.contains(&format!("\"{id}_")))
            || text.contains(&format!("-{file}-"))
            || text.contains(&format!("-{file}es-"))
    };
    let missing: Vec<String> = pausing("v0.107.1")
        .into_iter()
        .filter(|model| !named(model) && !UNREACHABLE.contains(&model.1.as_str()))
        .map(|(_, id)| id)
        .collect();
    assert!(
        missing.is_empty(),
        "no scenario names these models, which can pause: {missing:?}"
    );
}

#[test]
fn every_model_that_pauses_in_v0_111_0_alone_is_known() {
    let older = pausing("v0.107.1");
    let only: Vec<String> = pausing("v0.111.0")
        .into_iter()
        .filter(|model| !older.contains(model))
        .map(|(_, id)| id)
        .collect();
    assert_eq!(
        only, V0_111_0_ONLY,
        "the models that pause in v0.111.0 alone changed"
    );
}

#[test]
fn ids_split_at_each_word() {
    assert_eq!(id("SculptingStrike"), "SCULPTING_STRIKE");
    assert_eq!(id("BiiigHug"), "BIIIG_HUG");
    assert_eq!(id("StratagemPower"), "STRATAGEM_POWER");
}
