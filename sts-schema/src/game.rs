//! Which game a type is about, at the type level.
//!
//! [`Sts1`] and [`Sts2`] are zero-sized markers. Anything that differs between
//! the games takes a `G: Game`, and each module declares its own extension
//! trait for the axis it cares about — [`GameVersion`](crate::GameVersion) for
//! how a build is identified, [`GameRng`](crate::GameRng) for seeds and
//! streams. This trait stays a marker so that no single definition accumulates
//! every difference.
//!
//! A marker serialises as its own name, so it doubles as the wire tag: a
//! [`Provenance<Sts1>`](crate::Provenance) will not parse from a payload
//! claiming `sts2`. The impls are hand-written because serde's derive gives a
//! unit struct no name at all — it would go over the wire as `null`.

use core::fmt::{self, Debug, Formatter};
use core::hash::Hash;

use serde::de::{DeserializeOwned, Error, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A game, as a type.
///
/// No items of its own: it names the set of games and collects the bounds the
/// extension traits rely on.
pub trait Game:
    Copy + Clone + Debug + Default + PartialEq + Eq + Hash + Serialize + DeserializeOwned + 'static
{
}

/// Slay the Spire.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sts1;

/// Slay the Spire 2.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sts2;

impl Game for Sts1 {}
impl Game for Sts2 {}

impl Serialize for Sts1 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str("sts1")
    }
}

impl Serialize for Sts2 {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str("sts2")
    }
}

/// Reads the one name a marker answers to.
///
/// A visitor rather than `<&str>::deserialize` because `visit_str` is handed a
/// `&str` by every deserializer, borrowing or not, and this compares it in
/// place. Deserializing a `&str` would restrict the format to one that can lend
/// out its input; a `String` would allocate to hold four bytes we throw away.
struct Tag<G> {
    /// The accepted name, as a one-element slice so serde can report it.
    expected: &'static [&'static str; 1],
    game: G,
}

impl<'de, G> Visitor<'de> for Tag<G> {
    type Value = G;

    fn expecting(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, r#""{}""#, self.expected[0])
    }

    fn visit_str<E: Error>(self, name: &str) -> Result<G, E> {
        if name == self.expected[0] {
            Ok(self.game)
        } else {
            Err(E::unknown_variant(name, self.expected))
        }
    }
}

impl<'de> Deserialize<'de> for Sts1 {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_str(Tag {
            expected: &["sts1"],
            game: Sts1,
        })
    }
}

impl<'de> Deserialize<'de> for Sts2 {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_str(Tag {
            expected: &["sts2"],
            game: Sts2,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_marker_is_free() {
        assert_eq!(core::mem::size_of::<Sts1>(), 0);
        assert_eq!(core::mem::size_of::<Sts2>(), 0);
    }

    #[test]
    fn a_marker_is_its_own_wire_tag() {
        assert_eq!(
            serde_json::to_string(&Sts1).expect("serialises"),
            r#""sts1""#
        );
        assert_eq!(
            serde_json::from_str::<Sts2>(r#""sts2""#).expect("parses"),
            Sts2
        );
    }

    /// A `StringDeserializer` hands over an owned `String`, which is what any
    /// deserializer that cannot lend out its input does. Reading a `&str` would
    /// reject this.
    #[test]
    fn a_name_need_not_be_borrowed() {
        use serde::de::IntoDeserializer;
        use serde::de::value::{Error as ValueError, StringDeserializer};

        let d: StringDeserializer<ValueError> =
            alloc::string::String::from("sts1").into_deserializer();
        assert_eq!(Sts1::deserialize(d).expect("parses"), Sts1);
    }

    /// The reason a marker is serialisable at all: a payload cannot claim to be
    /// from a different game than the type it is read into.
    #[test]
    fn the_wrong_game_does_not_parse() {
        assert!(serde_json::from_str::<Sts1>(r#""sts2""#).is_err());
        assert!(serde_json::from_str::<Sts2>(r#""sts1""#).is_err());
    }
}
