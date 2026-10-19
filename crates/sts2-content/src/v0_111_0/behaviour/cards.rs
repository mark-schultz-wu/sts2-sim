//! The cards whose rules this build ports for itself, which the shared cards
//! hand over through [`crate::build::OwnRules`].

use crate::behaviour::cards::Left;
use sts2_core::{Amount, Effect, Fight, Played, Scaling};

use super::super::V0_111_0 as V;
use super::super::generated::cards::{
    CardVar, Cards, DefectCard, EventCard, IroncladCard, NecrobinderCard, QuestCard, RegentCard,
    SilentCard, TokenCard,
};
use super::super::generated::powers::Power;
use crate::generated::runstate::MapPointType;

type Number = i32;

/// `Eidolon`, which waits on its last play's question.
const EIDOLON: Cards = Cards::Necrobinder(NecrobinderCard::Eidolon);

/// `Tracking`'s `PowerCmd.Apply<TrackingPower>(…, 50m, …)`, which it prints
/// nowhere.
const TRACKING: Number = 50;

/// Whether a shared card's in-fight hooks are this build's own.
pub(crate) fn own_card_hooks(card: Cards) -> bool {
    matches!(card, Cards::Defect(DefectCard::RocketPunch))
}

/// `GetResultLocationForCardPlay`, for a card this build ports for itself.
pub(crate) fn result_pile(
    card: Cards,
    base: (Option<sts2_core::Pile>, sts2_core::PilePosition),
) -> Option<sts2_core::HookResponse<(Option<sts2_core::Pile>, sts2_core::PilePosition)>> {
    use sts2_core::{HookResponse, Pile, PilePosition};

    match card {
        // `ShiningStrike`: the top of the draw pile where the base class
        // discards. v0.107.1's put itself there in its own play.
        Cards::Regent(RegentCard::ShiningStrike) => Some(HookResponse::Decision(match base {
            (Some(Pile::Discard), _) => (Some(Pile::Draw), PilePosition::Top),
            base => base,
        })),
        _ => None,
    }
}

/// A card's `EventHook` in a fight, for one this build ports for itself.
pub(crate) fn own_card_event_hook(
    card: Cards,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) -> Option<()> {
    use sts2_core::Rules as _;

    match (card, hook) {
        // `RocketPunch.AfterCardGeneratedForCombat`: a Status its owner made
        // takes one off its cost until it is played. v0.107.1's set the cost
        // to nothing.
        (
            Cards::Defect(DefectCard::RocketPunch),
            sts2_core::EventHook::CardGeneratedForCombat {
                card,
                creator: Some(_),
                ..
            },
        ) => {
            if V::card_kind(card) == Some(sts2_core::CardKind::Status) {
                asked.push(sts2_core::HookEffect::ModifySelf {
                    change: sts2_core::Change::ChangeCost(sts2_core::CostPool::Energy, -1),
                    lasting: sts2_core::Lasting::UntilPlayed,
                });
            }
            Some(())
        }
        _ => None,
    }
}

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
        Cards::Defect(DefectCard::Hyperbeam) => Some(on_play_from(card, fight, 0, Left::of(&[]))),
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
        // `Scrape`: its damage to the target, then its `Cards` drawn, and
        // each copy drawn whose cost with every modifier is anything but
        // nothing discarded. v0.107.1's read the copy's own modifiers alone.
        Cards::Defect(DefectCard::Scrape) => {
            let target = fight.target();
            fight.attack(Scaling::Dynamic, |fight| {
                let attack = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(CardVar::Damage),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(attack);
            });
            let drew = fight.draw(fight.var(CardVar::Cards));
            // A shuffle inside the draw can ask, and the discard waits for it.
            if fight.waiting() {
                return Some(Played::Waiting {
                    at: crate::behaviour::cards::Resume::Own(card, 1, Left::of(&[])).into(),
                });
            }
            Some(on_drew_from(card, fight, 1, &drew))
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
            Some(eidolon_plays(fight, &ethereal))
        }
        // `Abundance`: `GetDistinctForCombat` of three Powers from its
        // owner's own pool, off `CombatCardGeneration`, each upgraded, and
        // the one chosen into hand, free this turn.
        // `MadScience` with Violence: its damage to the target, as one
        // attack of `ViolenceHits` hits. v0.107.1's made each hit its own.
        Cards::Event(EventCard::MadScienceViolence) => {
            let target = fight.target();
            let hits = usize::try_from(fight.var(CardVar::ViolenceHits))
                .expect("a count of hits is not negative");
            crate::behaviour::cards::event::hit::<V, _>(fight, target, hits);
            if fight.waiting() {
                return Some(Played::Waiting {
                    at: crate::behaviour::cards::Resume::Own(card, 1, Left::of(&[])).into(),
                });
            }
            Some(Played::Done)
        }
        // `MadScience` with Chaos: its block, then a card of the character's
        // pool into hand through `AddGeneratedCardToCombat`, free this turn.
        // v0.107.1's added it as any other card.
        Cards::Event(EventCard::MadScienceChaos) => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(CardVar::Block),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            crate::behaviour::free_card_into_hand(fight, |_| true, true);
            Some(Played::Done)
        }
        // `ShiningStrike`: its damage to the target, then its stars. Where it
        // goes after is its own `GetResultLocationForCardPlay`.
        Cards::Regent(RegentCard::ShiningStrike) => {
            let target = fight.target();
            fight.attack(Scaling::Dynamic, |fight| {
                let attack = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(CardVar::Damage),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(attack);
            });
            if fight.waiting() {
                return Some(Played::Waiting {
                    at: crate::behaviour::cards::Resume::Own(card, 1, Left::of(&[])).into(),
                });
            }
            Some(on_play_from(card, fight, 1, Left::of(&[])))
        }
        Cards::Necrobinder(NecrobinderCard::Misery) => Some(misery(fight)),
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
pub(crate) fn on_play_from<F: Fight<V>>(
    card: Cards,
    fight: &mut F,
    from: u8,
    left: Left<V>,
) -> Played<V> {
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
                    at: crate::behaviour::cards::Resume::Own(card, 1, Left::of(&[])).into(),
                };
            }
            on_play_from(card, fight, 1, Left::of(&[]))
        }
        (Cards::Defect(DefectCard::Hyperbeam), 1) => {
            fight.apply_to_actor(
                Power::HyperbeamFocusDownPower,
                sts2_core::PowerAmount::Counted(fight.var(CardVar::FocusPower)),
            );
            Played::Done
        }
        // `Eidolon` after a play's question: the plays its list has left.
        // `ShiningStrike` after its attack: its stars.
        (Cards::Regent(RegentCard::ShiningStrike), 1) => {
            fight.gain_stars(fight.var(CardVar::Stars));
            Played::Done
        }
        // Mad Science's Violence after its attack's question: nothing more.
        (Cards::Event(EventCard::MadScienceViolence), 1) => Played::Done,
        (Cards::Necrobinder(NecrobinderCard::Eidolon), 1) => {
            let left: Vec<_> = left.cards().collect();
            eidolon_plays(fight, &left)
        }
        (card, from) => unreachable!("{card:?} has no command {from} of its own"),
    }
}

/// The rest of a play of this build's own that waited on its own draw, with
/// every copy the draw drew.
pub(crate) fn on_drew_from<F: Fight<V>>(
    card: Cards,
    fight: &mut F,
    from: u8,
    drew: &[sts2_core::CardId<V>],
) -> Played<V> {
    use sts2_core::{Cost, PileAct};

    match (card, from) {
        // `Scrape` once its draw is over.
        (Cards::Defect(DefectCard::Scrape), 1) => {
            let costly: Vec<_> = drew
                .iter()
                .map(|&id| {
                    fight.find_card(id).unwrap_or_else(|| {
                        unimplemented!("Scrape discarding {id:?}, which left the fight")
                    })
                })
                .filter(|&card| fight.card_costs(card, true).0 != Cost::fixed(0))
                .collect();
            fight.act_on(PileAct::Discard, &costly);
            Played::Done
        }
        (card, from) => on_play_from(card, fight, from, Left::of(&[])),
    }
}

/// `Misery`: the target's debuffs as they stand, each temporary power's
/// amount added onto the debuff it lends. Then its damage to the target.
/// Each other hittable enemy takes each amount that is not zero, from the
/// debuff's applier. `PowerCmd.ModifyAmount` puts it onto an instance it
/// stacks with, and `PowerCmd.Apply` puts on a copy otherwise.
fn misery<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use super::super::generated::powers::PowerInstanceType;
    use sts2_core::{PowerAmount, Table as _};

    let target = fight.target();
    let debuffs: Vec<_> = fight
        .powers_on(target)
        .into_iter()
        .filter(|held| crate::behaviour::cards::debuff_for_amount::<V>(held.power, held.amount))
        .collect();
    let mut amounts: Vec<Number> = debuffs
        .iter()
        .map(|held| match held.amount {
            PowerAmount::Counted(amount) => amount,
            PowerAmount::Endless => unimplemented!("Misery copying an endless {:?}", held.power),
        })
        .collect();
    for (at, held) in debuffs.iter().enumerate() {
        if let Some((lent, _)) = crate::behaviour::powers::lends::<V>(held.power)
            && let Some(onto) = debuffs.iter().position(|other| other.power == lent)
        {
            amounts[onto] += amounts[at];
        }
    }
    crate::behaviour::cards::necrobinder::strike::<V, _>(fight);
    for enemy in fight.hittable_opponents() {
        if enemy == target {
            continue;
        }
        for (held, &amount) in debuffs.iter().zip(&amounts) {
            if amount == 0 {
                continue;
            }
            let power = held.power;
            // `ClonePreservingMutability` builds its internal data anew,
            // which is all a noted play is.
            if !held.fresh && !crate::behaviour::powers::clone_starts_anew::<V>(power) {
                unimplemented!("Misery copying {power:?}, whose model carries state");
            }
            let stacks = |on: &sts2_core::HeldInstance<V>| match power.data().instance {
                None => on.power == power,
                Some(PowerInstanceType::InstancedPerApplier) => {
                    on.power == power && on.applier == held.applier
                }
                Some(PowerInstanceType::Instanced) => false,
            };
            if fight.powers_on(enemy).iter().any(stacks) {
                fight.modify_power_amount(enemy, power, amount, held.applier);
                continue;
            }
            let copy = <crate::behaviour::powers::State<V> as sts2_core::PowerModel<V>>::new(power);
            fight.apply_power_copy(enemy, copy, PowerAmount::Counted(amount), held.applier);
        }
    }
    Played::Done
}

/// `Eidolon`'s `CardCmd.AutoPlay` of each of `cards`, in order. Each is
/// awaited, so the plays after one that asks wait on the answer.
fn eidolon_plays<F: Fight<V>>(fight: &mut F, cards: &[sts2_core::CardAt<V>]) -> Played<V> {
    for (played, &card) in cards.iter().enumerate() {
        let at = crate::behaviour::cards::standing::<V, _>(fight, card)
            .unwrap_or_else(|| unimplemented!("Eidolon playing a card no pile holds"));
        fight.auto_play_card(at);
        if fight.waiting() {
            let left = Left::of(&cards[played + 1..]);
            return Played::Waiting {
                at: crate::behaviour::cards::Resume::Own(EIDOLON, 1, left).into(),
            };
        }
    }
    Played::Done
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
