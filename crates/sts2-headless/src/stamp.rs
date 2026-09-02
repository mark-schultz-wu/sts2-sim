//! Which build something was made against.
//!
//! [`Version`] identifies a build with borrowed strings, which is right for a
//! constant and useless for anything read back. A greeting off the wire and a
//! recording off disk both arrive as owned data. This is that shape, and it is
//! also the one place that decides what "the same build" means.

use serde::{Deserialize, Serialize};
use std::fmt;
use sts2_core::Version;

/// A build, as the four values that name one.
///
/// Field names are the greeting's, so a greeting deserializes into one and a
/// recording's header is written in the same spelling. One shape rather than
/// two that drift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stamp {
    /// The build's own version string. A label, not an identity.
    pub game: String,
    pub commit: String,
    #[serde(rename = "assemblyHash")]
    pub assembly_hash: i64,
    #[serde(rename = "contentHash")]
    pub content_hash: u32,
}

impl Stamp {
    pub fn of(version: &Version) -> Self {
        Self {
            game: version.name.to_string(),
            commit: version.commit.to_string(),
            assembly_hash: version.assembly_hash,
            content_hash: version.content_hash,
        }
    }

    /// Whether this is the build a set of content tables describes.
    ///
    /// Both hashes, because either alone lets something through. The xtask
    /// README says what each one moves on.
    ///
    /// The name and commit are not compared. They are labels the build reports
    /// about itself, and a build whose hashes agree is the build whatever it
    /// chooses to call itself.
    pub fn describes(&self, version: &Version) -> bool {
        self.content_hash == version.content_hash && self.assembly_hash == version.assembly_hash
    }
}

impl fmt::Display for Stamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}), assembly {}, content {}",
            self.game, self.commit, self.assembly_hash, self.content_hash
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_content::v0_107_1::V0_107_1;
    use sts2_core::ContentVersion;

    fn ours() -> Stamp {
        Stamp::of(&V0_107_1::VERSION)
    }

    #[test]
    fn a_build_describes_itself() {
        assert!(ours().describes(&V0_107_1::VERSION));
    }

    /// A patch that adds or renames content moves this one.
    #[test]
    fn a_different_content_hash_is_a_different_build() {
        let mut other = ours();
        other.content_hash += 1;
        assert!(!other.describes(&V0_107_1::VERSION));
    }

    /// And a rebalance that touches no id moves only this one, which is why
    /// both are compared.
    #[test]
    fn a_different_assembly_hash_is_a_different_build() {
        let mut other = ours();
        other.assembly_hash += 1;
        assert!(!other.describes(&V0_107_1::VERSION));
    }

    /// The labels are what the build calls itself, not what it is.
    #[test]
    fn the_labels_are_not_the_identity() {
        let mut relabelled = ours();
        relabelled.game = "v0.107.1-local".into();
        relabelled.commit = "deadbeef".into();
        assert!(relabelled.describes(&V0_107_1::VERSION));
    }
}
