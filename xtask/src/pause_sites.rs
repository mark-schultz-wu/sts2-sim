//! `cargo xtask pause-sites`: where the game's own code can pause.
//!
//! A choice pauses where the game awaits `SignalPlayerChoiceBegun`, and the
//! simulator needs a resume point at each call that can lead there. This
//! lists them all from the game's assembly, so a hunt is not the only way
//! to find a missing one.
//!
//! `tools/pause_sites` reads the pinned `sts2.dll` with Mono.Cecil. It lists
//! each call in the content that pauses by itself (`pauses`). It also lists
//! each hook or other overridable method with such an override (`dispatches`).
//! Each build's list is `crates/sts2-golden/tests/pause-sites-<build>.txt`,
//! and the `pause_sites` test holds each model in it to a scenario.
//!
//! ```text
//! cargo xtask pause-sites                 # print the list
//! cargo xtask pause-sites --write         # rewrite the committed list
//! cargo xtask pause-sites --check         # fail where it differs
//! cargo xtask pause-sites --why Hooks.Hook.AfterTurnEnd
//! ```
//!
//! The build is `STS2_GAME_VERSION`'s.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{self, FileError, ProcessError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    File(#[from] FileError),
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error(transparent)]
    Version(#[from] crate::codegen::VersionError),
    #[error("no pinned assembly at {0}; `mod/game.sh` pins it")]
    Unpinned(PathBuf),
    #[error(
        "{0} differs from the game's pause sites; `cargo xtask pause-sites --write` rewrites it"
    )]
    Differs(PathBuf),
}

#[derive(clap::Args)]
pub struct Args {
    /// Rewrite the committed list.
    #[arg(long, conflicts_with_all = ["check", "why"])]
    write: bool,
    /// Fail where the committed list differs.
    #[arg(long, conflicts_with = "why")]
    check: bool,
    /// The shortest chain of calls from this method, less
    /// `MegaCrit.Sts2.Core.`, to the signal. It follows each virtual call
    /// into every override, and stops at a pause boundary. A call on the
    /// object itself reaches every override too, so a chain can name an
    /// override the object cannot be.
    #[arg(long)]
    why: Option<String>,
}

/// The committed list of a build's pause sites.
pub fn listed(root: &Path, version: &str) -> PathBuf {
    root.join("crates/sts2-golden/tests")
        .join(format!("pause-sites-{version}.txt"))
}

/// The assembly `mod/game.sh` pinned for a build, which the mods compile
/// against.
fn pinned(version: &str) -> Result<PathBuf, Error> {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".local/share")))
        .unwrap_or_default();
    let dll = data.join("sts2/pins").join(version).join("sts2.dll");
    match dll.is_file() {
        true => Ok(dll),
        false => Err(Error::Unpinned(dll)),
    }
}

/// `dotnet`, from `DOTNET_ROOT` or `~/.dotnet`, which the mods build with
/// too.
fn dotnet() -> Command {
    let root = std::env::var_os("DOTNET_ROOT")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".dotnet")))
        .unwrap_or_default();
    let mut command = Command::new(root.join("dotnet"));
    command
        .env("DOTNET_ROOT", &root)
        .env("DOTNET_CLI_TELEMETRY_OPTOUT", "1")
        .env("DOTNET_NOLOGO", "1");
    command
}

pub fn run(Args { write, check, why }: Args) -> Result<String, Error> {
    let root = crate::workspace();
    let version = crate::codegen::version()?;
    let dll = pinned(&version)?;
    let out = root.join("target/pause-sites");
    error::output(
        dotnet()
            .args(["build", "-c", "Release", "-v", "q", "--nologo", "-o"])
            .arg(&out)
            .arg(root.join("tools/pause_sites/pause_sites.csproj")),
    )?;
    let mut tool = dotnet();
    tool.arg(out.join("pause_sites.dll")).arg(&dll);
    if let Some(why) = &why {
        tool.args(["--why", why]);
    }
    let output = error::output(&mut tool)?;
    let found = String::from_utf8_lossy(&output.stdout).into_owned();
    let summary = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if why.is_some() {
        return Ok(found);
    }
    let path = listed(&root, &version);
    if write {
        error::write(&path, &found)?;
        return Ok(format!("{summary}, written to {}\n", path.display()));
    }
    if check {
        return match error::read(&path)? == found {
            true => Ok(format!("{summary}, as {} lists\n", path.display())),
            false => Err(Error::Differs(path)),
        };
    }
    Ok(found + &summary + "\n")
}
