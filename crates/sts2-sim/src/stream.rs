//! Reaching a run's named random streams at the counters an observation holds.
//!
//! A `MegaRng` is a seed and a draw count, and an observation records the count
//! rather than the generator. So a rule that draws rebuilds the stream, seeks to
//! where the run left it, draws, and writes the new count back.
//!
//! Seeking and recording are separate calls. Every seek happens before any
//! record, so a combat that cannot be built leaves the counters alone.
//!
//! A stream is named through [`RunStreamKinds`], so a rule writes
//! `V::RunRngType::NICHE` and never the name the game seeds that stream with.
//! The seed offset the name hashes to is on the build's own type, taken at
//! compile time.

use crate::rng::MegaRng;
use sts2_core::rng::GameRng;
use sts2_core::{Counters, Rules, StreamKinds};

use crate::combat::Error;

/// A run stream, wound forward to the run's own draw count.
pub(crate) fn seek<V: Rules>(
    raw_seed: u32,
    counters: &Counters<V>,
    kind: V::RunRngType,
) -> Result<MegaRng, Error> {
    // The game's own name for the stream, which is what an error reports.
    let stream = kind.name();
    let at = counters
        .run
        .get(&kind)
        .copied()
        .ok_or(Error::MissingCounter { stream })?;
    let at = i32::try_from(at).map_err(|_| Error::CounterOutOfRange { stream })?;

    let mut rng = MegaRng::for_stream(raw_seed, kind);
    rng.fast_forward(at)
        .map_err(|source| Error::Seek { stream, source })?;
    Ok(rng)
}

/// Write back how far the stream has been drawn.
pub(crate) fn record<V: Rules>(
    counters: &mut Counters<V>,
    kind: V::RunRngType,
    rng: &MegaRng,
) -> Result<(), Error> {
    let counter = u32::try_from(rng.counter()).map_err(|_| Error::CounterOutOfRange {
        stream: kind.name(),
    })?;
    counters.run.insert(kind, counter);
    Ok(())
}
