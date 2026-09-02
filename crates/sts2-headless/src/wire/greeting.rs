//! What the game says about itself before anything is asked of it.

use crate::Error;
use crate::stamp::Stamp;
use serde::Deserialize;
use serde_json::Value;
use sts2_core::Version;

// Written by `build.rs`, which hashes the mod's source with the same script the
// mod's own build runs. See that file for why it is derived rather than stored.
include!(concat!(env!("OUT_DIR"), "/mod_identity.rs"));

/// What the game said about itself when it connected.
///
/// Deserialized directly rather than through [`crate::Response`]. The greeting
/// arrives once before any request and is never a reply to one. Putting it in
/// the response enum obliges every caller to match a case none can reach.
#[derive(Debug, Clone, Deserialize)]
pub struct Greeting {
    /// Which build of the mod answered, as a hash of its source.
    ///
    /// Optional only so that a mod too old to identify itself is reported as
    /// the stale mod it is. Not as a greeting missing a field.
    #[serde(rename = "mod")]
    pub identity: Option<String>,
    /// The build's own version string, not a constant the mod carries.
    pub game: String,
    pub commit: String,
    #[serde(rename = "assemblyHash")]
    pub assembly_hash: i64,
    /// XxHash32 over the model-id namespace. This is the one to check before
    /// trusting generated content tables. It moves when content moves, and an
    /// assembly change that adds no content leaves those tables valid.
    #[serde(rename = "contentHash")]
    pub content_hash: u32,
    /// Every command this game accepts.
    pub commands: Vec<String>,
}

impl Greeting {
    /// Whether this is the mod this client was built against.
    ///
    /// Its own check, beside [`Greeting::describes`]. That one asks which
    /// *game* is running, and this asks which *mod* is answering. The crate
    /// README says what the difference catches.
    ///
    /// Deliberately strict: an edit to the mod's source invalidates a deployed
    /// build. Narrowing it to the files that shape the wire
    /// means maintaining a list, and that list is what rots.
    pub(crate) fn built_from_our_source(&self) -> Result<(), Error> {
        match self.identity.as_deref() {
            Some(found) if found == MOD_IDENTITY => Ok(()),
            Some(found) => Err(Error::WrongMod {
                expected: MOD_IDENTITY.to_string(),
                found: found.to_string(),
            }),
            // A mod from before the greeting carried this. It cannot be the one
            // we were built against, since ours does.
            None => Err(Error::WrongMod {
                expected: MOD_IDENTITY.to_string(),
                found: "a mod too old to say".to_string(),
            }),
        }
    }

    /// Which build connected.
    pub fn stamp(&self) -> Stamp {
        Stamp {
            game: self.game.clone(),
            commit: self.commit.clone(),
            assembly_hash: self.assembly_hash,
            content_hash: self.content_hash,
        }
    }

    /// Whether this is the build a set of content tables describes.
    ///
    /// Checked at launch, against the [`ContentVersion`] the game was typed
    /// for, so a client physically cannot drive a build its tables do not
    /// describe.
    ///
    /// What counts as the same build is [`Stamp::describes`], which a recording
    /// asks too. Answering that question twice is how the two answers come to
    /// differ.
    pub(crate) fn describes(&self, version: &Version) -> Result<(), Error> {
        let found = self.stamp();
        match found.describes(version) {
            true => Ok(()),
            false => Err(Error::WrongBuild {
                expected: Stamp::of(version).to_string(),
                found: found.to_string(),
            }),
        }
    }
}

/// Reads the one message that is not a [`crate::Response`].
///
/// Checked in two steps so a wrong message names itself. Deserializing straight
/// into [`Greeting`] reports a missing `protocol` field when what actually
/// happened is that the game said `bye`.
pub(crate) fn parse_greeting(line: &str) -> Result<Greeting, Error> {
    let value: Value =
        serde_json::from_str(line).map_err(|e| Error::Decode(line.to_string(), e))?;
    match value.get("type").and_then(Value::as_str) {
        Some("hello") => {
            serde_json::from_value(value).map_err(|e| Error::Decode(line.to_string(), e))
        }
        Some(other) => Err(Error::NoGreeting(other.to_string())),
        None => Err(Error::NoGreeting("untyped".to_string())),
    }
}
