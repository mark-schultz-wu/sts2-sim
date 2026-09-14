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

        let Some(live) = self.fight() else {
            return refused(Reason::NotInCombat);
        };
        let play = match combat::resolve(live, index, target) {
            Ok(play) => play,
            Err(reason) => return refused(reason),
        };

        let played = combat::play(live, play, self.raw_seed(), &self.counters(), self.relics())?;
        Ok(Ok(self.settle(played)))
    }

    /// End the player's turn, and set up the next one.
    ///
    /// On a copy of the fight and the graphs beside it, as playing a card is.
    pub(crate) fn end_turn(&mut self) -> Result<Answered<V>, Error> {
        let Some(live) = self.fight() else {
            return Ok(Err(Refusal {
                what: "end_turn".into(),
                reason: Reason::NotInCombat,
            }));
        };

        let max_energy: V::Count = num_traits::cast(V::max_energy(self.character()))
            .expect("the turn's energy fits the build's count type");
        let ended = combat::end_turn(
            live,
            self.raw_seed(),
            self.ascension(),
            &self.counters(),
            max_energy,
            self.relics(),
        )?;
        Ok(Ok(self.settle(ended)))
    }

    /// Put what a decision left behind in place of the fight it acted on.
    ///
    /// A decision that won the fight goes on to the rewards, which is
    /// `CombatManager.EndCombat` calling `RewardsCmd.Offer`.
    fn settle(&mut self, fought: combat::Fought<V>) -> sts2_core::Applied<V> {
        self.drew(fought.counters);
        // The belt the fight wrote, beside the streams it drew from. Neither
        // is committed until the decision is one the simulator could carry
        // out whole.
        self.hold(fought.relics);
        match fought.won {
            false => self.fought(fought.live),
            true => {
                let combat = fought.live.finish();
                let offered = self.combat_rewards(&combat);
                self.won(combat, offered);
            }
        }
        let mut applied = self.report();
        // A decision the end of a fight cut short is `Decided::Abandoned`. The
        // rewards it put up are what is left of it.
        applied.resolved = applied.asking.is_none();
        applied
    }

    /// `RewardsSet.WithRewardsFromRoom`, for the fight just won.
    fn combat_rewards(&mut self, combat: &sts2_core::Combat<V>) -> Vec<crate::reward::Offer<V>> {
        let encounter = combat
            .encounter
            .unwrap_or_else(|| unimplemented!("a fight with no encounter behind it"));
        let ascension = self.ascension();
        let character = self.character();
        let gold = V::encounter_gold(encounter, ascension);
        let act = self.act();
        let (rng, odds) = self.rewarding();
        crate::reward::combat::won::<V, _>(rng, odds, ascension, character, gold, act)
    }
}
