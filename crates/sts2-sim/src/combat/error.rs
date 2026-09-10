//! Failures while constructing or advancing a combat.

use sts2_core::ContentVersion;

/// Why a combat stopped, whether it was being built or carried forward.
///
/// Generic, because a card the port does not cover names itself, and this
/// crate reports ids rather than spelling them. A front end spells one.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error<V: ContentVersion> {
    #[error("the encounter roster must be supplied by its caller")]
    ConfiguredRoster,
    #[error(transparent)]
    Content(#[from] sts2_core::ContentError),
    #[error("the {stream} RNG counter is missing")]
    MissingCounter { stream: &'static str },
    #[error("the {stream} RNG counter is outside the supported range")]
    CounterOutOfRange { stream: &'static str },
    #[error("seeking the {stream} RNG failed")]
    Seek {
        stream: &'static str,
        #[source]
        source: sts2_core::rng::Error,
    },
    #[error("combat has too many creatures to assign an id")]
    CreatureIdOverflow,
    /// An upgraded card prints different numbers, and the tables carry the
    /// unupgraded ones. See `OnUpgrade` on the card.
    #[error("the simulator does not know what an upgraded card prints")]
    UpgradedCard,
    /// An affliction or an enchantment rewrites what the card does.
    #[error("the simulator does not know what an altered card prints")]
    AlteredCard,
    /// One of the four cards that work their target out from the fight.
    #[error("the card decides what it targets, and the simulator cannot ask")]
    ComputedTarget,
    /// A `CalculatedVar`, whose printed number counts something in the fight.
    #[error("the {name} the card prints is counted from live combat")]
    CalculatedVar { name: &'static str },
    /// A build can store exact fractions even where this combat slice only
    /// evaluates whole-number effects.
    #[error("the {name} the card prints is not a whole number")]
    FractionalVar { name: &'static str },
    /// A card whose `OnPlay` nobody has ported.
    #[error("the simulator does not know what {card:?} does")]
    UnportedCard { card: V::Card },
    /// A command a card issued that the fight cannot answer.
    #[error("the simulator does not implement {0} yet")]
    Unimplemented(&'static str),
    /// A card whose type the tables do not settle, which decides where a
    /// played copy goes.
    #[error("the card works its own type out from the fight")]
    ComputedType,
}
