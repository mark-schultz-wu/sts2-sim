//! How a run plays at random: the census's runner and the recorder alike.
//!
//! One rule for both. A run the game recorded is the run the census plays on
//! the simulator, for as long as the two agree. A seed picks the same index
//! from the same list.

use sts2_core::{Answer, Decision};

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

/// Whether a run at random can throw a potion away here. That is a reward
/// screen offering a potion not yet taken, with every belt slot full.
///
/// Mark's rule, 2026-10-21. A discard is legal for every potion at almost
/// every step. Picked as evenly as a card, it throws most potions away. Here
/// it makes room, which is what a player discards for.
pub fn making_room<V: sts2_core::ContentVersion>(
    asking: Option<&sts2_core::Question<V>>,
    potions: usize,
    slots: V::Count,
) -> bool {
    let Some(sts2_core::Question::Rewards { rewards, .. }) = asking else {
        return false;
    };
    let full = num_traits::ToPrimitive::to_usize(&slots).is_some_and(|slots| potions >= slots);
    full && rewards.iter().any(|detail| {
        !detail.taken
            && matches!(
                detail.reward,
                sts2_core::RewardKind::Potion { potion: Some(_) }
            )
    })
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

    /// Any of `n` moves, each as likely as the next: the fuzzer's pick,
    /// which leaves nothing out.
    ///
    /// # Panics
    ///
    /// On no moves, which a run waiting on nobody has.
    pub fn any(&mut self, n: usize) -> usize {
        assert!(n > 0, "a choice among no moves");
        self.below(n)
    }

    /// Which of `legal` to play.
    ///
    /// Ending the turn is picked one time in ten while anything else is
    /// legal, so a fight is played rather than passed. Throwing a potion
    /// away is picked only while `making_room`, which [`making_room`] says.
    /// Backing out of a selection is never picked, because no run needs it.
    /// Anything else is picked evenly.
    ///
    /// # Panics
    ///
    /// On an empty list, which a run waiting on nobody has.
    pub fn choose<V: sts2_core::ContentVersion>(
        &mut self,
        legal: &[Decision<V>],
        making_room: bool,
    ) -> usize {
        assert!(!legal.is_empty(), "a choice among no moves");
        let others: Vec<usize> = (0..legal.len())
            .filter(|&at| !matches!(legal[at], Decision::EndTurn))
            .filter(|&at| making_room || !matches!(legal[at], Decision::DiscardPotion { .. }))
            .filter(|&at| !matches!(legal[at], Decision::Answer(Answer::Cancel)))
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

#[cfg(test)]
mod discard_tests {
    use super::Pick;
    use sts2_core::{Answer, BeltIndex, Decision};

    type V = sts2_content::v0_107_1::V0_107_1;

    /// A run at random throws a potion away only while it makes room.
    #[test]
    fn a_discard_is_picked_only_while_making_room() {
        let discard = Decision::<V>::DiscardPotion {
            index: BeltIndex::try_new(0).expect("a belt index"),
        };
        let legal = [Decision::<V>::Proceed, discard];
        let mut pick = Pick::for_seed("discards");
        assert!((0..200).all(|_| pick.choose(&legal, false) == 0));
        assert!((0..200).any(|_| pick.choose(&legal, true) == 1));
    }

    /// A run at random never backs out of a selection.
    #[test]
    fn backing_out_is_never_picked() {
        let legal = [
            Decision::<V>::Answer(Answer::Done),
            Decision::<V>::Answer(Answer::Cancel),
        ];
        let mut pick = Pick::for_seed("cancels");
        assert!((0..200).all(|_| pick.choose(&legal, true) == 0));
    }
}
