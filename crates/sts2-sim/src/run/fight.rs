//! Driving the fight the run is standing in.

use sts2_core::{
    Answer, BeltIndex, CombatId, Decided, HandIndex, PotionTable, Reason, Refusal, Rules,
};

use super::{Settled, State};
use crate::combat;
use crate::combat::Error;

impl<V: Rules> State<V> {
    /// Answer the selection on which a card play stopped.
    pub(super) fn answer_fight(&mut self, answer: &Answer) -> Result<Decided, Error> {
        let Answer::Cards(chosen) = *answer else {
            unimplemented!("an answer that is not a card selection");
        };
        let Some(fighting) = self.fighting() else {
            unreachable!("a fight answer has a live fight");
        };
        let since = fighting.live.removed_from_deck().len();
        let ended = combat::answer(fighting, chosen)?;
        Ok(self.settle(ended, since))
    }

    /// Play one card from the hand.
    ///
    /// A refused play leaves the run exactly as it was. Everything that can
    /// refuse is asked before the fight is touched.
    pub(crate) fn play_card(
        &mut self,
        index: HandIndex,
        target: Option<CombatId>,
    ) -> Result<Settled<V>, Error> {
        let refused = |reason| {
            Ok(Err(Refusal {
                what: "play_card".into(),
                reason,
            }))
        };

        let Some(fighting) = self.fighting() else {
            return refused(Reason::NotInCombat);
        };
        let play = match combat::resolve(fighting.live, index, target) {
            Ok(play) => play,
            Err(reason) => return refused(reason),
        };

        let since = fighting.live.removed_from_deck().len();
        let ended = combat::play(fighting, play)?;
        Ok(Ok(self.settle(ended, since)))
    }

    /// Drink one potion from the belt.
    ///
    /// `OnUseWrapper` takes the potion out of the belt before its effect runs.
    ///
    /// What can be drunk is the belt's own question, and the observation
    /// carries that answer beside every potion. This holds the decision to it
    /// rather than working out a second answer.
    pub(crate) fn use_potion(
        &mut self,
        index: BeltIndex,
        target: Option<CombatId>,
    ) -> Result<Settled<V>, Error> {
        let refused = |reason| {
            Ok(Err(Refusal {
                what: "use_potion".into(),
                reason,
            }))
        };

        // The belt alone, rather than the whole report it appears in.
        let belt = self.belt();
        let at = usize::from(index.get());
        let Some(held) = belt.get(at) else {
            return refused(Reason::OutOfRange {
                index: i64::from(index.get()),
                count: belt.len(),
            });
        };
        if let Some(why) = held.reason {
            return refused(Reason::Undrinkable {
                potion: held.potion,
                why,
            });
        }
        if let Some(id) = target
            && !held.targets.contains(&id)
        {
            return refused(Reason::NoSuchCreature { id });
        }
        if !held.targets.is_empty() && target.is_none() {
            return refused(Reason::WrongPotionTarget {
                potion: held.potion,
                target,
                takes: held
                    .potion
                    .target()
                    .unwrap_or_else(|| unimplemented!("a potion that decides its own target")),
            });
        }

        let potion = held.potion;
        let Some(fighting) = self.fighting() else {
            unimplemented!("a potion drunk outside a fight");
        };
        let since = fighting.live.removed_from_deck().len();
        let ended = combat::drink(fighting, potion, target)?;
        self.drank(at);
        Ok(Ok(self.settle(ended, since)))
    }

    /// End the player's turn, and set up the next one.
    pub(crate) fn end_turn(&mut self) -> Result<Settled<V>, Error> {
        let max_energy: V::Count = num_traits::cast(V::max_energy(self.character()))
            .expect("the turn's energy fits the build's count type");
        let ascension = self.ascension();
        let Some(fighting) = self.fighting() else {
            return Ok(Err(Refusal {
                what: "end_turn".into(),
                reason: Reason::NotInCombat,
            }));
        };

        let since = fighting.live.removed_from_deck().len();
        let ended = combat::end_turn(fighting, ascension, max_energy)?;
        Ok(Ok(self.settle(ended, since)))
    }

    /// Carry what a decision did to the fight over to the rest of the run.
    ///
    /// `since` is how many deck copies the fight had taken before the
    /// decision began. A decision that won the fight goes on to the rewards,
    /// which is `CombatManager.EndCombat` calling `RewardsCmd.Offer`.
    #[tracing::instrument(level = "trace", skip_all)]
    pub(super) fn settle(&mut self, ended: Option<combat::Ended>, since: usize) -> Decided {
        self.lose_cards(since);
        match ended {
            None => {}
            // Not `finish`: the run never left the fight, and the piles and
            // the powers are where the player fell.
            Some(combat::Ended::Lost) => {
                let (live, relics) = self.left_fight();
                let combat = live.report(relics);
                self.lost(combat);
            }
            Some(combat::Ended::Won) => {
                let (live, relics) = self.left_fight();
                let returned = live.returned().to_vec();
                let combat = live.finish(relics);
                let mut offered = self.combat_rewards(&combat);
                let at = offered
                    .iter()
                    .position(|offer| matches!(offer, crate::reward::Offer::Card { .. }))
                    .unwrap_or(offered.len());
                for card in returned.into_iter().rev() {
                    offered.insert(at, crate::reward::Offer::SpecialCard { card, taken: false });
                }
                self.won(combat, offered);
            }
        }
        // A decision the end of a fight cut short is `Decided::Abandoned`. The
        // rewards it put up are what is left of it.
        self.decided()
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
        // `GenerateRewardsFor` switches on the room, which is what adds a
        // relic, moves the potion odds, and prices the card reward's
        // rarities. The encounter carries the room it belongs to.
        let room = match <_ as sts2_core::EncounterTable>::room(encounter) {
            room if room == <_ as sts2_core::RoomKinds>::ELITE => {
                crate::reward::combat::Room::Elite
            }
            room if room == <_ as sts2_core::RoomKinds>::BOSS => crate::reward::combat::Room::Boss,
            _ => crate::reward::combat::Room::Monster,
        };
        let (rng, odds) = self.rewarding();
        let (mut offered, rarity) =
            crate::reward::combat::won::<V, _>(rng, odds, ascension, character, gold, act, room);
        if let Some(rarity) = rarity {
            // `PullNextRelicFromFront`, which the run's own bookkeeping covers.
            let relic = self.pull_relic(rarity);
            let at = crate::reward::combat::relic_slot(&offered);
            offered.insert(
                at,
                crate::reward::Offer::Relic {
                    relic,
                    taken: false,
                },
            );
        }
        offered
    }
}
