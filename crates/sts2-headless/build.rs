//! Works out which mod build this client expects to talk to.
//!
//! The answer is a hash of the mod's source, produced by
//! `mod/sts2_headless/identity.sh`. That is the same script the mod's own build
//! runs to compile the value into its greeting. One hasher invoked twice, rather
//! than one here and another in MSBuild. Two implementations of "the same hash"
//! are two things that can disagree, and the symptom of disagreeing is that
//! every mod looks stale.
//!
//! It has to be derived from *source* rather than read from a build artifact. A
//! committed hash file leaves the client and a stale deployed mod agreeing on
//! the same old value. That is exactly the skew this exists to catch.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let mod_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../mod/sts2_headless")
        .canonicalize()
        .expect("the mod's source should sit beside this crate in the workspace");

    // Per file rather than on the directory: a directory's mtime does not move
    // when a file inside it is edited in place, so cargo would cache a stale
    // identity through exactly the edit that changed it.
    watch(&mod_dir);

    let script = mod_dir.join("identity.sh");
    let out = Command::new("sh")
        .arg(&script)
        .output()
        .unwrap_or_else(|e| panic!("running {}: {e}", script.display()));

    assert!(
        out.status.success(),
        "{} failed: {}",
        script.display(),
        String::from_utf8_lossy(&out.stderr)
    );

    let identity = String::from_utf8(out.stdout)
        .expect("a hash is ascii")
        .trim()
        .to_string();
    assert!(!identity.is_empty(), "{} printed nothing", script.display());

    let dest = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"))
        .join("mod_identity.rs");
    std::fs::write(
        &dest,
        format!("pub const MOD_IDENTITY: &str = \"{identity}\";\n"),
    )
    .unwrap_or_else(|e| panic!("writing {}: {e}", dest.display()));
}

/// Rebuild when any of the mod's sources change, the script included.
fn watch(dir: &Path) {
    println!("cargo:rerun-if-changed={}", dir.display());
    for entry in std::fs::read_dir(dir).expect("the mod's source should be readable") {
        let path = entry.expect("a readable directory entry").path();
        if let Some("cs" | "sh") = path.extension().and_then(|e| e.to_str()) {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}
