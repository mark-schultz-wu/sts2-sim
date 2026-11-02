//! Where a run gets written when nobody says where.
//!
//! All that is left of what used to be this crate's `session` module. The rest
//! of it was never a front-end concern: the socket, the daemon, the request
//! envelope. It is how *the oracle* stays alive between two invocations, and a
//! simulator has no such problem. It lives in `sts2-headless` now, behind
//! [`sts2_headless::Session`], and this front end holds a
//! [`RunTable`](sts2_core::RunTable) like any other caller.

use std::path::PathBuf;

/// Where runs are written when nobody says where.
///
/// Under the state directory rather than the working directory. A recording is
/// kept by default, so it is a file the user did not ask for. Not one to tidy
/// out of a repository. `sts2 run status` prints the path of the one in
/// progress, and `sts2 run start --record` overrides it.
pub fn runs_dir() -> PathBuf {
    match std::env::var_os("XDG_STATE_HOME") {
        Some(dir) => PathBuf::from(dir).join("sts2/runs"),
        None => match std::env::var_os("HOME") {
            Some(home) => PathBuf::from(home).join(".local/state/sts2/runs"),
            None => PathBuf::from(format!("/tmp/sts2-{}-runs", uid())),
        },
    }
}

/// The user, for keeping two people's runs apart under `/tmp`.
fn uid() -> String {
    std::env::var("UID")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "shared".into())
}
