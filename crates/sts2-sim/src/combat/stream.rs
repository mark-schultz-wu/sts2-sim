//! Reaching a run's named random streams at the counters an observation holds.
//!
//! A `MegaRng` is a seed and a draw count, and an observation records the count
//! rather than the generator. So a rule that draws rebuilds the stream, seeks to
//! where the run left it, draws, and writes the new count back.
//!
//! Seeking and recording are separate calls. Every seek happens before any
//! record, so a combat that cannot be built leaves the counters alone.

use crate::rng::MegaRng;
use sts2_core::rng::GameRng;
use sts2_core::{Counters, Rules};

use super::Error;

/// The stream `kind` names, wound forward to the run's own draw count.
pub(crate) fn seek<V: Rules>(
    raw_seed: u32,
    counters: &Counters<V>,
    kind: V::RunRngType,
    stream: &'static str,
) -> Result<MegaRng, Error> {
    let at = counters
        .run
        .get(&kind)
        .copied()
        .ok_or(Error::MissingCounter { stream })?;
    let at = i32::try_from(at).map_err(|_| Error::CounterOutOfRange { stream })?;

    let mut rng = MegaRng::named(raw_seed, stream);
    rng.fast_forward(at)
        .map_err(|source| Error::Seek { stream, source })?;
    Ok(rng)
}

/// Write back how far the stream has been drawn.
pub(crate) fn record<V: Rules>(
    counters: &mut Counters<V>,
    kind: V::RunRngType,
    stream: &'static str,
    rng: &MegaRng,
) -> Result<(), Error> {
    let counter = u32::try_from(rng.counter()).map_err(|_| Error::CounterOutOfRange { stream })?;
    counters.run.insert(kind, counter);
    Ok(())
}
