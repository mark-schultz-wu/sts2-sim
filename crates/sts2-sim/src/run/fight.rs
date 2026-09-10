//! Driving the fight the run is standing in.

use sts2_core::{Answered, CombatId, HandIndex, Reason, Refusal, Rules};

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
        index: HandIndex,
        target: Option<CombatId>,
    ) -> Result<Answered<V>, Error> {
        let refused = |reason| {
            Ok(Err(Refusal {
                what: "play_card".into(),
                reason,
            }))
        };

        let Some(ongoing) = self.fight() else {
            return refused(Reason::NotInCombat);
        };
        let play = match combat::resolve(&ongoing.combat, index, target) {
            Ok(play) => play,
            Err(reason) => return refused(reason),
        };

        let played = combat::play(&ongoing.combat, play)?;
        self.fought(combat::Ongoing {
            combat: played,
            machines: ongoing.machines.clone(),
        });
        Ok(Ok(self.report()))
    }

    /// End the player's turn, and set up the next one.
    ///
    /// On a copy of the fight and the graphs beside it, as playing a card is.
    pub(crate) fn end_turn(&mut self) -> Result<Answered<V>, Error> {
        let Some(ongoing) = self.fight() else {
            return Ok(Err(Refusal {
                what: "end_turn".into(),
                reason: Reason::NotInCombat,
            }));
        };

        let max_energy: V::Count = num_traits::cast(V::max_energy(self.character()))
            .expect("the turn's energy fits the build's count type");
        let (ended, counters) = combat::end_turn(
            ongoing,
            self.raw_seed(),
            self.ascension(),
            &self.counters(),
            max_energy,
        )?;

        self.drew(counters);
        self.fought(ended);
        Ok(Ok(self.report()))
    }
}
