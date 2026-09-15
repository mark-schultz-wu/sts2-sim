//! Reaching a run's named random streams at the counters an observation holds.
//!
//! A `MegaRng` is a seed and a draw count, and an observation records the count
//! rather than the generator. So a rule that draws rebuilds the stream, seeks to
//! where the run left it, draws, and writes the new count back.
//!
//! Seeking and recording are separate calls. Every seek happens before any
//! record, so a combat that cannot be built leaves the counters alone.
//!
//! A stream is named by [`RunRng`], the job it performs, and the build says
//! which of its own streams that is. Nothing here spells a stream's game-side
//! name: the seed offset that name hashes to is on the build's type, taken at
//! compile time.

use crate::rng::MegaRng;
use sts2_core::rng::GameRng;
use sts2_core::{Counters, Rules, RunRng};

use crate::combat::Error;

/// The stream `kind` performs, wound forward to the run's own draw count.
pub(crate) fn seek<V: Rules>(
    raw_seed: u32,
    counters: &Counters<V>,
    kind: RunRng,
) -> Result<MegaRng, Error> {
    let stream = V::run_rng(kind);
    let at = counters
        .run
        .get(&stream)
        .copied()
        .ok_or(Error::MissingCounter { stream: kind })?;
    let at = i32::try_from(at).map_err(|_| Error::CounterOutOfRange { stream: kind })?;

    let mut rng = MegaRng::for_stream(raw_seed, stream);
    rng.fast_forward(at).map_err(|source| Error::Seek {
        stream: kind,
        source,
    })?;
    Ok(rng)
}

/// Write back how far the stream has been drawn.
pub(crate) fn record<V: Rules>(
    counters: &mut Counters<V>,
    kind: RunRng,
    rng: &MegaRng,
) -> Result<(), Error> {
    let counter =
        u32::try_from(rng.counter()).map_err(|_| Error::CounterOutOfRange { stream: kind })?;
    counters.run.insert(V::run_rng(kind), counter);
    Ok(())
}
