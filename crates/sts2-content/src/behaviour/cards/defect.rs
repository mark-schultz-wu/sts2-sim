//! The Defect's cards the generator leaves to a hand port.

use crate::ids::{CardIds as _, CardVarIds as _, OrbIds as _, PowerIds as _};
use sts2_core::{Amount, Effect, Fight, Pile, PileAct, Played, PowerAmount, Scaling};

/// Where a Defect card's play stops to ask something.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume<V: crate::Build> {
    /// `Hologram`, after its owner picked the card to take back.
    HologramIntoHand,
    /// `Scavenge`, after its owner picked the card to exhaust.
    ScavengeExhaust,
    /// `Overclock`, after a question inside its draw was answered.
    OverclockDrew,
    /// `Reboot`, after a question after its shuffle was answered.
    RebootShuffled,
    /// `Modded`, after a question inside its draw was answered.
    ModdedDrew,
    /// `Scrape`, after a question inside its draw was answered.
    ScrapeDrew,
    /// After a question inside the card's attack was answered.
    IceLanceHit,
    MeteorStrikeHit,
    HyperbeamHit,
    AllForOneHit,
    RefractHit,
    AdaptiveStrikeHit,
    UproarHit,
    ShatterHit,
    ClawHit,
    GoForTheEyesHit,
    GunkUpHit,
    MomentumStrikeHit,
    TeslaCoilHit,
    /// `Sunder`, whose energy comes where its attack killed.
    SunderHit,
    /// `FlakCannon`, after a question inside its exhausts was answered, with
    /// the statuses left to exhaust and this many hits to land.
    FlakCannon {
        left: crate::behaviour::cards::Left<V>,
        hits: u8,
    },
}

/// The rest of the play waits at `at`, where a command left a question and
/// more commands follow.
fn wait<V: crate::Build>(at: Resume<V>) -> Played<V> {
    Played::Waiting {
        at: crate::behaviour::cards::Resume::Defect(at).into(),
    }
}

/// The rest of a Defect card's play that stood still on a question its own
/// command raised.
pub(super) fn resume_waited<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V>,
) -> Played<V> {
    match at {
        Resume::OverclockDrew => {
            add_status::<V, _>(fight, V::Card::BURN);
            Played::Done
        }
        Resume::RebootShuffled => {
            fight.draw(fight.var(V::CardVar::CARDS));
            Played::Done
        }
        Resume::ModdedDrew => modded_drew::<V, _>(fight),
        // A draw that never ran, as the fight ended, drew nothing.
        Resume::ScrapeDrew => scrape_drew::<V, _>(fight, &[]),
        Resume::IceLanceHit => ice_lance_hit::<V, _>(fight),
        Resume::MeteorStrikeHit => meteor_strike_hit::<V, _>(fight),
        Resume::HyperbeamHit => hyperbeam_hit::<V, _>(fight),
        Resume::AllForOneHit => all_for_one_hit::<V, _>(fight),
        Resume::RefractHit => refract_hit::<V, _>(fight),
        Resume::AdaptiveStrikeHit => adaptive_strike_hit::<V, _>(fight),
        Resume::UproarHit => uproar_hit::<V, _>(fight),
        Resume::ShatterHit => shatter_hit::<V, _>(fight),
        Resume::ClawHit => claw_hit::<V, _>(fight),
        Resume::GoForTheEyesHit => go_for_the_eyes_hit::<V, _>(fight),
        Resume::GunkUpHit => gunk_up_hit::<V, _>(fight),
        Resume::MomentumStrikeHit => momentum_strike_hit::<V, _>(fight),
        Resume::TeslaCoilHit => tesla_coil_hit::<V, _>(fight),
        Resume::SunderHit => {
            let results = fight.waited_attack_results();
            sunder_hit::<V, _>(fight, results.iter().any(|result| result.killed))
        }
        Resume::FlakCannon { left, hits } => {
            let left: Vec<_> = left.cards().collect();
            flak_cannon::<V, _>(fight, &left, hits)
        }
        at => unreachable!("{at:?} stops on a pick, not on a question it waited for"),
    }
}

pub(super) fn resume_play<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V>,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        Resume::HologramIntoHand | Resume::ScavengeExhaust => {
            resume_picked::<V, _>(fight, at, chosen)
        }
        at => unreachable!("{at:?} waits on a question, not a pick"),
    }
}

/// [`resume_play`] for the stops that ask for a pick.
fn resume_picked<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V>,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        // `CardPileCmd.Add(cardModel, PileType.Hand)`.
        Resume::HologramIntoHand => {
            fight.act_on(PileAct::IntoHand, chosen.as_slice());
            Played::Done
        }
        // `CardCmd.Exhaust` of the one taken, then its `Energy` as Energy
        // Next Turn.
        Resume::ScavengeExhaust => {
            fight.act_on(PileAct::Exhaust, chosen.as_slice());
            let energy = sts2_core::PowerAmount::Counted(fight.var(V::CardVar::ENERGY));
            fight.apply_to_actor(V::Power::ENERGY_NEXT_TURN_POWER, energy);
            Played::Done
        }
        at => unreachable!("{at:?} waits on a question, not a pick"),
    }
}

/// The rest of `IceLance`, once its attack is over: its Frosts channelled.
fn ice_lance_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    for _ in 0..fight.var(V::CardVar::REPEAT) {
        fight.channel(V::Orb::FROST_ORB);
    }
    Played::Done
}

/// The rest of `MeteorStrike`, once its attack is over: three Plasma
/// channelled.
fn meteor_strike_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    for _ in 0..3 {
        fight.channel(V::Orb::PLASMA_ORB);
    }
    Played::Done
}

/// The rest of `Hyperbeam`, once its attack is over: its Focus off its
/// owner.
fn hyperbeam_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.apply_to_actor(
        V::Power::FOCUS_POWER,
        PowerAmount::Counted(-fight.var(V::CardVar::FOCUS_POWER)),
    );
    Played::Done
}

/// The rest of `AllForOne`, once its attack is over: each card of the
/// discard pile that costs nothing into hand, one at a time.
fn all_for_one_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let free: Vec<_> = fight
        .cards_in(Pile::Discard)
        .into_iter()
        .filter(|&card| {
            matches!(
                <V as sts2_core::Rules>::card_kind(fight.card_ident(card).card),
                Some(
                    sts2_core::CardKind::Attack
                        | sts2_core::CardKind::Skill
                        | sts2_core::CardKind::Power
                )
            ) && fight.card_costs(card, true).0 == sts2_core::Cost::fixed(0)
        })
        .collect();
    for card in free {
        let at = fight
            .cards_in(Pile::Discard)
            .into_iter()
            .find(|held| held.id() == card.id());
        if let Some(at) = at {
            fight.act_on(PileAct::IntoHand, &[at]);
        }
    }
    Played::Done
}

/// The rest of `Refract`, once its attack is over: its Glass orbs
/// channelled.
fn refract_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    for _ in 0..fight.var(V::CardVar::REPEAT) {
        fight.channel(V::Orb::GLASS_ORB);
    }
    Played::Done
}

/// The rest of `AdaptiveStrike`, once its attack is over: a clone of itself
/// into the discard pile, costing nothing for the fight.
fn adaptive_strike_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    if let Some(clone) = fight.clone_this(Pile::Discard) {
        fight.modify_card(
            clone,
            sts2_core::Change::SetCost(sts2_core::CostPool::Energy, 0),
            sts2_core::Lasting::ThisFight,
        );
    }
    Played::Done
}

/// The rest of `Sunder`, once its attack is over: its energy where `gains`.
fn sunder_hit<V: crate::Build, F: Fight<V>>(fight: &mut F, gains: bool) -> Played<V> {
    if gains {
        fight.modify_energy(fight.var(V::CardVar::ENERGY));
    }
    Played::Done
}

/// `FlakCannon` over the statuses `cards`: each exhausted, then `hits` hits,
/// each at an enemy drawn for it. A wait keeps the statuses still to go,
/// which have to fit a hand.
fn flak_cannon<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    cards: &[sts2_core::CardAt<V>],
    hits: u8,
) -> Played<V> {
    for (at, &card) in cards.iter().enumerate() {
        fight.act_on(PileAct::Exhaust, &[card]);
        if fight.waiting() {
            let left = crate::behaviour::cards::Left::of(&cards[at + 1..]);
            return wait::<V>(Resume::FlakCannon { left, hits });
        }
    }
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..hits {
            let Some(target) = fight.random_target() else {
                break;
            };
            let hit = fight.resolve(Effect::Attack {
                dealer: fight.actor(),
                target,
                damage: Amount::Var(V::CardVar::DAMAGE),
                scaling: Scaling::Dynamic,
            });
            fight.apply(hit);
        }
    });
    Played::Done
}

/// Its block, onto its owner.
fn block<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(V::CardVar::BLOCK),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
}

/// `CardPileCmd.AddGeneratedCardToCombat` of one `status` into the discard
/// pile.
fn add_status<V: crate::Build, F: Fight<V>>(fight: &mut F, status: V::Card) {
    fight.add_generated_cards(sts2_core::CardIdent::new(status), Pile::Discard, 1);
}

/// `Ftl`: its damage to the target, then its `Cards` drawn while its owner
/// has finished fewer plays this turn than its `PlayMax`.
fn ftl<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    hit::<V, _>(fight, fight.target(), 1);
    let finished = fight.history().finished_this_turn().count();
    let finished = crate::behaviour::cards::Number::try_from(finished)
        .expect("a turn's plays fit a game number");
    if finished < fight.var(V::CardVar::PLAY_MAX) {
        fight.draw(fight.var(V::CardVar::CARDS));
    }
    Played::Done
}

/// `Compact`: its block, then each Status in hand transformed into a Fuel,
/// upgraded where Compact is. A card in a fight is always transformable.
fn compact<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(V::CardVar::BLOCK),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    let statuses: Vec<_> = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&card| {
            V::card_kind(fight.card_ident(card).card) == Some(sts2_core::CardKind::Status)
        })
        .collect();
    for card in statuses {
        let mut into = sts2_core::CardIdent::new(V::Card::FUEL);
        into.upgrade = u8::from(fight.upgraded());
        fight.transform(card, into);
    }
    Played::Done
}

pub(super) fn on_play<V: crate::Build, F: Fight<V>>(card: V::Card, fight: &mut F) -> Played<V> {
    match card {
        // `GeneticAlgorithm`: its block, then `BuffFromPlay` of its
        // `Increase` on this copy and on its deck version.
        __c0 if __c0 == V::Card::GENETIC_ALGORITHM => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(V::CardVar::BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            let increase = fight.var(V::CardVar::INCREASE);
            fight.modify_this(
                sts2_core::Change::Own(crate::behaviour::cards::CardWrite::BuffFromPlay(increase)),
                sts2_core::Lasting::ThisFight,
            );
            fight.write_deck_version(crate::behaviour::cards::DeckWrite::BuffFromPlay(increase));
            Played::Done
        }
        // `HelixDrill`: its damage to the target, once for each hit it works
        // out, as `Barrage` does.
        __c0 if __c0 == V::Card::BARRAGE || __c0 == V::Card::HELIX_DRILL => barrage::<V, _>(fight),
        // `IceLance`: its damage to the target, then its `Repeat` of Frost orbs
        // channelled.
        __c0 if __c0 == V::Card::ICE_LANCE => {
            let target = fight.target();
            hit::<V, _>(fight, target, 1);
            if fight.waiting() {
                return wait::<V>(Resume::IceLanceHit);
            }
            ice_lance_hit::<V, _>(fight)
        }
        // `Voltaic`: a Lightning orb channelled for each its owner channelled
        // this fight before.
        __c0 if __c0 == V::Card::VOLTAIC => {
            for _ in 0..fight.var(V::CardVar::CALCULATED_CHANNELS) {
                fight.channel(V::Orb::LIGHTNING_ORB);
            }
            Played::Done
        }
        // `FlakCannon`: every status its owner holds outside the exhaust pile
        // exhausted, then its damage once for each, each hit at an enemy
        // drawn at random. The hits are counted before the statuses go.
        __c0 if __c0 == V::Card::FLAK_CANNON => {
            let hits = u8::try_from(fight.var(V::CardVar::CALCULATED_HITS))
                .expect("a count of statuses fits a u8");
            // `AllCards`: the hand, then the draw and discard piles.
            let statuses: Vec<_> = [Pile::Hand, Pile::Draw, Pile::Discard]
                .into_iter()
                .flat_map(|pile| fight.cards_in(pile))
                .filter(|&at| {
                    <V as sts2_core::Rules>::card_kind(fight.card_ident(at).card)
                        == Some(sts2_core::CardKind::Status)
                })
                .collect();
            flak_cannon::<V, _>(fight, &statuses, hits)
        }
        // `Spinner`: a Glass orb channelled once upgraded, then its Spinner.
        __c0 if __c0 == V::Card::SPINNER => {
            if fight.upgraded() {
                fight.channel(V::Orb::GLASS_ORB);
            }
            fight.apply_to_actor(
                V::Power::SPINNER_POWER,
                PowerAmount::Counted(fight.var(V::CardVar::SPINNER_POWER)),
            );
            Played::Done
        }
        __c0 if __c0 == V::Card::UPROAR => uproar::<V, _>(fight),
        __c0 if __c0 == V::Card::SHATTER => shatter::<V, _>(fight),
        // `DoubleEnergy`: as much energy again as its owner has left.
        __c0 if __c0 == V::Card::DOUBLE_ENERGY => {
            fight.modify_energy(fight.energy());
            Played::Done
        }
        // `Chill`: a Frost channelled for each enemy that can be hit, the
        // enemies counted once.
        __c0 if __c0 == V::Card::CHILL => {
            for _ in fight.hittable_opponents() {
                fight.channel(V::Orb::FROST_ORB);
            }
            Played::Done
        }
        __c0 if __c0 == V::Card::COMPACT => compact::<V, _>(fight),
        // `Modded`: its `Repeat` orb slots, its `Cards` drawn, then one more
        // energy on its own cost for the fight. A shuffle inside the draw can
        // ask, and the cost waits for it.
        __c0 if __c0 == V::Card::MODDED => {
            fight.add_orb_slots(fight.var(V::CardVar::REPEAT));
            fight.draw(fight.var(V::CardVar::CARDS));
            if fight.waiting() {
                return Played::Waiting {
                    at: crate::behaviour::cards::Resume::Defect(Resume::ModdedDrew).into(),
                };
            }
            modded_drew::<V, _>(fight)
        }
        // `MeteorStrike`: its damage to the target, then three Plasma orbs.
        __c0 if __c0 == V::Card::METEOR_STRIKE => {
            let target = fight.target();
            hit::<V, _>(fight, target, 1);
            if fight.waiting() {
                return wait::<V>(Resume::MeteorStrikeHit);
            }
            meteor_strike_hit::<V, _>(fight)
        }
        // `Chaos`: a random orb off `CombatOrbGeneration` for each of its
        // `Repeat`.
        __c0 if __c0 == V::Card::CHAOS => {
            for _ in 0..fight.var(V::CardVar::REPEAT) {
                if let Some(orb) = fight.random_orb(V::rollable_orbs()) {
                    fight.channel(orb);
                }
            }
            Played::Done
        }
        // `WhiteNoise`: a Power of the character's pool, free this turn, into
        // the hand.
        __c0 if __c0 == V::Card::WHITE_NOISE => {
            crate::behaviour::free_card_into_hand(
                fight,
                |card| <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Power),
                true,
            );
            Played::Done
        }
        // `MultiCast`: the first orb evoked X times, one more upgraded, and
        // dequeued only on the last.
        __c0 if __c0 == V::Card::MULTI_CAST => {
            let mut count = fight.x_value();
            if fight.upgraded() {
                count += 1;
            }
            for at in 0..count {
                fight.evoke_next(at == count - 1);
            }
            Played::Done
        }
        // `Hyperbeam`: its damage to every enemy, then its `FocusPower` taken
        // off its owner.
        __c0 if __c0 == V::Card::HYPERBEAM => {
            fight.attack(Scaling::Dynamic, |fight| {
                let targets = fight.opponents();
                // One `CreatureCmd.Damage` over them all.
                fight.damage_command(|fight| {
                    for target in targets {
                        let hit = fight.resolve(Effect::Attack {
                            dealer: fight.actor(),
                            target,
                            damage: Amount::Var(V::CardVar::DAMAGE),
                            scaling: Scaling::Dynamic,
                        });
                        fight.apply(hit);
                    }
                });
            });
            if fight.waiting() {
                return wait::<V>(Resume::HyperbeamHit);
            }
            hyperbeam_hit::<V, _>(fight)
        }
        // `AllForOne`: its damage to the target, then every Attack, Skill or
        // Power in the discard pile that costs nothing into the hand.
        __c0 if __c0 == V::Card::ALL_FOR_ONE => {
            let target = fight.target();
            hit::<V, _>(fight, target, 1);
            if fight.waiting() {
                return wait::<V>(Resume::AllForOneHit);
            }
            all_for_one_hit::<V, _>(fight)
        }
        // `SweepingBeam`: its damage to every enemy, then its `Cards` drawn.
        __c0 if __c0 == V::Card::SWEEPING_BEAM => {
            fight.attack(Scaling::Dynamic, |fight| {
                let targets = fight.opponents();
                // One `CreatureCmd.Damage` over them all.
                fight.damage_command(|fight| {
                    for target in targets {
                        let hit = fight.resolve(Effect::Attack {
                            dealer: fight.actor(),
                            target,
                            damage: Amount::Var(V::CardVar::DAMAGE),
                            scaling: Scaling::Dynamic,
                        });
                        fight.apply(hit);
                    }
                });
            });
            fight.draw(fight.var(V::CardVar::CARDS));
            Played::Done
        }
        // `Refract`: its damage to the target twice, then a Glass orb for
        // each of its `Repeat`.
        __c0 if __c0 == V::Card::REFRACT => {
            let target = fight.target();
            hit::<V, _>(fight, target, 2);
            if fight.waiting() {
                return wait::<V>(Resume::RefractHit);
            }
            refract_hit::<V, _>(fight)
        }
        // `Darkness`: a Dark orb, then every Dark orb's passive, once or twice
        // upgraded, one orb after another.
        __c0 if __c0 == V::Card::DARKNESS => {
            fight.channel(V::Orb::DARK_ORB);
            let times = if fight.upgraded() { 2 } else { 1 };
            let dark: Vec<usize> = fight
                .channelled()
                .iter()
                .enumerate()
                .filter(|(_, held)| held.orb == V::Orb::DARK_ORB)
                .map(|(place, _)| place)
                .collect();
            for place in dark {
                for _ in 0..times {
                    fight.orb_passive_at(place);
                }
            }
            Played::Done
        }
        // `Synchronize`: its calculated Focus, lent until the turn ends.
        __c0 if __c0 == V::Card::SYNCHRONIZE => {
            fight.apply_to_actor(
                V::Power::SYNCHRONIZE_POWER,
                PowerAmount::Counted(fight.var(V::CardVar::CALCULATED_FOCUS)),
            );
            Played::Done
        }
        // `ConsumingShadow`: a Dark orb for each of its `Repeat`, then its
        // power on its owner.
        __c0 if __c0 == V::Card::CONSUMING_SHADOW => {
            for _ in 0..fight.var(V::CardVar::REPEAT) {
                fight.channel(V::Orb::DARK_ORB);
            }
            fight.apply_to_actor(
                V::Power::CONSUMING_SHADOW_POWER,
                PowerAmount::Counted(fight.var(V::CardVar::CONSUMING_SHADOW_POWER)),
            );
            Played::Done
        }
        __c0 if __c0 == V::Card::FTL => ftl::<V, _>(fight),
        // `Reboot`: the whole hand into the draw pile, shuffled, then its
        // `Cards` drawn. A question after the shuffle stands the draw still.
        __c0 if __c0 == V::Card::REBOOT => {
            let hand = fight.cards_in(Pile::Hand);
            fight.act_on(PileAct::IntoDraw, &hand);
            fight.shuffle();
            if fight.waiting() {
                return Played::Waiting {
                    at: crate::behaviour::cards::Resume::Defect(Resume::RebootShuffled).into(),
                };
            }
            fight.draw(fight.var(V::CardVar::CARDS));
            Played::Done
        }
        // `Turbo`: its energy, then a Void into the discard pile.
        __c0 if __c0 == V::Card::TURBO => {
            fight.modify_energy(fight.var(V::CardVar::ENERGY));
            add_status::<V, _>(fight, V::Card::VOID);
            Played::Done
        }
        // `Sunder`: its damage to the target, then its energy where a result
        // says `WasTargetKilled`, whatever brought the target back after.
        __c0 if __c0 == V::Card::SUNDER => {
            let target = fight.target();
            let results = fight.attack(Scaling::Dynamic, |fight| {
                let attack = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(V::CardVar::DAMAGE),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(attack);
            });
            if fight.waiting() {
                return wait::<V>(Resume::SunderHit);
            }
            sunder_hit::<V, _>(fight, results.iter().any(|result| result.killed))
        }
        // `Capacitor`: its `Repeat` more orb slots.
        __c0 if __c0 == V::Card::CAPACITOR => {
            fight.add_orb_slots(fight.var(V::CardVar::REPEAT));
            Played::Done
        }
        // `Scavenge`: one card of the hand exhausted, then energy next turn.
        __c0 if __c0 == V::Card::SCAVENGE => Played::Asking {
            pile: Pile::Hand,
            selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
            at: crate::behaviour::cards::Resume::Defect(Resume::ScavengeExhaust).into(),
        },
        // `Glacier`: its block, then two Frosts channelled.
        __c0 if __c0 == V::Card::GLACIER => {
            block::<V, _>(fight);
            fight.channel(V::Orb::FROST_ORB);
            fight.channel(V::Orb::FROST_ORB);
            Played::Done
        }
        // `Overclock`: its `Cards` drawn, then a Burn into the discard pile.
        // A shuffle inside the draw can ask, and the Burn waits for it.
        __c0 if __c0 == V::Card::OVERCLOCK => {
            fight.draw(fight.var(V::CardVar::CARDS));
            if fight.waiting() {
                return Played::Waiting {
                    at: crate::behaviour::cards::Resume::Defect(Resume::OverclockDrew).into(),
                };
            }
            add_status::<V, _>(fight, V::Card::BURN);
            Played::Done
        }
        // `AdaptiveStrike`: its damage to the target, then a clone of itself
        // into the discard pile, free for the rest of the fight.
        __c0 if __c0 == V::Card::ADAPTIVE_STRIKE => {
            hit::<V, _>(fight, fight.target(), 1);
            if fight.waiting() {
                return wait::<V>(Resume::AdaptiveStrikeHit);
            }
            adaptive_strike_hit::<V, _>(fight)
        }
        // `BoostAway`: its block, then a Dazed into the discard pile.
        __c0 if __c0 == V::Card::BOOST_AWAY => {
            block::<V, _>(fight);
            add_status::<V, _>(fight, V::Card::DAZED);
            Played::Done
        }
        // `FightThrough`: its block, then two Wounds into the discard pile,
        // one at a time.
        __c0 if __c0 == V::Card::FIGHT_THROUGH => {
            block::<V, _>(fight);
            add_status::<V, _>(fight, V::Card::WOUND);
            add_status::<V, _>(fight, V::Card::WOUND);
            Played::Done
        }
        // `BulkUp`: its `OrbSlots` fewer orb slots, then its Strength and its
        // Dexterity.
        __c0 if __c0 == V::Card::BULK_UP => {
            fight.remove_orb_slots(fight.var(V::CardVar::ORB_SLOTS));
            let strength = fight.var(V::CardVar::STRENGTH_POWER);
            fight.apply_to_actor(
                V::Power::STRENGTH_POWER,
                sts2_core::PowerAmount::Counted(strength),
            );
            let dexterity = fight.var(V::CardVar::DEXTERITY_POWER);
            fight.apply_to_actor(
                V::Power::DEXTERITY_POWER,
                sts2_core::PowerAmount::Counted(dexterity),
            );
            Played::Done
        }
        // `Quadcast`: with any orb held, the first orb evoked its `Repeat`
        // times, leaving on the last.
        __c0 if __c0 == V::Card::QUADCAST => {
            if fight.orbs_held() > 0 {
                let repeat = fight.var(V::CardVar::REPEAT);
                for evoked in 1..=repeat {
                    fight.evoke_next(evoked == repeat);
                }
            }
            Played::Done
        }
        __c0 if __c0 == V::Card::CLAW => claw::<V, _>(fight),
        __c0 if __c0 == V::Card::COMPILE_DRIVER => compile_driver::<V, _>(fight),
        __c0 if __c0 == V::Card::GO_FOR_THE_EYES => go_for_the_eyes::<V, _>(fight),
        __c0 if __c0 == V::Card::GUNK_UP => gunk_up::<V, _>(fight),
        __c0 if __c0 == V::Card::HOLOGRAM => hologram::<V, _>(fight),
        __c0 if __c0 == V::Card::DUALCAST => dualcast::<V, _>(fight),
        __c0 if __c0 == V::Card::TEMPEST => tempest::<V, _>(fight),
        __c0 if __c0 == V::Card::SCRAPE => scrape::<V, _>(fight),
        __c0 if __c0 == V::Card::MOMENTUM_STRIKE => momentum_strike::<V, _>(fight),
        __c0 if __c0 == V::Card::TESLA_COIL => tesla_coil::<V, _>(fight),
        card => crate::behaviour::cards::generated_from::<V, _>(card, fight, 0),
    }
}

/// Its damage to `target`, `hits` times, as one attack.
fn hit<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    target: sts2_core::CombatId,
    hits: crate::behaviour::cards::Number,
) {
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..hits {
            let attack = fight.resolve(Effect::Attack {
                dealer: fight.actor(),
                target,
                damage: Amount::Var(V::CardVar::DAMAGE),
                scaling: Scaling::Dynamic,
            });
            fight.apply(attack);
        }
    });
}

/// `Barrage`: its damage to the target, once for each of its
/// `CalculatedHits`.
fn barrage<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    let hits = fight.var(V::CardVar::CALCULATED_HITS);
    hit::<V, _>(fight, target, hits);
    Played::Done
}

/// `Claw`: its damage to the target, then its `Increase` onto the damage of
/// every Claw its owner holds in the fight, itself included.
fn claw<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    hit::<V, _>(fight, target, 1);
    if fight.waiting() {
        return wait::<V>(Resume::ClawHit);
    }
    claw_hit::<V, _>(fight)
}

/// The rest of `Claw`, once its attack is over: every Claw's damage raised.
fn claw_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let raise = sts2_core::Change::Own(crate::behaviour::cards::CardWrite::BuffFromPlay(
        fight.var(V::CardVar::INCREASE),
    ));
    // `AllCards`: the hand, the draw, discard and exhaust piles, then the
    // play pile, which holds the Claw being played.
    for pile in [Pile::Hand, Pile::Draw, Pile::Discard, Pile::Exhaust] {
        for card in fight.cards_in(pile) {
            if fight.card_ident(card).card == V::Card::CLAW {
                fight.modify_card(card, raise, sts2_core::Lasting::ThisFight);
            }
        }
    }
    fight.modify_this(raise, sts2_core::Lasting::ThisFight);
    Played::Done
}

/// `CompileDriver`: its damage to the target, then as many cards drawn as
/// its `CalculatedCards`.
fn compile_driver<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    hit::<V, _>(fight, target, 1);
    let cards = fight.var(V::CardVar::CALCULATED_CARDS);
    fight.draw(cards);
    Played::Done
}

/// `GoForTheEyes`: its damage to the target, then its Weak on it where the
/// target `IntendsToAttack`: an attack or a death blow among its intents.
fn go_for_the_eyes<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    hit::<V, _>(fight, target, 1);
    if fight.waiting() {
        return wait::<V>(Resume::GoForTheEyesHit);
    }
    go_for_the_eyes_hit::<V, _>(fight)
}

/// The rest of `GoForTheEyes`, once its attack is over: its Weak, where the
/// target means to attack.
fn go_for_the_eyes_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    use crate::generated::monsters::IntentKind;
    use sts2_core::CreatureView as _;

    let target = fight.target();
    let attacking = fight
        .creatures()
        .find(|creature| creature.id() == target)
        .is_some_and(|creature| {
            creature
                .intents()
                .any(|intent| matches!(intent, IntentKind::Attack | IntentKind::DeathBlow))
        });
    if attacking {
        let weak = sts2_core::PowerAmount::Counted(fight.var(V::CardVar::WEAK_POWER));
        fight.apply_power(target, V::Power::WEAK_POWER, weak);
    }
    Played::Done
}

/// `GunkUp`: its damage to the target, once for each of its `Repeat`, then a
/// Slimed into the discard pile.
fn gunk_up<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    let hits = fight.var(V::CardVar::REPEAT);
    hit::<V, _>(fight, target, hits);
    if fight.waiting() {
        return wait::<V>(Resume::GunkUpHit);
    }
    gunk_up_hit::<V, _>(fight)
}

/// The rest of `GunkUp`, once its attack is over: a Slimed into the discard
/// pile.
fn gunk_up_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.add_generated_cards(sts2_core::CardIdent::new(V::Card::SLIMED), Pile::Discard, 1);
    Played::Done
}

/// `Hologram`: its block, then one card of the discard pile into the hand.
fn hologram<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(V::CardVar::BLOCK),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    Played::Asking {
        pile: Pile::Discard,
        selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: crate::behaviour::cards::Resume::Defect(Resume::HologramIntoHand).into(),
    }
}

/// `MomentumStrike`: its damage to the target, then its own cost set to
/// nothing for the rest of the fight.
fn momentum_strike<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    fight.attack(Scaling::Dynamic, |fight| {
        let attack = fight.resolve(Effect::Attack {
            dealer: fight.actor(),
            target,
            damage: Amount::Var(V::CardVar::DAMAGE),
            scaling: Scaling::Dynamic,
        });
        fight.apply(attack);
    });
    if fight.waiting() {
        return wait::<V>(Resume::MomentumStrikeHit);
    }
    momentum_strike_hit::<V, _>(fight)
}

/// The rest of `MomentumStrike`, once its attack is over: its own cost to
/// nothing for the fight.
fn momentum_strike_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.modify_this(
        sts2_core::Change::SetCost(sts2_core::CostPool::Energy, 0),
        sts2_core::Lasting::ThisFight,
    );
    Played::Done
}

/// `Scrape`: its damage to the target, then its `Cards` drawn, and each drawn
/// card whose own cost is anything but nothing discarded.
fn scrape<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    fight.attack(Scaling::Dynamic, |fight| {
        let attack = fight.resolve(Effect::Attack {
            dealer: fight.actor(),
            target,
            damage: Amount::Var(V::CardVar::DAMAGE),
            scaling: Scaling::Dynamic,
        });
        fight.apply(attack);
    });
    let drew = fight.draw(fight.var(V::CardVar::CARDS));
    // A shuffle inside the draw can ask, and the discard waits for it.
    if fight.waiting() {
        return Played::Waiting {
            at: crate::behaviour::cards::Resume::Defect(Resume::ScrapeDrew).into(),
        };
    }
    scrape_drew::<V, _>(fight, &drew)
}

/// `Scrape`, once its draw is over: each copy it drew whose own cost is
/// anything but nothing discarded, wherever it stands.
fn scrape_drew<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    drew: &[sts2_core::CardId<V>],
) -> Played<V> {
    let costly: Vec<sts2_core::CardAt<V>> = drew
        .iter()
        .map(|&id| {
            fight
                .find_card(id)
                .unwrap_or_else(|| unimplemented!("Scrape discarding {id:?}, which left the fight"))
        })
        .filter(|&card| fight.local_cost(card) != sts2_core::Cost::fixed(0))
        .collect();
    fight.act_on(PileAct::Discard, &costly);
    Played::Done
}

/// The rest of a Defect card's play that waited on its own draw, with every
/// copy that draw drew.
pub(super) fn resume_drew<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V>,
    drew: &[sts2_core::CardId<V>],
) -> Played<V> {
    match at {
        Resume::ScrapeDrew => scrape_drew::<V, _>(fight, drew),
        at => resume_waited::<V, _>(fight, at),
    }
}

/// `Tempest`: a Lightning channelled for each energy of its X, and one more
/// once upgraded.
fn tempest<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let orbs = fight.x_value() + crate::behaviour::cards::Number::from(fight.upgraded());
    for _ in 0..orbs {
        fight.channel(V::Orb::LIGHTNING_ORB);
    }
    Played::Done
}

/// `Shatter`: its damage to every opponent. Then, once for each orb held as
/// it began, the first orb is evoked twice: once where it stands, then once
/// as it leaves.
fn shatter<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.attack(Scaling::Dynamic, |fight| {
        let targets = fight.opponents();
        // One `CreatureCmd.Damage` over them all.
        fight.damage_command(|fight| {
            for target in targets {
                let attack = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(V::CardVar::DAMAGE),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(attack);
            }
        });
    });
    if fight.waiting() {
        return wait::<V>(Resume::ShatterHit);
    }
    shatter_hit::<V, _>(fight)
}

/// The rest of `Shatter`, once its attack is over: each orb evoked twice.
fn shatter_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    for _ in 0..fight.orbs_held() {
        fight.evoke_next(false);
        fight.evoke_next(true);
    }
    Played::Done
}

/// `Dualcast`. Evoke the first orb twice: once where it stands, then once
/// as it leaves. Nothing with no orb.
fn dualcast<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    if fight.orbs_held() > 0 {
        fight.evoke_next(false);
        fight.evoke_next(true);
    }
    Played::Done
}

/// `TeslaCoil`: its `Damage` at its target, then each Lightning's passive at
/// the same target, twice for an upgraded copy. The Lightnings are listed as
/// the damage leaves them.
fn tesla_coil<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    fight.attack(Scaling::Dynamic, |fight| {
        let hit = fight.resolve(Effect::Attack {
            dealer: fight.actor(),
            target,
            damage: Amount::Var(V::CardVar::DAMAGE),
            scaling: Scaling::Dynamic,
        });
        fight.apply(hit);
    });
    if fight.waiting() {
        return wait::<V>(Resume::TeslaCoilHit);
    }
    tesla_coil_hit::<V, _>(fight)
}

/// The rest of `TeslaCoil`, once its attack is over: each Lightning's passive
/// on the target.
fn tesla_coil_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    let times = if fight.upgraded() { 2 } else { 1 };
    let lightning: Vec<_> = fight
        .channelled()
        .into_iter()
        .filter(|orb| orb.orb == V::Orb::LIGHTNING_ORB)
        .collect();
    for orb in &lightning {
        for _ in 0..times {
            crate::behaviour::orbs::passive_at(orb, target, fight);
        }
    }
    Played::Done
}

/// `Uproar`: its damage to the target twice, then an Attack from the draw
/// pile auto-played. The first of the playable Attacks stably shuffled off
/// `Shuffle`, or where none is playable, the first of all of them.
fn uproar<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..2 {
            let attack = fight.resolve(Effect::Attack {
                dealer: fight.actor(),
                target,
                damage: Amount::Var(V::CardVar::DAMAGE),
                scaling: Scaling::Dynamic,
            });
            fight.apply(attack);
        }
    });
    if fight.waiting() {
        return wait::<V>(Resume::UproarHit);
    }
    uproar_hit::<V, _>(fight)
}

/// The rest of `Uproar`, once its attack is over: an Attack of the draw
/// pile auto-played.
fn uproar_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    use crate::generated::cards::CardKeyword;
    use sts2_core::{CardKind, RunStreamKinds as _};

    let stream = <V as sts2_core::ContentVersion>::RunRngType::SHUFFLE;
    let attacks: Vec<_> = fight
        .cards_in(Pile::Draw)
        .into_iter()
        .filter(|&at| V::card_kind(fight.card_ident(at).card) == Some(CardKind::Attack))
        .collect();
    let mut playable: Vec<_> = attacks
        .iter()
        .copied()
        .filter(|&at| !fight.card_keyword(at, CardKeyword::Unplayable))
        .collect();
    fight.stable_shuffle_cards(&mut playable, stream);
    let chosen = match playable.first() {
        Some(&card) => Some(card),
        None => {
            let mut attacks = attacks;
            fight.stable_shuffle_cards(&mut attacks, stream);
            attacks.first().copied()
        }
    };
    if let Some(card) = chosen {
        fight.auto_play_card(card);
    }
    Played::Done
}

/// `Modded`, once its draw is over: its own energy cost one higher for the
/// rest of the fight.
fn modded_drew<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.modify_this(
        sts2_core::Change::ChangeCost(sts2_core::CostPool::Energy, 1),
        sts2_core::Lasting::ThisFight,
    );
    Played::Done
}
