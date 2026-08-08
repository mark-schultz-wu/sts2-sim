//! What produced an observation.
//!
//! The only part of the schema that describes the experiment rather than the
//! game. A result that cannot say what produced it is not a result.

use alloc::string::{String, ToString};

use alloc::vec::Vec;
use core::fmt::Debug;

use serde::de::{DeserializeOwned, Error};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{Game, Sts1, Sts2};

/// How a game identifies a build of itself.
///
/// The two do not agree on what a version is, so neither does this: STS1 is
/// distributed as a Steam build id, STS2 as a release number.
pub trait GameVersion: Game {
    type Version: Clone + Debug + PartialEq + Eq + Serialize + DeserializeOwned;
}

impl GameVersion for Sts1 {
    type Version = BuildId;
}

impl GameVersion for Sts2 {
    type Version = Release;
}

/// A Steam build id, which is how STS1 builds are told apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BuildId(pub u64);

/// An STS2 release number, as `0.107.1`.
///
/// Ordered, so a trace can be placed relative to a build that changed
/// behaviour.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Release {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl core::fmt::Display for Release {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl core::str::FromStr for Release {
    type Err = ();

    /// Accepts a leading `v`, since the game labels its own releases that way.
    fn from_str(s: &str) -> Result<Self, ()> {
        let mut parts = s.strip_prefix('v').unwrap_or(s).split('.');
        let mut next = || parts.next().ok_or(())?.parse::<u16>().map_err(|_| ());
        let (major, minor, patch) = (next()?, next()?, next()?);
        if parts.next().is_some() {
            return Err(());
        }
        Ok(Release {
            major,
            minor,
            patch,
        })
    }
}

impl Serialize for Release {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for Release {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = <&str>::deserialize(d)?;
        s.parse()
            .map_err(|()| D::Error::custom(alloc::format!("not a release number: {s}")))
    }
}

/// The configuration an observation came from.
///
/// Every field is mandatory, including [`mods`](Self::mods): no field has a
/// serde `default`, so an incomplete payload fails rather than being completed
/// with a claim nobody made.
///
/// `game` is the zero-sized [`Game`] marker, so it costs nothing, still appears
/// on the wire, and cannot disagree with the type it is part of.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, bound = "G: GameVersion")]
pub struct Provenance<G: GameVersion> {
    pub game: G,
    pub game_version: G::Version,
    /// What produced the observation: `sim`, `sts1-cli`, `sts2-cli 0.2.0`.
    pub adapter: String,
    /// Loaded mods, in load order. Empty means vanilla.
    pub mods: Vec<String>,
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::*;

    /// Each field separately, so that a `#[serde(default)]` added to any one
    /// of them fails here.
    #[test]
    fn every_field_is_mandatory() {
        for omitted in ["game", "game_version", "adapter", "mods"] {
            let mut fields = vec![
                ("game", "\"sts1\""),
                ("game_version", "10180494"),
                ("adapter", "\"sim\""),
                ("mods", "[]"),
            ];
            fields.retain(|(k, _)| *k != omitted);
            let body = fields
                .iter()
                .map(|(k, v)| alloc::format!("\"{k}\":{v}"))
                .collect::<Vec<_>>()
                .join(",");
            let json = alloc::format!("{{{body}}}");
            assert!(
                serde_json::from_str::<Provenance<Sts1>>(&json).is_err(),
                "omitting {omitted} must not parse, got {json}"
            );
        }
    }

    /// The marker is the tag: reading a payload into the wrong game fails
    /// rather than producing provenance that lies about its source.
    #[test]
    fn a_payload_cannot_change_game() {
        let json = r#"{"game":"sts1","game_version":10180494,"adapter":"sim","mods":[]}"#;
        assert!(serde_json::from_str::<Provenance<Sts1>>(json).is_ok());
        assert!(serde_json::from_str::<Provenance<Sts2>>(json).is_err());
    }

    /// A version is the kind of thing its own game uses, so neither game's
    /// form parses as the other's.
    #[test]
    fn each_game_has_its_own_kind_of_version() {
        let sts2 = r#"{"game":"sts2","game_version":"v0.107.1","adapter":"sts2-cli","mods":[]}"#;
        let p = serde_json::from_str::<Provenance<Sts2>>(sts2).expect("parses");
        assert_eq!(
            p.game_version,
            Release {
                major: 0,
                minor: 107,
                patch: 1
            }
        );
        // Canonical form drops the `v` the game labels releases with.
        assert!(
            serde_json::to_string(&p)
                .expect("serialises")
                .contains(r#""game_version":"0.107.1""#)
        );

        let swapped = r#"{"game":"sts2","game_version":10180494,"adapter":"x","mods":[]}"#;
        assert!(serde_json::from_str::<Provenance<Sts2>>(swapped).is_err());
    }

    #[test]
    fn releases_order_by_component() {
        let v = |s: &str| s.parse::<Release>().expect("parses");
        assert!(v("0.107.1") < v("0.107.2"));
        assert!(v("0.107.9") < v("0.108.0"));
        assert!(v("0.99.0") < v("1.0.0"));
        assert_eq!(v("v0.107.1"), v("0.107.1"));
        assert!("0.107".parse::<Release>().is_err());
        assert!("0.107.1.4".parse::<Release>().is_err());
    }
}
