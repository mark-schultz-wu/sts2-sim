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
        let ended = match combat::answer(fighting, chosen)? {
            None => self.turn_ended_by_play()?,
            ended => ended,
        };
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
        let play = match combat::resolve(fighting.live, fighting.relics, index, target) {
            Ok(play) => play,
            Err(reason) => return refused(reason),
        };

        let since = fighting.live.removed_from_deck().len();
        let ended = match combat::play(fighting, play)? {
            None => self.turn_ended_by_play()?,
            ended => ended,
        };
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
                    .unwrap_or_else(|| V::computed_potion_target(held.potion, true)),
            });
        }

        let potion = held.potion;
        let Some(fighting) = self.fighting() else {
            // `RemoveBeforeUse`, then what it asks the run for.
            self.drank(at);
            let effects = V::on_use_on_run(potion, &*self, false);
            if self.carry_out(effects).is_some() {
                unimplemented!("a potion drunk outside a fight that asks a question");
            }
            let used = self.notify_belt(sts2_core::EventHook::PotionUsed { in_fight: false });
            if self.carry_out(used).is_some() {
                unimplemented!("a relic asking a question after a potion outside a fight");
            }
            return Ok(Ok(Decided::Resolved));
        };
        let since = fighting.live.removed_from_deck().len();
        let ended = combat::drink(fighting, potion, target)?;
        self.drank(at);
        // What the potion asks of the run beside the fight, which is its belt.
        let effects = V::on_use_on_run(potion, &*self, true);
        if self.carry_out(effects).is_some() {
            unimplemented!("a potion drunk in a fight that asks the run a question");
        }
        Ok(Ok(self.settle(ended, since)))
    }

    /// `SetReadyToEndTurn`: a play that asked for the turn to end has it end
    /// once the play is over and nothing waits on the player.
    fn turn_ended_by_play(&mut self) -> Result<Option<combat::Ended>, Error> {
        let max_energy: V::Count = num_traits::cast(V::max_energy(self.character()))
            .expect("the turn's energy fits the build's count type");
        let ascension = self.ascension();
        let Some(fighting) = self.fighting() else {
            return Ok(None);
        };
        if !fighting.live.ending_turn() || fighting.live.question().is_some() {
            return Ok(None);
        }
        fighting.live.take_ending_turn();
        combat::end_turn(fighting, ascension, max_energy)
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
        self.asked_by_fight();
        self.lose_cards(since);
        self.gold_from_fight();
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
                let extra_rewards = live.extra_rewards().to_vec();
                let taken = live.taken_from_deck().to_vec();
                // `AddExtraReward`, which the room's own rewards come before.
                let (extra, leaving): (Vec<_>, Vec<_>) =
                    live.leaving().iter().partition(|effect| {
                        matches!(
                            effect,
                            sts2_core::HookEffect::AddGoldReward(_)
                                | sts2_core::HookEffect::AddCardRemovalReward
                        )
                    });
                let proportion = live.gold_proportion();
                let escaped = live.any_escaped();
                let combat = live.finish(relics);
                if let Some(resuming) = self.resuming.as_mut() {
                    resuming.escaped = escaped;
                }
                // `AfterCombatEnd` runs before `RewardsCmd.Offer` builds the
                // rewards. It walks the deck's cards ahead of the fight's
                // listeners.
                if let Some(encounter) = combat.encounter {
                    self.deck_combat_ended(sts2_core::EncounterTable::room(encounter));
                }
                if let Some(asked) = self.carry_out(leaving) {
                    let pickup = asked.pickup;
                    unimplemented!("a fight's end asking for {pickup:?}");
                }
                let mut offered = self.combat_rewards(&combat, proportion, &extra_rewards, &taken);
                // `GoldReward.Populate` draws `NextInt(min, max + 1)` off the
                // rewards stream, after the room's own, even for a gold
                // reward whose two bounds are the one amount.
                // A card removal's own `Populate` draws nothing.
                for effect in extra {
                    offered.push(match effect {
                        sts2_core::HookEffect::AddGoldReward(amount) => {
                            let (rng, _) = self.rewarding();
                            crate::reward::Offer::Gold {
                                amount: crate::reward::fixed_gold::<V, _>(rng, amount),
                                taken: false,
                            }
                        }
                        sts2_core::HookEffect::AddCardRemovalReward => {
                            crate::reward::Offer::CardRemoval { taken: false }
                        }
                        _ => unreachable!("the extra rewards are gold and card removals"),
                    });
                }
                crate::reward::sort(&mut offered);
                self.won(combat, offered);
            }
        }
        // A decision the end of a fight cut short is `Decided::Abandoned`. The
        // rewards it put up are what is left of it.
        self.decided()
    }

    /// `RewardsSet.WithRewardsFromRoom`, for the fight just won.
    ///
    /// `proportion` is the room's `GoldProportion`. A gold reward is offered
    /// while it is above nothing, over the encounter's range scaled by it.
    fn combat_rewards(
        &mut self,
        combat: &sts2_core::Combat<V>,
        proportion: f32,
        extra_rewards: &[combat::ExtraReward<V>],
        taken: &[sts2_core::DeckCard<V>],
    ) -> Vec<crate::reward::Offer<V>> {
        let encounter = combat
            .encounter
            .unwrap_or_else(|| unimplemented!("a fight with no encounter behind it"));
        let ascension = self.ascension();
        let character = self.character();
        let gold = (proportion > 0.0).then(|| {
            let range = V::encounter_gold(encounter, ascension);
            // `(int)Math.Round((float)gold * GoldProportion)`, which rounds a
            // half to the even side.
            let scaled = |gold: u16| (f32::from(gold) * proportion).round_ties_even() as u16;
            sts2_core::rng::Interval::new(scaled(range.min()), scaled(range.max()))
        });
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
        let added = self.card_reward_added();
        let forced = self.potion_forced(<_ as sts2_core::EncounterTable>::room(encounter));
        let (rng, odds) = self.rewarding();
        let (mut offered, rarity) = crate::reward::combat::won::<V, _>(
            rng,
            odds,
            ascension,
            character,
            gold,
            act,
            room,
            crate::reward::combat::Modifiers {
                forced,
                added: &added,
            },
        );
        if let Some(rarity) = rarity {
            // `PullNextRelicFromFront`, which the run's own bookkeeping covers.
            let relic = self.pull_relic(rarity);
            offered.push(crate::reward::Offer::Relic {
                relic,
                rarity: <_ as sts2_core::RarityKinds>::UNDRAFTED,
                wax: false,
                taken: false,
            });
        }
        // `CombatRoom.ExtraRewards`, which `Populate` reaches after the
        // room's own.
        for extra in extra_rewards {
            offered.push(match *extra {
                combat::ExtraReward::SpecialCard(card) => crate::reward::Offer::SpecialCard {
                    card,
                    state: <V::CardState as sts2_core::CardModel<V>>::new(card.card),
                    taken: false,
                },
                combat::ExtraReward::Returned(id) => {
                    let returned = taken
                        .iter()
                        .find(|card| card.id == id)
                        .unwrap_or_else(|| unreachable!("a card handed back was taken"));
                    crate::reward::Offer::SpecialCard {
                        card: returned.ident,
                        state: returned.state,
                        taken: false,
                    }
                }
                combat::ExtraReward::Card => {
                    let added = self.card_reward_added();
                    let (rng, odds) = self.rewarding();
                    crate::reward::combat::card_reward::<V, _>(
                        rng, odds, ascension, character, act, room, &added,
                    )
                }
            });
        }
        self.reward_offers(&mut offered);
        let room = Some(<_ as sts2_core::EncounterTable>::room(encounter));
        self.modify_rewards(&mut offered, room);
        offered
    }
}
