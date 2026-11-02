//! The game's text, read from the player's own install.
//!
//! What a card, relic or power says is the game's own writing, so the
//! repository holds none of it. This copies the tables the front end reads
//! out of each supported build's `SlayTheSpire2.pck`, into `OUT_DIR`, and
//! sets `text_<build>` for each build it found. A build with no install here
//! builds without its text, and the front end shows ids and numbers instead.
//!
//! An install is a supported build when its `sts2.dll` has the hash that
//! build's lock under `mod/games/` pins. It is looked for in
//! `STS2_GAME_DIR`, in `STS2_STEAM_DIR`, and in
//! `$XDG_DATA_HOME/sts2/games/<build>`, where `mod/game.sh` puts the pinned
//! builds.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

/// The tables the front end reads, by the name the pack files them under.
const TABLES: [&str; 6] = [
    "cards",
    "relics",
    "powers",
    "potions",
    "enchantments",
    "afflictions",
];

fn main() {
    let manifest = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets it"));
    let locks = manifest.join("../../mod/games");
    println!("cargo::rerun-if-changed={}", locks.display());
    for var in ["STS2_GAME_DIR", "STS2_STEAM_DIR", "XDG_DATA_HOME", "HOME"] {
        println!("cargo::rerun-if-env-changed={var}");
    }

    let builds = pinned(&locks);
    let candidates = candidates(builds.keys());
    for (build, pin) in &builds {
        let cfg = format!("text_{}", build.replace('.', "_"));
        println!("cargo::rustc-check-cfg=cfg({cfg})");
        let Some(game) = candidates.iter().find(|game| holds(game, pin)) else {
            println!(
                "cargo::warning=no install of Slay the Spire 2 {build} found, so the terminal front end builds without its text. Set STS2_GAME_DIR to one."
            );
            continue;
        };
        let pack_path = game.join("SlayTheSpire2.pck");
        println!("cargo::rerun-if-changed={}", pack_path.display());
        let mut pack =
            sts2_pack::Pack::open(&pack_path).unwrap_or_else(|e| panic!("{build}'s pack: {e}"));
        let dir = out.join("text").join(build.replace('.', "_"));
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        for table in TABLES {
            let text = pack
                .text(&format!("localization/eng/{table}.json"))
                .unwrap_or_else(|e| panic!("{build}'s {table} text: {e}"));
            let path = dir.join(format!("{table}.json"));
            std::fs::write(&path, text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        }
        println!("cargo::rustc-cfg={cfg}");
    }
}

/// Each build a lock pins, and the hash of its `sts2.dll`.
fn pinned(locks: &Path) -> BTreeMap<String, String> {
    let entries = std::fs::read_dir(locks).unwrap_or_else(|e| panic!("{}: {e}", locks.display()));
    let mut builds = BTreeMap::new();
    for entry in entries {
        let path = entry.expect("a directory entry").path();
        let Some(build) = path
            .file_name()
            .and_then(|name| name.to_str()?.strip_suffix(".lock"))
        else {
            continue;
        };
        let lock =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let pin = (lock.lines())
            .filter(|line| !line.starts_with('#'))
            .find_map(|line| {
                let (hash, file) = line.split_once("  ")?;
                (file == "sts2.dll").then(|| hash.to_owned())
            })
            .unwrap_or_else(|| panic!("{} pins no sts2.dll", path.display()));
        builds.insert(build.to_owned(), pin);
    }
    builds
}

/// The installs to look in, in order.
fn candidates<'a>(builds: impl Iterator<Item = &'a String>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = ["STS2_GAME_DIR", "STS2_STEAM_DIR"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .collect();
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")));
    if let Some(data) = data {
        dirs.extend(builds.map(|build| data.join("sts2/games").join(build)));
    }
    dirs
}

/// Whether `game` is an install whose `sts2.dll` hashes to `pin`. The
/// assembly sits in the platform's `data_sts2_*` directory.
fn holds(game: &Path, pin: &str) -> bool {
    let Ok(entries) = std::fs::read_dir(game) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let named = entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with("data_sts2_"));
        named
            && std::fs::read(entry.path().join("sts2.dll")).is_ok_and(|dll| {
                let hash = Sha256::digest(&dll);
                hash.iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
                    == pin
            })
    })
}
