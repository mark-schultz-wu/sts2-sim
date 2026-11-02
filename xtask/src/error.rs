//! The failures every command shares: a file that cannot be read or written,
//! and a program that cannot be run or that fails.
//!
//! Each command's own failures are its module's `Error`. These two are the
//! ones every module wraps.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Output};

/// What was being done to a file.
#[derive(Debug, Clone, Copy)]
pub enum Verb {
    Reading,
    Writing,
    Creating,
    Removing,
}

impl std::fmt::Display for Verb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Verb::Reading => "reading",
            Verb::Writing => "writing",
            Verb::Creating => "creating",
            Verb::Removing => "removing",
        })
    }
}

/// A file or directory the filesystem refused.
#[derive(Debug, thiserror::Error)]
#[error("{verb} {}: {source}", path.display())]
pub struct FileError {
    pub verb: Verb,
    pub path: PathBuf,
    #[source]
    pub source: std::io::Error,
}

impl FileError {
    fn at(verb: Verb, path: &Path) -> impl FnOnce(std::io::Error) -> FileError + '_ {
        move |source| FileError {
            verb,
            path: path.to_path_buf(),
            source,
        }
    }
}

/// `fs::read_to_string`.
pub fn read(path: &Path) -> Result<String, FileError> {
    std::fs::read_to_string(path).map_err(FileError::at(Verb::Reading, path))
}

/// `fs::write`.
pub fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), FileError> {
    std::fs::write(path, contents).map_err(FileError::at(Verb::Writing, path))
}

/// `fs::create_dir_all`.
pub fn create_dir(path: &Path) -> Result<(), FileError> {
    std::fs::create_dir_all(path).map_err(FileError::at(Verb::Creating, path))
}

/// `fs::remove_dir_all`, where the directory exists.
pub fn remove_dir(path: &Path) -> Result<(), FileError> {
    match path.is_dir() {
        true => std::fs::remove_dir_all(path).map_err(FileError::at(Verb::Removing, path)),
        false => Ok(()),
    }
}

/// `File::create`.
pub fn create(path: &Path) -> Result<std::fs::File, FileError> {
    std::fs::File::create(path).map_err(FileError::at(Verb::Creating, path))
}

/// Every entry of a directory, as paths.
pub fn entries(dir: &Path) -> Result<Vec<PathBuf>, FileError> {
    std::fs::read_dir(dir)
        .map_err(FileError::at(Verb::Reading, dir))?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(FileError::at(Verb::Reading, dir))
        })
        .collect()
}

/// A program that did not start, or that finished badly.
#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("running {program}, which this needs installed: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{program} failed with {status}\n{stderr}")]
    Failed {
        program: String,
        status: ExitStatus,
        stderr: String,
    },
    #[error("{what}: {source}")]
    Io {
        what: &'static str,
        #[source]
        source: std::io::Error,
    },
}

/// The program a command runs, by the name a message calls it.
fn program(command: &Command) -> String {
    command.get_program().to_string_lossy().into_owned()
}

/// Run to completion with the output inherited, and fail on a bad status.
pub fn status(command: &mut Command) -> Result<(), ProcessError> {
    let status = command.status().map_err(|source| ProcessError::Spawn {
        program: program(command),
        source,
    })?;
    match status.success() {
        true => Ok(()),
        false => Err(ProcessError::Failed {
            program: program(command),
            status,
            stderr: String::new(),
        }),
    }
}

/// Run to completion capturing the output, and fail on a bad status.
pub fn output(command: &mut Command) -> Result<Output, ProcessError> {
    let output = spawned(command)?;
    match output.status.success() {
        true => Ok(output),
        false => Err(ProcessError::Failed {
            program: program(command),
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        }),
    }
}

/// Run to completion capturing the output, whatever the status.
pub fn spawned(command: &mut Command) -> Result<Output, ProcessError> {
    command.output().map_err(|source| ProcessError::Spawn {
        program: program(command),
        source,
    })
}

/// Build one of `sts2-golden`'s examples in release, and where it landed.
///
/// The build leaves out `STS2_GAME_DIR` and `STS2_GAME_VERSION`. The examples
/// read them as they run. `sts2-golden`'s build script reads them too, and
/// with another build's game named, it asks that game for every v0.107.1
/// scenario.
/// `sts2-run`, the one runner, built for release. See `docs/RUNNERS.md`.
pub fn build_runner(root: &Path) -> Result<PathBuf, ProcessError> {
    status(
        Command::new("cargo")
            .args([
                "build",
                "-q",
                "--release",
                "-p",
                "sts2-run",
                "--bin",
                "sts2-run",
            ])
            .current_dir(root),
    )?;
    Ok(root.join("target/release/sts2-run"))
}

/// The runs merger, built as it is needed. A record or a hunt runs for
/// hours, and the nightly sweep can delete a binary built when it began.
pub fn merger(root: &Path) -> Result<PathBuf, ProcessError> {
    build_example(root, "merge_runs")
}

pub fn build_example(root: &Path, name: &str) -> Result<PathBuf, ProcessError> {
    status(
        Command::new("cargo")
            .args(["build", "-q", "--release", "-p", "sts2-golden", "--example"])
            .arg(name)
            .env_remove("STS2_GAME_DIR")
            .env_remove("STS2_GAME_VERSION")
            .current_dir(root),
    )?;
    Ok(root.join("target/release/examples").join(name))
}
