//! What the game says about itself before anything is asked of it.

use crate::Error;
use serde::Deserialize;
use serde_json::Value;
use sts2_content::Version;

/// The protocol version this crate speaks. A mismatch is refused at the
/// handshake rather than discovered field by field later.
pub const PROTOCOL_VERSION: u32 = 1;

/// What the game said about itself when it connected.
///
/// Deserialized directly rather than through [`crate::Response`], because the greeting
/// arrives once before any request and is never a reply to one — putting it in
/// the response enum would oblige every caller to match a case none can reach.
#[derive(Debug, Clone, Deserialize)]
pub struct Greeting {
    pub protocol: u32,
    /// The build's own version string, not a constant the mod carries.
    pub game: String,
    pub commit: String,
    #[serde(rename = "assemblyHash")]
    pub assembly_hash: i64,
    /// XxHash32 over the model-id namespace. This is the one to check before
    /// trusting generated content tables: it moves when content moves, whereas
    /// an assembly change that adds no content leaves those tables valid.
    #[serde(rename = "contentHash")]
    pub content_hash: u32,
    /// Every command this game accepts.
    pub commands: Vec<String>,
}

impl Greeting {
    /// Whether this is the build a set of content tables describes.
    ///
    /// Checked at launch, against the [`ContentVersion`] the game was typed
    /// for, so a client physically cannot drive a build its tables do not
    /// describe — which used to be the caller's job to remember.
    ///
    /// Both hashes, because either alone lets something through. The content
    /// hash is over the *set of model ids*, so it stays equal across a patch
    /// that only moved numbers; the assembly hash moves on any code change, so
    /// it catches that but says nothing about which ids exist.
    pub(crate) fn describes(&self, version: &Version) -> Result<(), Error> {
        if self.content_hash != version.content_hash || self.assembly_hash != version.assembly_hash
        {
            return Err(Error::WrongBuild {
                expected: format!(
                    "{} ({}), assembly {}, content {}",
                    version.name, version.commit, version.assembly_hash, version.content_hash
                ),
                found: format!(
                    "{} ({}), assembly {}, content {}",
                    self.game, self.commit, self.assembly_hash, self.content_hash
                ),
            });
        }

        Ok(())
    }
}

/// Reads the one message that is not a [`crate::Response`].
///
/// Checked in two steps so a wrong message names itself. Deserializing straight
/// into [`Greeting`] would report a missing `protocol` field when what actually
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
