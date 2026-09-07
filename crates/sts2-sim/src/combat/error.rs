//! Failures while constructing or advancing a combat.

/// Why combat construction failed.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
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
}
