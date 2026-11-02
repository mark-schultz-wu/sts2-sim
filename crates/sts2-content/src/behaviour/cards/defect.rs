//! The Defect's cards the generator leaves to a hand port.

use crate::behaviour::Played;
use crate::behaviour::paused::{Ask, Lent};
use crate::ids::{CardIds as _, CardVarIds as _, OrbIds as _, PowerIds as _};
use sts2_core::{Amount, Effect, Fight, FightKind, Pile, PileAct, PowerAmount, Scaling};

/// One card, as a pick asks for it.
const ONE: sts2_core::Selection = sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE);

/// The rest of `IceLance`, once its attack is over: its Frosts channelled.
fn ice_lance_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    // The game's loop reads its count before each pass.
    let mut pass = 0;
    while pass < fight.var(V::CardVar::REPEAT) {
        pass += 1;
        fight.channel(V::Orb::FROST_ORB);
    }
}

/// The rest of `MeteorStrike`, once its attack is over: three Plasma
/// channelled.
fn meteor_strike_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    for _ in 0..3 {
        fight.channel(V::Orb::PLASMA_ORB);
    }
}

/// The rest of `Hyperbeam`, once its attack is over: its Focus off its
/// owner.
fn hyperbeam_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    fight.apply_to_actor(
        V::Power::FOCUS_POWER,
        PowerAmount::Counted(-fight.var(V::CardVar::FOCUS_POWER)),
    );
}

/// The rest of `AllForOne`, once its attack is over: each card of the
/// discard pile that costs nothing into hand, one at a time.
fn all_for_one_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
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
}

/// The rest of `Refract`, once its attack is over: its Glass orbs
/// channelled.
fn refract_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    // The game's loop reads its count before each pass.
    let mut pass = 0;
    while pass < fight.var(V::CardVar::REPEAT) {
        pass += 1;
        fight.channel(V::Orb::GLASS_ORB);
    }
}

/// The rest of `AdaptiveStrike`, once its attack is over: a clone of itself
/// into the discard pile, costing nothing for the fight.
fn adaptive_strike_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    if let Some(clone) = fight.clone_this(Pile::Discard) {
        fight.modify_card(
            clone,
            sts2_core::Change::SetCost(sts2_core::CostPool::Energy, 0),
            sts2_core::Lasting::ThisFight,
        );
    }
}

/// The rest of `Sunder`, once its attack is over: its energy where `gains`.
fn sunder_hit<V: crate::Build, F: Fight<V>>(fight: &mut F, gains: bool) {
    if gains {
        fight.modify_energy(fight.var(V::CardVar::ENERGY));
    }
}

/// `Sunder`'s attack, and whether a result says `WasTargetKilled`.
fn sunder_attack<V: crate::Build, F: Fight<V>>(fight: &mut F) -> bool {
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
    results.iter().any(|result| result.killed)
}

/// Its damage to every enemy, as one `CreatureCmd.Damage`.
fn hit_all<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    fight.attack(Scaling::Dynamic, |fight| {
        let targets = fight.opponents();
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
}

/// `FlakCannon`'s statuses: every one its owner holds outside the exhaust
/// pile, as `AllCards` lists them, the hand, then the draw and discard
/// piles.
fn statuses<V: crate::Build, F: Fight<V>>(fight: &F) -> crate::behaviour::cards::Listed<V> {
    [Pile::Hand, Pile::Draw, Pile::Discard]
        .into_iter()
        .flat_map(|pile| fight.cards_in(pile))
        .filter(|&at| {
            <V as sts2_core::Rules>::card_kind(fight.card_ident(at).card)
                == Some(sts2_core::CardKind::Status)
        })
        .collect()
}

/// `FlakCannon`'s volley, once its exhausts are over: `hits` hits, each at
/// an enemy drawn for it.
fn flak_cannon_hits<V: crate::Build, F: Fight<V>>(fight: &mut F, hits: u8) {
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
fn ftl<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
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
fn compact<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
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

pub(super) fn on_play<'a, V: crate::Build, K: FightKind<V>>(
    card: V::Card,
    fight: &mut K::Fight<'a>,
) -> Played<V, K> {
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
        // `EnergySurge`: its energy to each living player, its owner too.
        __c0 if __c0 == V::Card::ENERGY_SURGE => {
            let energy = fight.var(V::CardVar::ENERGY);
            for player in crate::behaviour::cards::teammates::<V, _>(fight) {
                fight.for_player(player, |fight| fight.modify_energy(energy));
            }
            Played::Done
        }
        // `Ignition`: a Plasma orb channelled for the ally it is pointed at.
        __c0 if __c0 == V::Card::IGNITION => {
            let target = fight.target();
            fight.for_player(target, |fight| fight.channel(V::Orb::PLASMA_ORB));
            Played::Done
        }
        // `HelixDrill`: its damage to the target, once for each hit it works
        // out, as `Barrage` does.
        __c0 if __c0 == V::Card::BARRAGE || __c0 == V::Card::HELIX_DRILL => barrage::<V, _>(fight),
        // `Voltaic`: a Lightning orb channelled for each its owner channelled
        // this fight before.
        __c0 if __c0 == V::Card::VOLTAIC => {
            for _ in 0..fight.var(V::CardVar::CALCULATED_CHANNELS) {
                fight.channel(V::Orb::LIGHTNING_ORB);
            }
            Played::Done
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
        // `Chaos`: a random orb off `CombatOrbGeneration` for each of its
        // `Repeat`.
        __c0 if __c0 == V::Card::CHAOS => {
            // The game's loop reads its count before each pass.
            let mut pass = 0;
            while pass < fight.var(V::CardVar::REPEAT) {
                pass += 1;
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
        // `ConsumingShadow`: its loop reads `Repeat` before each pass, so a
        // Dampen that comes off mid-play, as an evoke kills its caster, adds
        // the upgrade's channel.
        __c0 if __c0 == V::Card::CONSUMING_SHADOW => {
            let mut pass = 0;
            while pass < fight.var(V::CardVar::REPEAT) {
                pass += 1;
                fight.channel(V::Orb::DARK_ORB);
            }
            fight.apply_to_actor(
                V::Power::CONSUMING_SHADOW_POWER,
                PowerAmount::Counted(fight.var(V::CardVar::CONSUMING_SHADOW_POWER)),
            );
            Played::Done
        }
        __c0 if __c0 == V::Card::FTL => ftl::<V, _>(fight),
        // `Turbo`: its energy, then a Void into the discard pile.
        __c0 if __c0 == V::Card::TURBO => {
            fight.modify_energy(fight.var(V::CardVar::ENERGY));
            add_status::<V, _>(fight, V::Card::VOID);
            Played::Done
        }
        // `Capacitor`: its `Repeat` more orb slots.
        __c0 if __c0 == V::Card::CAPACITOR => {
            fight.add_orb_slots(fight.var(V::CardVar::REPEAT));
            Played::Done
        }
        // `Glacier`: its block, then two Frosts channelled.
        __c0 if __c0 == V::Card::GLACIER => {
            block::<V, _>(fight);
            fight.channel(V::Orb::FROST_ORB);
            fight.channel(V::Orb::FROST_ORB);
            Played::Done
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
        __c0 if __c0 == V::Card::COMPILE_DRIVER => compile_driver::<V, _>(fight),
        __c0 if __c0 == V::Card::DUALCAST => dualcast::<V, _>(fight),
        __c0 if __c0 == V::Card::TEMPEST => tempest::<V, _>(fight),
        __c0 if __c0 == V::Card::ICE_LANCE
            || __c0 == V::Card::FLAK_CANNON
            || __c0 == V::Card::UPROAR
            || __c0 == V::Card::SHATTER
            || __c0 == V::Card::MODDED
            || __c0 == V::Card::METEOR_STRIKE
            || __c0 == V::Card::HYPERBEAM
            || __c0 == V::Card::ALL_FOR_ONE
            || __c0 == V::Card::REFRACT
            || __c0 == V::Card::REBOOT
            || __c0 == V::Card::SUNDER
            || __c0 == V::Card::SCAVENGE
            || __c0 == V::Card::OVERCLOCK
            || __c0 == V::Card::ADAPTIVE_STRIKE
            || __c0 == V::Card::CLAW
            || __c0 == V::Card::GO_FOR_THE_EYES
            || __c0 == V::Card::GUNK_UP
            || __c0 == V::Card::HOLOGRAM
            || __c0 == V::Card::SCRAPE
            || __c0 == V::Card::MOMENTUM_STRIKE
            || __c0 == V::Card::TESLA_COIL =>
        {
            start::<V, K>(card, fight)
        }
        card => crate::behaviour::cards::generated::<V, K>(card, fight),
    }
}

/// A Defect card's play that can stop on a question: from its start to its
/// end, yielding each question it stops on. Each step is lent the fight, and
/// after a question the answer.
#[diapause::coroutine(yield = Ask<V>, resume = for<'f, 'a> Lent<'f, 'a, V, K>)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub fn paused<V: crate::Build, K: FightKind<V>>(card: V::Card) {
    match card {
        // `IceLance`: its damage to the target, then its `Repeat` of Frost
        // orbs channelled.
        __c0 if __c0 == V::Card::ICE_LANCE => {
            hit_target::<V, _>(resumed!().fight, 1);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            ice_lance_hit::<V, _>(resumed!().fight);
        }
        // `FlakCannon`: every status its owner holds outside the exhaust pile
        // exhausted, then its damage once for each, each hit at an enemy
        // drawn at random. The hits are counted before the statuses go. A
        // hook's question inside an exhaust stands the rest still.
        __c0 if __c0 == V::Card::FLAK_CANNON => {
            let hits: u8 = u8::try_from(resumed!().fight.var(V::CardVar::CALCULATED_HITS))
                .expect("a count of statuses fits a u8");
            let cards: crate::behaviour::cards::Listed<V> = statuses::<V, _>(resumed!().fight);
            let count: usize = cards.len();
            for at in 0..count {
                resumed!().fight.act_on(PileAct::Exhaust, &[cards[at]]);
                if resumed!().fight.waiting() {
                    yield_!(Ask::Waiting);
                }
            }
            flak_cannon_hits::<V, _>(resumed!().fight, hits);
        }
        __c0 if __c0 == V::Card::UPROAR => {
            uproar::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            uproar_hit::<V, _>(resumed!().fight);
        }
        __c0 if __c0 == V::Card::SHATTER => {
            shatter::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            shatter_hit::<V, _>(resumed!().fight);
        }
        // `Modded`: its `Repeat` orb slots, its `Cards` drawn, then one more
        // energy on its own cost for the fight. A shuffle inside the draw can
        // ask, and the cost waits for it.
        __c0 if __c0 == V::Card::MODDED => {
            let fight = &mut *resumed!().fight;
            fight.add_orb_slots(fight.var(V::CardVar::REPEAT));
            fight.draw(fight.var(V::CardVar::CARDS));
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            modded_drew::<V, _>(resumed!().fight);
        }
        // `MeteorStrike`: its damage to the target, then three Plasma orbs.
        __c0 if __c0 == V::Card::METEOR_STRIKE => {
            hit_target::<V, _>(resumed!().fight, 1);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            meteor_strike_hit::<V, _>(resumed!().fight);
        }
        // `Hyperbeam`: its damage to every enemy, then its `FocusPower` taken
        // off its owner.
        __c0 if __c0 == V::Card::HYPERBEAM => {
            hit_all::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            hyperbeam_hit::<V, _>(resumed!().fight);
        }
        // `AllForOne`: its damage to the target, then every Attack, Skill or
        // Power in the discard pile that costs nothing into the hand.
        __c0 if __c0 == V::Card::ALL_FOR_ONE => {
            hit_target::<V, _>(resumed!().fight, 1);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            all_for_one_hit::<V, _>(resumed!().fight);
        }
        // `Refract`: its damage to the target twice, then a Glass orb for
        // each of its `Repeat`.
        __c0 if __c0 == V::Card::REFRACT => {
            hit_target::<V, _>(resumed!().fight, 2);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            refract_hit::<V, _>(resumed!().fight);
        }
        // `Reboot`: the whole hand into the draw pile, shuffled, then its
        // `Cards` drawn. A question after the shuffle stands the draw still.
        __c0 if __c0 == V::Card::REBOOT => {
            let fight = &mut *resumed!().fight;
            let hand = fight.cards_in(Pile::Hand);
            fight.act_on(PileAct::IntoDraw, &hand);
            fight.shuffle();
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            let fight = &mut *resumed!().fight;
            fight.draw(fight.var(V::CardVar::CARDS));
        }
        // `Sunder`: its damage to the target, then its energy where a result
        // says `WasTargetKilled`, whatever brought the target back after.
        __c0 if __c0 == V::Card::SUNDER => {
            let mut killed: bool = sunder_attack::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
                killed = resumed!()
                    .fight
                    .waited_attack_results()
                    .iter()
                    .any(|result| result.killed);
            }
            sunder_hit::<V, _>(resumed!().fight, killed);
        }
        // `Scavenge`: one card of the hand exhausted, as `CardCmd.Exhaust`,
        // then its `Energy` as Energy Next Turn.
        __c0 if __c0 == V::Card::SCAVENGE => {
            yield_!(Ask::Asking {
                pile: Pile::Hand,
                selection: ONE,
            });
            let Lent { fight, taken } = resumed!();
            fight.act_on(
                PileAct::Exhaust,
                crate::behaviour::paused::selected::<V>(taken).as_slice(),
            );
            let energy = PowerAmount::Counted(fight.var(V::CardVar::ENERGY));
            fight.apply_to_actor(V::Power::ENERGY_NEXT_TURN_POWER, energy);
        }
        // `Overclock`: its `Cards` drawn, then a Burn into the discard pile.
        // A shuffle inside the draw can ask, and the Burn waits for it.
        __c0 if __c0 == V::Card::OVERCLOCK => {
            let fight = &mut *resumed!().fight;
            fight.draw(fight.var(V::CardVar::CARDS));
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            add_status::<V, _>(resumed!().fight, V::Card::BURN);
        }
        // `AdaptiveStrike`: its damage to the target, then a clone of itself
        // into the discard pile, free for the rest of the fight.
        __c0 if __c0 == V::Card::ADAPTIVE_STRIKE => {
            hit_target::<V, _>(resumed!().fight, 1);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            adaptive_strike_hit::<V, _>(resumed!().fight);
        }
        __c0 if __c0 == V::Card::CLAW => {
            claw::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            claw_hit::<V, _>(resumed!().fight);
        }
        __c0 if __c0 == V::Card::GO_FOR_THE_EYES => {
            go_for_the_eyes::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            go_for_the_eyes_hit::<V, _>(resumed!().fight);
        }
        __c0 if __c0 == V::Card::GUNK_UP => {
            gunk_up::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            gunk_up_hit::<V, _>(resumed!().fight);
        }
        // `Hologram`: its block, then one card of the discard pile into the
        // hand, as `CardPileCmd.Add(cardModel, PileType.Hand)`.
        __c0 if __c0 == V::Card::HOLOGRAM => {
            block::<V, _>(resumed!().fight);
            yield_!(Ask::Asking {
                pile: Pile::Discard,
                selection: ONE,
            });
            let Lent { fight, taken } = resumed!();
            fight.act_on(
                PileAct::IntoHand,
                crate::behaviour::paused::selected::<V>(taken).as_slice(),
            );
        }
        // `Scrape`: its damage to the target, then its `Cards` drawn, and
        // each drawn card whose own cost is anything but nothing discarded. A
        // shuffle inside the draw can ask, and the discard waits for it. A
        // draw the ended fight never ran drew nothing.
        __c0 if __c0 == V::Card::SCRAPE => {
            hit_target::<V, _>(resumed!().fight, 1);
            let mut drew: sts2_core::Drew<V> = {
                let fight = &mut *resumed!().fight;
                let cards = fight.var(V::CardVar::CARDS);
                fight.draw(cards)
            };
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
                drew = match resumed!().taken.take() {
                    Some(sts2_core::Taken::Drew(drawn)) => drawn,
                    _ => sts2_core::Drew::new(),
                };
            }
            scrape_drew::<V, _>(resumed!().fight, &drew);
        }
        __c0 if __c0 == V::Card::MOMENTUM_STRIKE => {
            momentum_strike::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            momentum_strike_hit::<V, _>(resumed!().fight);
        }
        __c0 if __c0 == V::Card::TESLA_COIL => {
            tesla_coil::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            tesla_coil_hit::<V, _>(resumed!().fight);
        }
        card => unreachable!("{card:?} never stops on a question"),
    }
}

crate::behaviour::paused::paused_play!(V::Card, crate::behaviour::cards::Resume::Defect);

/// Its damage to the target, `hits` times, as one attack.
fn hit_target<V: crate::Build, F: Fight<V>>(fight: &mut F, hits: crate::behaviour::cards::Number) {
    let target = fight.target();
    hit::<V, _>(fight, target, hits);
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
fn barrage<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let target = fight.target();
    let hits = fight.var(V::CardVar::CALCULATED_HITS);
    hit::<V, _>(fight, target, hits);
    Played::Done
}

/// `Claw`: its damage to the target, then its `Increase` onto the damage of
/// every Claw its owner holds in the fight, itself included.
fn claw<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    let target = fight.target();
    hit::<V, _>(fight, target, 1);
}

/// The rest of `Claw`, once its attack is over: every Claw's damage raised.
fn claw_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
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
}

/// `CompileDriver`: its damage to the target, then as many cards drawn as
/// its `CalculatedCards`.
fn compile_driver<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let target = fight.target();
    hit::<V, _>(fight, target, 1);
    let cards = fight.var(V::CardVar::CALCULATED_CARDS);
    fight.draw(cards);
    Played::Done
}

/// `GoForTheEyes`: its damage to the target, then its Weak on it where the
/// target `IntendsToAttack`: an attack or a death blow among its intents.
fn go_for_the_eyes<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    let target = fight.target();
    hit::<V, _>(fight, target, 1);
}

/// The rest of `GoForTheEyes`, once its attack is over: its Weak, where the
/// target means to attack.
fn go_for_the_eyes_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
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
}

/// `GunkUp`: its damage to the target, once for each of its `Repeat`, then a
/// Slimed into the discard pile.
fn gunk_up<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    let target = fight.target();
    let hits = fight.var(V::CardVar::REPEAT);
    hit::<V, _>(fight, target, hits);
}

/// The rest of `GunkUp`, once its attack is over: a Slimed into the discard
/// pile.
fn gunk_up_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    fight.add_generated_cards(sts2_core::CardIdent::new(V::Card::SLIMED), Pile::Discard, 1);
}

/// `MomentumStrike`: its damage to the target, then its own cost set to
/// nothing for the rest of the fight.
fn momentum_strike<V: crate::Build, F: Fight<V>>(fight: &mut F) {
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
}

/// The rest of `MomentumStrike`, once its attack is over: its own cost to
/// nothing for the fight.
fn momentum_strike_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    fight.modify_this(
        sts2_core::Change::SetCost(sts2_core::CostPool::Energy, 0),
        sts2_core::Lasting::ThisFight,
    );
}

/// `Scrape`, once its draw is over: each copy it drew whose own cost is
/// anything but nothing discarded, wherever it stands.
fn scrape_drew<V: crate::Build, F: Fight<V>>(fight: &mut F, drew: &[sts2_core::CardId<V>]) {
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
}

/// `Tempest`: a Lightning channelled for each energy of its X, and one more
/// once upgraded.
fn tempest<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let orbs = fight.x_value() + crate::behaviour::cards::Number::from(fight.upgraded());
    for _ in 0..orbs {
        fight.channel(V::Orb::LIGHTNING_ORB);
    }
    Played::Done
}

/// `Shatter`: its damage to every opponent. Then, once for each orb held as
/// it began, the first orb is evoked twice: once where it stands, then once
/// as it leaves.
fn shatter<V: crate::Build, F: Fight<V>>(fight: &mut F) {
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
}

/// The rest of `Shatter`, once its attack is over: each orb evoked twice.
fn shatter_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    for _ in 0..fight.orbs_held() {
        fight.evoke_next(false);
        fight.evoke_next(true);
    }
}

/// `Dualcast`. Evoke the first orb twice: once where it stands, then once
/// as it leaves. Nothing with no orb.
fn dualcast<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    if fight.orbs_held() > 0 {
        fight.evoke_next(false);
        fight.evoke_next(true);
    }
    Played::Done
}

/// `TeslaCoil`: its `Damage` at its target, then each Lightning's passive at
/// the same target, twice for an upgraded copy. The Lightnings are listed as
/// the damage leaves them.
fn tesla_coil<V: crate::Build, F: Fight<V>>(fight: &mut F) {
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
}

/// The rest of `TeslaCoil`, once its attack is over: each Lightning's passive
/// on the target.
fn tesla_coil_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
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
}

/// `Uproar`: its damage to the target twice, then an Attack from the draw
/// pile auto-played. The first of the playable Attacks stably shuffled off
/// `Shuffle`, or where none is playable, the first of all of them.
fn uproar<V: crate::Build, F: Fight<V>>(fight: &mut F) {
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
}

/// The rest of `Uproar`, once its attack is over: an Attack of the draw
/// pile auto-played.
fn uproar_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
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
}

/// `Modded`, once its draw is over: its own energy cost one higher for the
/// rest of the fight.
fn modded_drew<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    fight.modify_this(
        sts2_core::Change::ChangeCost(sts2_core::CostPool::Energy, 1),
        sts2_core::Lasting::ThisFight,
    );
}
