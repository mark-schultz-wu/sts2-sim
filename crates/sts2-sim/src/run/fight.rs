//! Driving the fight the run is standing in.

use sts2_core::{
    Answer, Answered, BeltIndex, CombatId, HandIndex, PotionTable, Reason, Refusal, Rules,
};

use super::State;
use crate::combat;
use crate::combat::Error;

impl<V: Rules> State<V> {
    /// Answer the selection on which a card play stopped.
    pub(super) fn answer_fight(&mut self, answer: &Answer) -> Result<sts2_core::Applied<V>, Error> {
        let Answer::Cards(chosen) = answer else {
            unimplemented!("an answer that is not a card selection");
        };
        let Some(live) = self.fight() else {
            unreachable!("a fight answer has a live fight");
        };
        let fought = combat::answer(live, chosen, self.streams(), self.relics())?;
        Ok(self.settle(fought))
    }

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

        let played = combat::play(live, play, self.streams(), self.relics())?;
        Ok(Ok(self.settle(played)))
    }

    /// Drink one potion from the belt.
    ///
    /// `OnUseWrapper` takes the potion out of the belt before its effect runs.
    /// The belt and the fight are written back together or neither.
    ///
    /// What can be drunk is the belt's own question, and the observation
    /// carries that answer beside every potion. This holds the decision to it
    /// rather than working out a second answer.
    pub(crate) fn use_potion(
        &mut self,
        index: BeltIndex,
        target: Option<CombatId>,
    ) -> Result<Answered<V>, Error> {
        let refused = |reason| {
            Ok(Err(Refusal {
                what: "use_potion".into(),
                reason,
            }))
        };

        let belt = self.report().observation.potions;
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
        let Some(live) = self.fight() else {
            unimplemented!("a potion drunk outside a fight");
        };
        let drunk = combat::drink(live, potion, target, self.streams(), self.relics())?;
        self.drank(at);
        Ok(Ok(self.settle(drunk)))
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
            self.ascension(),
            self.streams(),
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
        self.drew(fought.streams);
        self.lose_cards(fought.live.removed_from_deck());
        // The belt the fight wrote, beside the streams it drew from. Neither
        // is committed until the decision is one the simulator could carry
        // out whole.
        self.hold(fought.relics);
        match fought.won {
            false => self.fought(fought.live),
            true => {
                let returned = fought.live.removed_from_deck().to_vec();
                let combat = fought.live.finish();
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
