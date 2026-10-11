//! What every random run carries, for the census's runner and the recorder.
//!
//! 25 Gorgets and a Mercury Hourglass, so random play reaches content instead
//! of dying early. Both play the same runs, so both carry the same.

use sts2_content::v0_107_1::relics::Relic;

/// The relics a run is granted before it starts, and how many of each.
pub const CARRIED: [(Relic, usize); 2] = [(Relic::Gorget, 25), (Relic::MercuryHourglass, 1)];
