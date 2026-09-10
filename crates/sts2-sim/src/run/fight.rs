//! Driving the fight the run is standing in.

use nonmax::NonMaxU32;
use sts2_core::{Answered, Reason, Refusal, Rules};

use super::State;
use crate::combat;
use crate::combat::Error;

impl<V: Rules> State<V> {
    /// Play one card from the hand.
    ///
    /// The fight is resolved on a copy and swapped in whole, so a card the
    /// simulator cannot play leaves the run exactly as it was.
    pub(crate) fn play_card(
        &mut self,
        index: u8,
        target: Option<NonMaxU32>,
    ) -> Result<Answered<V>, Error> {
        let refused = |reason| {
            Ok(Err(Refusal {
                what: "play_card".into(),
                reason,
            }))
        };

        let Some(combat) = self.fight() else {
            return refused(Reason::NotInCombat);
        };
        let play = match combat::resolve(combat, index, target) {
            Ok(play) => play,
            Err(reason) => return refused(reason),
        };

        let played = combat::play(combat, play)?;
        self.fought(played);
        Ok(Ok(self.report()))
    }
}
