//! The cards whose rules this build ports for itself, which the shared cards
//! hand over through [`crate::build::OwnRules`].

use sts2_core::{Amount, Effect, Fight, Played, Scaling};

use super::super::V0_111_0 as V;
use super::super::generated::cards::{
    CardVar, Cards, DefectCard, EventCard, IroncladCard, NecrobinderCard, QuestCard, SilentCard,
    TokenCard,
};
use super::super::generated::powers::Power;
use crate::generated::runstate::MapPointType;

type Number = i32;

/// `Eidolon`, which waits on its last play's question.
const EIDOLON: Cards = Cards::Necrobinder(NecrobinderCard::Eidolon);

/// `Tracking`'s `PowerCmd.Apply<TrackingPower>(…, 50m, …)`, which it prints
/// nowhere.
const TRACKING: Number = 50;

/// What a card of this build's own does when it is played, or nothing for a
/// shared one.
pub(crate) fn on_play<F: Fight<V>>(card: Cards, fight: &mut F) -> Option<Played<V>> {
    match card {
        // `ExpectAFight`: as much block as its `CalculatedBlock`, a move's.
        Cards::Ironclad(IroncladCard::ExpectAFight) => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(CardVar::CalculatedBlock),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            Some(Played::Done)
        }
        // `ForgottenRitual`: its energy, every play. v0.107.1's waited on a
        // card exhausted that turn, which the shared port reads, so this build
        // plays its generated script.
        Cards::Ironclad(IroncladCard::ForgottenRitual) => Some(
            crate::behaviour::cards::generated_from::<V, _>(card, fight, 0),
        ),
        // `Hyperbeam`: its damage to every enemy, then its Focus as a
        // `HyperbeamFocusDownPower` on its owner.
        Cards::Defect(DefectCard::Hyperbeam) => Some(on_play_from(card, fight, 0)),
        // `Outbreak`: its Poison onto each enemy that can be hit, from its
        // owner, then `PoisonPower.Trigger` on each that carries Poison.
        Cards::Silent(SilentCard::Outbreak) => {
            let poison = fight.power_var(CardVar::PoisonPower);
            fight.apply_to_opponents(Power::PoisonPower, sts2_core::PowerAmount::Counted(poison));
            for enemy in fight.hittable_opponents() {
                trigger_poison(fight, enemy);
            }
            Some(Played::Done)
        }
        // `Compact`: its block, then each Status of the hand that can be
        // transformed made into a Fuel, upgraded with it, in one
        // `CardCmd.Transform`. v0.107.1's transformed each in turn.
        Cards::Defect(DefectCard::Compact) => {
            use sts2_core::{CardKind, Pile, Rules as _};

            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(CardVar::Block),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            let mut fuel = sts2_core::CardIdent::new(Cards::Token(TokenCard::Fuel));
            fuel.upgrade = u8::from(fight.upgraded());
            let statuses: Vec<_> = fight
                .cards_in(Pile::Hand)
                .into_iter()
                .filter(|&card| V::card_kind(fight.card_ident(card).card) == Some(CardKind::Status))
                .map(|card| (card, fuel))
                .collect();
            fight.transform_all(&statuses);
            Some(Played::Done)
        }
        // `Haze`: its Poison onto each enemy that can be hit, then its Weak
        // onto each.
        Cards::Silent(SilentCard::Haze) => {
            let poison = fight.power_var(CardVar::PoisonPower);
            for enemy in fight.hittable_opponents() {
                fight.apply_power(
                    enemy,
                    Power::PoisonPower,
                    sts2_core::PowerAmount::Counted(poison),
                );
            }
            let weak = fight.power_var(CardVar::WeakPower);
            for enemy in fight.hittable_opponents() {
                fight.apply_power(
                    enemy,
                    Power::WeakPower,
                    sts2_core::PowerAmount::Counted(weak),
                );
            }
            Some(Played::Done)
        }
        // `Expertise`: its `Cards` drawn, each of which retains this turn.
        Cards::Silent(SilentCard::Expertise) => {
            let drew = fight.draw(fight.var(CardVar::Cards));
            let drawn: Vec<_> = drew
                .into_iter()
                .filter_map(|id| fight.find_card(id))
                .collect();
            fight.act_on(sts2_core::PileAct::RetainThisTurn, &drawn);
            Some(Played::Done)
        }
        // `Tracking`: 50 Tracking on its owner, every play.
        Cards::Silent(SilentCard::Tracking) => {
            fight.apply_to_actor(
                Power::TrackingPower,
                sts2_core::PowerAmount::Counted(TRACKING),
            );
            Some(Played::Done)
        }
        // `Eidolon`: `CardCmd.AutoPlay` of each card in the exhaust pile that
        // is Ethereal and not Unplayable, as the play began, with no target
        // named.
        Cards::Necrobinder(NecrobinderCard::Eidolon) => {
            use crate::generated::cards::CardKeyword;
            use sts2_core::Pile;

            let ethereal: Vec<_> = fight
                .cards_in(Pile::Exhaust)
                .into_iter()
                .filter(|&at| {
                    fight.card_keyword(at, CardKeyword::Ethereal)
                        && !fight.card_keyword(at, CardKeyword::Unplayable)
                })
                .collect();
            let count = ethereal.len();
            for (played, card) in ethereal.into_iter().enumerate() {
                let at = crate::behaviour::cards::standing::<V, _>(fight, card)
                    .unwrap_or_else(|| unimplemented!("Eidolon playing a card no pile holds"));
                fight.auto_play_card(at);
                // `CardCmd.AutoPlay` is awaited, so the plays left wait on
                // the answer.
                if fight.waiting() {
                    if played + 1 < count {
                        unimplemented!("Eidolon's plays waiting on a question with more to play");
                    }
                    return Some(Played::Waiting {
                        at: crate::behaviour::cards::Resume::Own(EIDOLON, 1).into(),
                    });
                }
            }
            Some(Played::Done)
        }
        // `Abundance`: `GetDistinctForCombat` of three Powers from its
        // owner's own pool, off `CombatCardGeneration`, each upgraded, and
        // the one chosen into hand, free this turn.
        Cards::Event(EventCard::Abundance) => {
            use sts2_core::{CardKind, Rules as _, RunStreamKinds as _};

            let mut powers: Vec<_> = V::combat_cards(fight.character())
                .iter()
                .copied()
                .filter(|&card| V::card_kind(card) == Some(CardKind::Power))
                .collect();
            let stream = <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION;
            let mut offered: [sts2_core::CardIdent<V>; sts2_core::CHOICES] = fight
                .generate_cards(&mut powers, stream)
                .unwrap_or_else(|_| unimplemented!("a pool offering fewer than three Powers"));
            for card in &mut offered {
                card.upgrade = 1;
            }
            Some(Played::Choosing {
                offered: offered.into_iter().collect(),
                skippable: false,
                at: crate::behaviour::cards::Resume::Event(
                    crate::behaviour::cards::event::Resume::AbundanceChoice,
                )
                .into(),
            })
        }
        _ => None,
    }
}

/// `PoisonPower.Trigger` on `creature`, where it carries Poison: `TriggerCount`
/// rounds, its stack or one more than the Accelerant its living opponents
/// carry, whichever is fewer. Each round hits it, unblockable and
/// unpowered, for the stack it holds then, and takes one off while it lives.
fn trigger_poison<F: Fight<V>>(fight: &mut F, creature: sts2_core::CombatId) {
    use sts2_core::{CreatureView as _, PowerAmount};

    let stack_of = |fight: &F| {
        fight
            .creatures()
            .find(|held| held.id() == creature)
            .and_then(|held| held.power(Power::PoisonPower))
    };
    let amount = match stack_of(fight) {
        None => return,
        Some(PowerAmount::Counted(amount)) => amount,
        Some(PowerAmount::Endless) => unimplemented!("an endless Poison"),
    };
    let side = fight
        .creatures()
        .find(|held| held.id() == creature)
        .map(|held| held.side())
        .expect("a creature with Poison stands in the fight");
    let accelerant: Number = fight
        .creatures()
        .filter(|opponent| opponent.side() != side && opponent.alive())
        .filter_map(|opponent| match opponent.power(Power::AccelerantPower) {
            Some(PowerAmount::Counted(amount)) => Some(amount),
            Some(PowerAmount::Endless) => unimplemented!("an endless Accelerant"),
            None => None,
        })
        .sum();
    for _ in 0..amount.min(1 + accelerant) {
        let Some(PowerAmount::Counted(stack)) = stack_of(fight) else {
            break;
        };
        fight.damage_unpowered(None, creature, stack, true);
        let alive = fight
            .creatures()
            .any(|held| held.id() == creature && held.alive());
        if alive {
            fight.modify_power_amount(creature, Power::PoisonPower, -1, None);
        }
    }
}

/// A play of this build's own from the command at `from`, which is past the
/// first after a wait.
pub(crate) fn on_play_from<F: Fight<V>>(card: Cards, fight: &mut F, from: u8) -> Played<V> {
    match (card, from) {
        (Cards::Defect(DefectCard::Hyperbeam), 0) => {
            fight.attack(Scaling::Dynamic, |fight| {
                let targets = fight.opponents();
                // One `CreatureCmd.Damage` over them all.
                fight.damage_command(|fight| {
                    for target in targets {
                        let hit = fight.resolve(Effect::Attack {
                            dealer: fight.actor(),
                            target,
                            damage: Amount::Var(CardVar::Damage),
                            scaling: Scaling::Dynamic,
                        });
                        fight.apply(hit);
                    }
                });
            });
            if fight.waiting() {
                return Played::Waiting {
                    at: crate::behaviour::cards::Resume::Own(card, 1).into(),
                };
            }
            on_play_from(card, fight, 1)
        }
        (Cards::Defect(DefectCard::Hyperbeam), 1) => {
            fight.apply_to_actor(
                Power::HyperbeamFocusDownPower,
                sts2_core::PowerAmount::Counted(fight.var(CardVar::FocusPower)),
            );
            Played::Done
        }
        // `Eidolon` after its last play's question: nothing more.
        (Cards::Necrobinder(NecrobinderCard::Eidolon), 1) => Played::Done,
        (card, from) => unreachable!("{card:?} has no command {from} of its own"),
    }
}

/// A number a card of this build's own works out, or nothing for a shared one.
pub(crate) fn calculated<P: sts2_core::Printed<V> + ?Sized>(
    card: Cards,
    key: CardVar,
    printed: &P,
    owner: sts2_core::CardOwner<'_, V>,
) -> Option<Number> {
    match (card, key) {
        // `ExpectAFight`: a base, and more for each Strength its owner has,
        // none for less than none.
        // `Sacrifice`: a base, and more for each of three times the maximum
        // health a living Osty has. v0.107.1's counts twice it.
        (Cards::Necrobinder(NecrobinderCard::Sacrifice), CardVar::CalculatedBlock) => {
            let thrice = owner.osty_max_hp.map_or(0, |hp| hp * 3);
            Some(
                printed.face(CardVar::CalculationBase)
                    + printed.face(CardVar::CalculationExtra) * thrice,
            )
        }
        (Cards::Ironclad(IroncladCard::ExpectAFight), CardVar::CalculatedBlock) => {
            let strength = owner
                .powers
                .iter()
                .find(|held| held.power == Power::StrengthPower)
                .map_or(0, |held| match held.amount {
                    sts2_core::PowerAmount::Counted(amount) => amount,
                    sts2_core::PowerAmount::Endless => unimplemented!("an endless Strength"),
                });
            Some(
                printed.face(CardVar::CalculationBase)
                    + printed.face(CardVar::CalculationExtra) * strength.max(0),
            )
        }
        _ => None,
    }
}

/// A deck card of this build's own, by what it saves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OwnCard {
    /// `Dowsing.RoomsEntered`.
    Dowsing { rooms_entered: u8 },
}

/// `Dowsing.maxRooms`: the `?` rooms it waits for.
const DOWSING_ROOMS: u8 = 5;

/// What a deck card of this build's own saves as it is made.
pub(crate) fn own_card_state(card: Cards) -> Option<OwnCard> {
    match card {
        Cards::Quest(QuestCard::Dowsing) => Some(OwnCard::Dowsing { rooms_entered: 0 }),
        _ => None,
    }
}

/// `Dowsing.RoomsEntered`'s setter prints `Rooms` as the rooms it has left.
pub(crate) fn own_card_written(own: OwnCard) -> Option<sts2_core::Change<V>> {
    match own {
        OwnCard::Dowsing { rooms_entered } if rooms_entered > 0 => Some(sts2_core::Change::Raise(
            CardVar::Rooms,
            -Number::from(rooms_entered),
        )),
        OwnCard::Dowsing { .. } => None,
    }
}

/// `EventHook` for a deck card of this build's own.
pub(crate) fn own_deck_card_event_hook(
    card: Cards,
    own: &mut OwnCard,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (own, hook) {
        // `Dowsing.BeforeRoomEntered`: one more `?` room, if it is the map
        // point's first, and an Abundance in its place at five.
        // `PlayerCmd.CompleteQuest` writes the map point's history, which
        // the run here does not keep.
        (
            OwnCard::Dowsing { rooms_entered },
            sts2_core::EventHook::RoomEntering { point, base },
        ) => {
            if base && point == Some(MapPointType::Unknown) {
                *rooms_entered += 1;
                if *rooms_entered >= DOWSING_ROOMS {
                    asked.push(sts2_core::HookEffect::TransformTo(Cards::Event(
                        EventCard::Abundance,
                    )));
                }
            }
        }
        (_, hook) => unimplemented!("what {card:?} in the deck does for {hook:?}"),
    }
}
