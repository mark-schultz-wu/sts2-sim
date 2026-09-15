//! Failures while constructing or advancing a combat.

/// Why a combat stopped.
///
/// Every variant is the game or the run handing this crate something it cannot
/// read. A rule the simulator has not ported is not here. That panics where it
/// is reached, so an unimplemented path stops the run. A run that carries on
/// past one reports a result nobody can trust.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
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
