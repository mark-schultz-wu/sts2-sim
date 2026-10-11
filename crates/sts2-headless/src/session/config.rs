//! Where the game is, and how it is launched.

use std::path::PathBuf;
use std::time::Duration;

/// The game's executable, inside [`Config::game_dir`].
pub(crate) const BINARY: &str = "SlayTheSpire2";

/// How to launch the game.
#[derive(Debug, Clone)]
pub struct Config {
    /// The directory holding the `SlayTheSpire2` binary. Also the working
    /// directory, which the game expects.
    pub game_dir: PathBuf,
    /// Let the game's own output through to this process's stdout and stderr.
    ///
    /// Off by default
    pub inherit_game_output: bool,
    /// How long to wait for the game to start and connect back.
    pub startup_timeout: Duration,
    /// How long to wait for a reply to one command.
    pub command_timeout: Duration,
    /// An `XDG_DATA_HOME` of the game's own, or `None` for the user's.
    ///
    /// Games that share a profile race on its saves. Each writes
    /// `progress.save.tmp` and renames it into place, so one renames the
    /// file another just did, and the game throws `SaveException`. A game
    /// with a home of its own gets a copy of the user's profile there as it
    /// launches. So the mods `deploy.sh` enabled are enabled.
    pub data_home: Option<PathBuf>,
}

impl Config {
    pub fn new(game_dir: impl Into<PathBuf>) -> Self {
        Self {
            game_dir: game_dir.into(),
            inherit_game_output: false,
            startup_timeout: Duration::from_secs(60),
            command_timeout: Duration::from_secs(30),
            data_home: None,
        }
    }

    /// Give the game a profile of its own. See [`Config::data_home`].
    pub fn data_home(mut self, dir: impl Into<PathBuf>) -> Self {
        self.data_home = Some(dir.into());
        self
    }

    /// Let the game's output through. See [`Config::inherit_game_output`].
    pub fn inherit_game_output(mut self, yes: bool) -> Self {
        self.inherit_game_output = yes;
        self
    }
}

/// The directory the game keeps its profiles under, inside an
/// `XDG_DATA_HOME`. Godot names it after the project.
pub(crate) const PROFILE: &str = "SlayTheSpire2";

/// The user's own `XDG_DATA_HOME`, where `deploy.sh` enables the mods.
pub(crate) fn user_data_home() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
}

/// Copies the user's profile into `home`, all but its logs, over whatever
/// an earlier launch left there.
pub(crate) fn seed_profile(home: &std::path::Path) -> std::io::Result<()> {
    let Some(user) = user_data_home() else {
        return Err(std::io::Error::other(
            "neither XDG_DATA_HOME nor HOME is set",
        ));
    };
    copy_tree(&user.join(PROFILE), &home.join(PROFILE))
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        if entry.file_name() == "logs" {
            continue;
        }
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Where the game is, if the environment says.
///
/// `STS2_GAME_DIR` is the same variable the justfile and `deploy.sh` read. A
/// shell set up to build the mod is already set up to launch it.
pub fn game_dir_from_env() -> Option<PathBuf> {
    std::env::var_os("STS2_GAME_DIR").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::copy_tree;

    /// The profile comes over whole, settings and saves alike, and the logs
    /// stay behind, since each game writes its own.
    #[test]
    fn a_profile_copies_all_but_its_logs() {
        let root = std::env::temp_dir().join(format!("sts2-profile-{}", std::process::id()));
        let from = root.join("from");
        let to = root.join("to");
        std::fs::create_dir_all(from.join("default/1")).expect("a profile directory");
        std::fs::create_dir_all(from.join("logs")).expect("a log directory");
        std::fs::write(from.join("default/1/settings.save"), "mods").expect("a settings file");
        std::fs::write(from.join("logs/godot.log"), "old").expect("a log file");

        copy_tree(&from, &to).expect("the profile copies");

        assert_eq!(
            std::fs::read_to_string(to.join("default/1/settings.save")).expect("settings came"),
            "mods"
        );
        assert!(!to.join("logs").exists(), "the logs stayed behind");
        std::fs::remove_dir_all(&root).expect("the scratch directory goes");
    }
}
