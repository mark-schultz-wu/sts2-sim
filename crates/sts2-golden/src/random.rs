//! How a run plays at random: the census's runner and the recorder alike.
//!
//! One rule for both. A run the game recorded is the run the census plays on
//! the simulator, for as long as the two agree. A seed picks the same index
//! from the same list.

use sts2_core::Decision;

/// The profile's wins with the character as a run on `seed` starts, from
/// none to five.
///
/// The Architect picks a won run's last dialogue by it: one for each of a
/// character's first few wins, then a pool. Six counts reach them all. The
/// draw has a generator of its own, so a seed picks the moves it picked
/// before.
pub fn wins(seed: &str) -> u32 {
    let mut pick = Pick::for_seed(&format!("{seed} wins"));
    u32::try_from(pick.below(6)).expect("a count below six fits a u32")
}

/// SplitMix64, which is enough for picking among a handful of moves.
pub struct Pick(u64);

impl Pick {
    /// The generator a run's seed starts.
    pub fn for_seed(seed: &str) -> Self {
        Self(
            seed.bytes()
                .fold(0, |at, b| at.wrapping_mul(31) ^ u64::from(b)),
        )
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    /// Which of `legal` to play.
    ///
    /// Ending the turn is picked one time in ten while anything else is
    /// legal, so a fight is played rather than passed. Anything else is
    /// picked evenly.
    ///
    /// # Panics
    ///
    /// On an empty list, which a run waiting on nobody has.
    pub fn choose<V: sts2_core::ContentVersion>(&mut self, legal: &[Decision<V>]) -> usize {
        assert!(!legal.is_empty(), "a choice among no moves");
        let others: Vec<usize> = (0..legal.len())
            .filter(|&at| !matches!(legal[at], Decision::EndTurn))
            .collect();
        let end = legal
            .iter()
            .position(|decision| matches!(decision, Decision::EndTurn));
        match end {
            Some(end) if others.is_empty() || self.below(10) == 0 => end,
            _ => others[self.below(others.len())],
        }
    }
}
