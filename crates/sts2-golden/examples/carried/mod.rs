//! What every random run carries, for the census's runner and the recorder.
//!
//! 25 Gorgets and a Mercury Hourglass, so random play reaches content instead
//! of dying early. Both play the same runs, so both carry the same.

use sts2_content::ids::RelicIds as _;

/// The relics a run is granted before it starts, and how many of each.
pub fn carried<V: sts2_content::Build>() -> [(V::Relic, usize); 2] {
    [(V::Relic::GORGET, 25), (V::Relic::MERCURY_HOURGLASS, 1)]
}
