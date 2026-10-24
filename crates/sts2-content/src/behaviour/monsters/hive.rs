//! The second act's monsters.

use sts2_core::{
    Amount, CardAt, CardTable as _, Effect, Fight, Pile, PowerAmount, RunStreamKinds as _, Scaling,
};

use crate::behaviour::monsters::{Number, Resume};
use crate::generated::ascension::AscensionLevel;
use crate::generated::cards::CardRarity;
use crate::generated::runstate::CombatSide;
use crate::ids::{CardIds as _, EnchantmentIds as _, MonsterIds as _, MoveIds as _, PowerIds as _};

/// `FlutterMove`'s `PowerCmd.Apply<FlutterPower>(…, 5m, …)`.
const FLUTTER: Number = 5;

/// What a creature's health is counted in.
type Health = i32;

/// Powers a Hive monster applies to itself as it enters combat.
pub(super) fn starting_powers<V: crate::Build>(
    monster: V::Monster,
    ascension: AscensionLevel,
    side: CombatSide,
) -> Option<Vec<(V::Power, PowerAmount<Number>)>> {
    // `PlatingPower`'s own amount, which is also the block it opens with.
    const PLATING: Number = 15;
    const TOUGH_PLATING: Number = 18;

    // `SlumberPower`, which counts the hits the beetle sleeps through.
    const SLUMBER: Number = 3;

    // `ReattachPower`, which is both what keeps a segment in the fight and
    // the health its reattach gives back.
    const REATTACH: Number = 25;

    Some(match monster {
        __c0 if __c0 == V::Monster::THIEVING_HOPPER => {
            vec![(V::Power::ESCAPE_ARTIST_POWER, PowerAmount::Counted(5))]
        }
        __c0 if __c0 == V::Monster::DECIMILLIPEDE_SEGMENT_FRONT
            || __c0 == V::Monster::DECIMILLIPEDE_SEGMENT_MIDDLE
            || __c0 == V::Monster::DECIMILLIPEDE_SEGMENT_BACK =>
        {
            vec![(V::Power::REATTACH_POWER, PowerAmount::Counted(REATTACH))]
        }
        __c0 if __c0 == V::Monster::BOWLBUG_ROCK => {
            vec![(V::Power::IMBALANCED_POWER, PowerAmount::Counted(1))]
        }
        __c0 if __c0 == V::Monster::EXOSKELETON => {
            vec![(V::Power::HARD_TO_KILL_POWER, PowerAmount::Counted(9))]
        }
        // Each of these calls the base alone. The Ovicopter starts a sound
        // loop beside it.
        __c0 if __c0 == V::Monster::BOWLBUG_EGG
            || __c0 == V::Monster::SPINY_TOAD
            || __c0 == V::Monster::OVICOPTER =>
        {
            Vec::new()
        }
        __c0 if __c0 == V::Monster::CHOMPER => {
            vec![(V::Power::ARTIFACT_POWER, PowerAmount::Counted(2))]
        }
        __c0 if __c0 == V::Monster::ENTOMANCER => {
            vec![(V::Power::PERSONAL_HIVE_POWER, PowerAmount::Counted(1))]
        }
        // The Rocket's `AfterAddedToRoom` also surrounds its opponents, which
        // `starting_opponent_powers` names.
        __c0 if __c0 == V::Monster::CRUSHER => vec![
            (V::Power::BACK_ATTACK_LEFT_POWER, PowerAmount::Counted(1)),
            (V::Power::CRAB_RAGE_POWER, PowerAmount::Counted(1)),
        ],
        __c0 if __c0 == V::Monster::ROCKET => vec![
            (V::Power::BACK_ATTACK_RIGHT_POWER, PowerAmount::Counted(1)),
            (V::Power::CRAB_RAGE_POWER, PowerAmount::Counted(1)),
        ],
        __c0 if __c0 == V::Monster::INFESTED_PRISM => {
            let spark = match ascension.has(AscensionLevel::DeadlyEnemies) {
                true => 3,
                false => 2,
            };
            vec![(V::Power::VITAL_SPARK_POWER, PowerAmount::Counted(spark))]
        }
        __c0 if __c0 == V::Monster::PARAFRIGHT => {
            vec![(V::Power::ILLUSION_POWER, PowerAmount::Counted(1))]
        }
        // `ToughEgg.AfterAddedToRoom`, for an egg not yet hatched: Hatch at
        // two while the enemies take their turn, and at one otherwise.
        __c0 if __c0 == V::Monster::TOUGH_EGG => {
            let hatch = match side {
                CombatSide::Enemy => 2,
                _ => 1,
            };
            vec![(V::Power::HATCH_POWER, PowerAmount::Counted(hatch))]
        }
        __c0 if __c0 == V::Monster::LOUSE_PROGENITOR => {
            vec![(
                V::Power::CURL_UP_POWER,
                PowerAmount::Counted(curl_block(ascension)),
            )]
        }
        __c0 if __c0 == V::Monster::SLUMBERING_BEETLE => {
            let plating = match ascension.has(AscensionLevel::ToughEnemies) {
                true => TOUGH_PLATING,
                false => PLATING,
            };
            vec![
                (V::Power::PLATING_POWER, PowerAmount::Counted(plating)),
                (V::Power::SLUMBER_POWER, PowerAmount::Counted(SLUMBER)),
            ]
        }
        _ => return None,
    })
}

/// What a Hive monster's `AfterAddedToRoom` writes over its rolled health.
///
/// `DecimillipedeSegment` is the one that does. A segment carries an even
/// number of points, and no two in the row carry the same number. The player
/// can then tell them apart by the bar alone. It walks up in twos from the
/// rolled number, and wraps round its own range rather than leave it.
pub(super) fn health_after_added<V: crate::Build>(
    monster: V::Monster,
    ascension: AscensionLevel,
    rolled: Health,
    teammates: &[Health],
) -> Option<Health> {
    if !(monster == V::Monster::DECIMILLIPEDE_SEGMENT_FRONT
        || monster == V::Monster::DECIMILLIPEDE_SEGMENT_MIDDLE
        || monster == V::Monster::DECIMILLIPEDE_SEGMENT_BACK)
    {
        return None;
    }
    // `MaxInitialHp` and `MinInitialHp`, which are the same two numbers the
    // health was rolled between.
    let (min_hp, max_hp) = V::monster_hp(monster);
    let (lowest, highest): (Health, Health) = (
        min_hp.at(ascension as u8).into(),
        max_hp.at(ascension as u8).into(),
    );
    let mut health = rolled;
    if health % 2 == 1 {
        health += 1;
    }
    while teammates.contains(&health) {
        health += 2;
        if health > highest {
            health = lowest;
        }
    }
    Some(health)
}

/// Which move a Hive monster selects from a conditional state.
pub(super) fn conditional<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    monster: V::Monster,
    at: u8,
    spawn: sts2_core::MonsterSpawn<V::Encounter>,
    powers: &[sts2_core::Listener<'_, V, P>],
) -> Result<u8, sts2_core::ContentError> {
    let held = |wanted| {
        powers.iter().find_map(|listener| match listener {
            sts2_core::Listener::Power { power, state, .. } if *power == wanted => Some(*state),
            _ => None,
        })
    };
    let wanted = match monster {
        __c0 if __c0 == V::Monster::BOWLBUG_ROCK => match held(V::Power::IMBALANCED_POWER) {
            Some(state) if state.is_off_balance() => V::MoveId::DIZZY_MOVE,
            _ => V::MoveId::HEADBUTT_MOVE,
        },
        __c0 if __c0 == V::Monster::SLUMBERING_BEETLE => match held(V::Power::SLUMBER_POWER) {
            Some(_) => V::MoveId::SNORE_MOVE,
            None => V::MoveId::ROLL_OUT_MOVE,
        },
        // `CanLay`: three or fewer stand on its side, itself included.
        __c0 if __c0 == V::Monster::OVICOPTER => match spawn.standing <= LAYS_WHILE {
            true => V::MoveId::LAY_EGGS_MOVE,
            false => V::MoveId::NUTRITIONAL_PASTE_MOVE,
        },
        monster => unimplemented!("what {monster:?} asks the fight at state {at}"),
    };
    crate::behaviour::monsters::state_of::<V>(monster, wanted, at)
}

/// Perform one of the Hive monsters' moves, where the generator left it to a
/// hand port.
pub(super) fn perform<V: crate::Build, F: Fight<V>>(
    monster: V::Monster,
    move_id: V::MoveId,
    step: u8,
    fight: &mut F,
) -> Option<u8> {
    match (monster, move_id) {
        (__c0, __c1)
            if (__c0 == V::Monster::THIEVING_HOPPER && __c1 == V::MoveId::THIEVERY_MOVE) =>
        {
            thievery::<V, _>(fight)
        }
        // `WebMove`: its hit, then Frail at two on each target. `Curled`
        // picks its animations alone.
        (__c0, __c1)
            if (__c0 == V::Monster::LOUSE_PROGENITOR && __c1 == V::MoveId::WEB_CANNON_MOVE) =>
        {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            fight.apply_to_opponents(V::Power::FRAIL_POWER, PowerAmount::Counted(2));
        }
        (__c0, __c1)
            if (__c0 == V::Monster::LOUSE_PROGENITOR && __c1 == V::MoveId::POUNCE_MOVE) =>
        {
            fight.attack_opponents()
        }
        // `ToxicMove`: `CardPileCmd.AddToCombatAndPreview<Toxic>(targets,
        // Hand, 2)`, into the hand of the one player.
        // `LayEggsMove`: three times, `CreatureCmd.Add<ToughEgg>` into the
        // last slot no enemy holds, then Minion on it.
        (__c0, __c1) if (__c0 == V::Monster::OVICOPTER && __c1 == V::MoveId::LAY_EGGS_MOVE) => {
            for _ in 0..EGGS {
                if let Some(egg) = fight.summon_into_last_free_slot(V::Monster::TOUGH_EGG) {
                    fight.apply_power(egg, V::Power::MINION_POWER, PowerAmount::Counted(1));
                }
            }
        }
        // `Hatch` draws `NextInt(HatchlingMinHp, HatchlingMaxHp)`.
        (__c0, __c1) if (__c0 == V::Monster::TOUGH_EGG && __c1 == V::MoveId::HATCH_MOVE) => {
            hatch::<V, _>(fight, |fight, min, max| fight.niche_int(min, max));
        }
        // `IllusionMove`: `CreatureCmd.Add<Parafright>` into the `illusion`
        // slot while the fight is live.
        (__c0, __c1) if (__c0 == V::Monster::THE_OBSCURA && __c1 == V::MoveId::ILLUSION_MOVE) => {
            fight.summon_into_slot(V::Monster::PARAFRIGHT, ILLUSION_SLOT);
        }
        // `WailMove`: Strength at three on every creature on its side.
        (__c0, __c1) if (__c0 == V::Monster::THE_OBSCURA && __c1 == V::MoveId::SAIL_MOVE) => {
            use sts2_core::CreatureView as _;

            let side = fight.side();
            let team: Vec<_> = fight
                .creatures()
                .filter(|creature| creature.side() == side && creature.alive())
                .map(|creature| creature.id())
                .collect();
            for creature in team {
                fight.apply_power(creature, V::Power::STRENGTH_POWER, PowerAmount::Counted(3));
            }
        }
        // `SpitMove`: below three Personal Hive, one more of it and one
        // Strength. At three, two Strength.
        (__c0, __c1)
            if (__c0 == V::Monster::ENTOMANCER && __c1 == V::MoveId::PHEROMONE_SPIT_MOVE) =>
        {
            use sts2_core::CreatureView as _;

            let actor = fight.actor();
            let hive = fight
                .creatures()
                .find(|creature| creature.id() == actor)
                .and_then(|creature| creature.power(V::Power::PERSONAL_HIVE_POWER));
            match hive {
                Some(PowerAmount::Counted(amount)) if amount < HIVE_CAP => {
                    fight.apply_to_actor(V::Power::PERSONAL_HIVE_POWER, PowerAmount::Counted(1));
                    fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(1));
                }
                Some(PowerAmount::Counted(_)) => {
                    fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(2))
                }
                Some(PowerAmount::Endless) => unimplemented!("an endless Personal Hive"),
                None => unimplemented!("an Entomancer spitting with no Personal Hive"),
            }
        }
        // The Kaiser Crab's arms. The rest of each move is its background's
        // animation.
        (__c0, __c1)
            if (__c0 == V::Monster::CRUSHER
                && (__c1 == V::MoveId::THRASH_MOVE
                    || __c1 == V::MoveId::ENLARGING_STRIKE_MOVE)) =>
        {
            fight.attack_opponents()
        }
        // `BugStingMove`: its two hits, then Weak and Frail at two on each
        // target.
        (__c0, __c1) if (__c0 == V::Monster::CRUSHER && __c1 == V::MoveId::BUG_STING_MOVE) => {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            fight.apply_to_opponents(V::Power::WEAK_POWER, PowerAmount::Counted(2));
            fight.apply_to_opponents(V::Power::FRAIL_POWER, PowerAmount::Counted(2));
        }
        (__c0, __c1) if (__c0 == V::Monster::CRUSHER && __c1 == V::MoveId::ADAPT_MOVE) => {
            let strength = match fight.ascension().has(AscensionLevel::DeadlyEnemies) {
                true => 3,
                false => 2,
            };
            fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(strength));
        }
        // `GuardedStrikeMove`: its hit, then eighteen block as a move.
        (__c0, __c1) if (__c0 == V::Monster::CRUSHER && __c1 == V::MoveId::GUARDED_STRIKE_MOVE) => {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(GUARDED_BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
        }
        (__c0, __c1)
            if (__c0 == V::Monster::ROCKET
                && (__c1 == V::MoveId::TARGETING_RETICLE_MOVE
                    || __c1 == V::MoveId::PRECISION_BEAM_MOVE
                    || __c1 == V::MoveId::LASER_MOVE)) =>
        {
            fight.attack_opponents()
        }
        (__c0, __c1) if (__c0 == V::Monster::ROCKET && __c1 == V::MoveId::CHARGE_UP_MOVE) => {
            let strength = match fight.ascension().has(AscensionLevel::DeadlyEnemies) {
                true => 3,
                false => 2,
            };
            fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(strength));
        }
        (__c0, __c1) if (__c0 == V::Monster::ROCKET && __c1 == V::MoveId::RECHARGE_MOVE) => {}
        // `LiquifyMove`: Sandpit at four on itself, aimed at the player, then
        // six Frantic Escapes at random places: three into the draw pile and
        // three into the discard pile. `HasLiquified` picks its animations
        // alone. A fight here has one player.
        (__c0, __c1)
            if (__c0 == V::Monster::THE_INSATIABLE && __c1 == V::MoveId::LIQUIFY_GROUND_MOVE) =>
        {
            fight.apply_to_actor(V::Power::SANDPIT_POWER, PowerAmount::Counted(SANDPIT));
            for escape in 0..ESCAPES {
                let pile = match escape < ESCAPES / 2 {
                    true => Pile::Draw,
                    false => Pile::Discard,
                };
                fight.add_at_random(V::Card::FRANTIC_ESCAPE, pile);
            }
        }
        (__c0, __c1) if (__c0 == V::Monster::MYTE && __c1 == V::MoveId::TOXIC_MOVE) => {
            fight.add_generated_cards(sts2_core::CardIdent::new(V::Card::TOXIC), Pile::Hand, 2);
        }
        // `SpikesMove`: Thorns at five on itself. `IsSpiny` picks its
        // animations alone.
        (__c0, __c1)
            if (__c0 == V::Monster::SPINY_TOAD && __c1 == V::MoveId::PROTRUDING_SPIKES_MOVE) =>
        {
            fight.apply_to_actor(V::Power::THORNS_POWER, PowerAmount::Counted(SPINES))
        }
        // `ExplosionMove`: its hit, then the same Thorns taken back.
        (__c0, __c1)
            if (__c0 == V::Monster::SPINY_TOAD && __c1 == V::MoveId::SPIKE_EXPLOSION_MOVE) =>
        {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return Some(1);
            }
            fight.apply_to_actor(V::Power::THORNS_POWER, PowerAmount::Counted(-SPINES));
        }
        // `CurlAndGrowMove`: `CreatureCmd.GainBlock` of its `CurlBlock` as a
        // move, then Strength of its `GrowStrength` on itself, which a build
        // may raise from Deadly Enemies.
        (__c0, __c1)
            if (__c0 == V::Monster::LOUSE_PROGENITOR && __c1 == V::MoveId::CURL_AND_GROW_MOVE) =>
        {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(curl_block(fight.ascension())),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            let grow = match fight.ascension().has(AscensionLevel::DeadlyEnemies) {
                true => V::LOUSE_PROGENITOR_DEADLY_GROW,
                false => 5,
            };
            fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(grow));
        }
        // `FlutterMove`: `IsHovering`, which picks its sounds, then
        // `PowerCmd.Apply<FlutterPower>` of five on itself.
        (__c0, __c1)
            if (__c0 == V::Monster::THIEVING_HOPPER && __c1 == V::MoveId::FLUTTER_MOVE) =>
        {
            fight.apply_to_actor(V::Power::FLUTTER_POWER, PowerAmount::Counted(FLUTTER))
        }
        // `EscapeMove`: `CreatureCmd.Escape` on itself.
        (__c0, __c1) if (__c0 == V::Monster::THIEVING_HOPPER && __c1 == V::MoveId::ESCAPE_MOVE) => {
            fight.escape()
        }
        (__c0, __c1) if (__c0 == V::Monster::BOWLBUG_ROCK && __c1 == V::MoveId::HEADBUTT_MOVE) => {
            fight.attack_opponents()
        }
        // `DizzyMove`: `IsOffBalance` cleared, which Imbalanced keeps here.
        (__c0, __c1) if (__c0 == V::Monster::BOWLBUG_ROCK && __c1 == V::MoveId::DIZZY_MOVE) => {
            let actor = fight.actor();
            fight.write_power(actor, V::Power::IMBALANCED_POWER, |state| {
                state.set_off_balance(false)
            });
        }
        (__c0, __c1)
            if (__c0 == V::Monster::SLUMBERING_BEETLE && __c1 == V::MoveId::ROLL_OUT_MOVE) =>
        {
            return roll_out::<V, _>(fight, step);
        }
        // `WritheMove`, whose two hits are the intent's own repeat count.
        (__c0, __c1)
            if ((__c0 == V::Monster::DECIMILLIPEDE_SEGMENT_FRONT
                || __c0 == V::Monster::DECIMILLIPEDE_SEGMENT_MIDDLE
                || __c0 == V::Monster::DECIMILLIPEDE_SEGMENT_BACK)
                && __c1 == V::MoveId::WRITHE_MOVE) =>
        {
            fight.attack_opponents()
        }
        (__c0, __c1)
            if ((__c0 == V::Monster::DECIMILLIPEDE_SEGMENT_FRONT
                || __c0 == V::Monster::DECIMILLIPEDE_SEGMENT_MIDDLE
                || __c0 == V::Monster::DECIMILLIPEDE_SEGMENT_BACK)
                && __c1 == V::MoveId::BULK_MOVE) =>
        {
            return bulk::<V, _>(fight, step);
        }
        (__c0, __c1)
            if ((__c0 == V::Monster::DECIMILLIPEDE_SEGMENT_FRONT
                || __c0 == V::Monster::DECIMILLIPEDE_SEGMENT_MIDDLE
                || __c0 == V::Monster::DECIMILLIPEDE_SEGMENT_BACK)
                && __c1 == V::MoveId::CONSTRICT_MOVE) =>
        {
            return constrict::<V, _>(fight, step);
        }
        (__c0, __c1)
            if ((__c0 == V::Monster::DECIMILLIPEDE_SEGMENT_FRONT
                || __c0 == V::Monster::DECIMILLIPEDE_SEGMENT_MIDDLE
                || __c0 == V::Monster::DECIMILLIPEDE_SEGMENT_BACK)
                && __c1 == V::MoveId::REATTACH_MOVE) =>
        {
            reattach::<V, _>(fight)
        }
        (monster, move_id) => return V::generated_move(monster, move_id, step, fight),
    }
    None
}

/// Roll over the target, then harden.
///
/// `RolloutMove` applies the Strength to the beetle itself, so the fight it
/// hits and the creature it grows are not the same one.
fn roll_out<V: crate::Build, F: Fight<V>>(fight: &mut F, step: u8) -> Option<u8> {
    const HARDENS: Number = 2;

    if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
        return Some(1);
    }
    fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(HARDENS));
    None
}

/// Swell: hit, then grow.
///
/// `BulkMove` applies the Strength to the segment itself, so the creature it
/// hits and the one it grows are not the same.
fn bulk<V: crate::Build, F: Fight<V>>(fight: &mut F, step: u8) -> Option<u8> {
    const BULKS: Number = 2;

    if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
        return Some(1);
    }
    fight.apply_to_actor(V::Power::STRENGTH_POWER, PowerAmount::Counted(BULKS));
    None
}

/// Squeeze: hit, and leave the target weakened.
fn constrict<V: crate::Build, F: Fight<V>>(fight: &mut F, step: u8) -> Option<u8> {
    if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
        return Some(1);
    }
    fight.apply_to_opponents(V::Power::WEAK_POWER, PowerAmount::Counted(1));
    None
}

/// Join the row again, for the health the power carries.
///
/// `ReattachPower.DoReattach`, which is `CreatureCmd.Heal(Owner, Amount)` and
/// so is what brings the segment back. A row whose every other segment is down
/// does not reattach. The fight is over by then, and what plays instead is the
/// death of all of them at once.
fn reattach<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    use sts2_core::CreatureView as _;

    // `GetOtherSegments`, which is the rest of the row carrying this power.
    let actor = fight.actor();
    let side = fight.side();
    let alone = fight
        .creatures()
        .filter(|creature| {
            creature.id() != actor
                && creature.side() == side
                && creature.power(V::Power::REATTACH_POWER).is_some()
        })
        .all(|creature| !creature.alive());
    if alone {
        return;
    }
    let carried = fight
        .creatures()
        .find(|creature| creature.id() == actor)
        .and_then(|creature| creature.power(V::Power::REATTACH_POWER));
    let amount = match carried {
        Some(PowerAmount::Counted(amount)) => amount,
        Some(PowerAmount::Endless) => unimplemented!("an endless Reattach power"),
        None => unreachable!("a segment reattaching carries the power that reattaches it"),
    };
    fight.write_power(actor, V::Power::REATTACH_POWER, |state| {
        state.set_reviving(false)
    });
    fight.heal(actor, amount);
}

/// `LiquifyMove`'s Sandpit, `PowerCmd.Apply(…, 4m, …)`.
const SANDPIT: Number = 4;

/// `LiquifyMove`'s six Frantic Escapes.
const ESCAPES: usize = 6;

/// `GuardedStrikeMove`'s `CreatureCmd.GainBlock(…, 18m, ValueProp.Move, …)`.
const GUARDED_BLOCK: Number = 18;

/// The Personal Hive `SpitMove` stops growing at.
const HIVE_CAP: Number = 3;

/// `illusion`, the first of `TheObscuraNormal.Slots`.
const ILLUSION_SLOT: u8 = 0;

/// `LayEggsMove`'s loop, which lays three.
const EGGS: usize = 3;

/// `CanLay`'s bound on how many stand on the Ovicopter's side.
const LAYS_WHILE: u8 = 3;

/// The Thorns `SpikesMove` applies and `ExplosionMove` takes back.
const SPINES: Number = 5;

/// `LouseProgenitor.CurlBlock`, either side of `ToughEnemies`.
fn curl_block(ascension: crate::generated::ascension::AscensionLevel) -> Number {
    match ascension.has(AscensionLevel::ToughEnemies) {
        true => 18,
        false => 14,
    }
}

/// Take the highest-priority deck card still in combat, then attack.
fn thievery<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    let cards: Vec<CardAt<V>> = [Pile::Draw, Pile::Discard]
        .into_iter()
        .flat_map(|pile| fight.cards_in(pile))
        .filter(|card| fight.deck_version(card.id()).is_some())
        .collect();
    let priorities = [
        &[CardRarity::Uncommon][..],
        &[CardRarity::Common, CardRarity::Rare, CardRarity::Event][..],
        &[CardRarity::Basic, CardRarity::Quest][..],
        &[CardRarity::Ancient][..],
    ];
    let candidates = priorities
        .into_iter()
        .find_map(|rarities| {
            let matching: Vec<_> = cards
                .iter()
                .copied()
                .filter(|&card| {
                    let ident = fight.card_ident(card);
                    let imbued = ident.enchantment.map(|enchanted| enchanted.enchantment)
                        == Some(V::Enchantment::IMBUED);
                    let priority_accepts_imbued = rarities == [CardRarity::Ancient];
                    (imbued == priority_accepts_imbued)
                        && (rarities.contains(&ident.card.rarity()) || imbued)
                })
                .collect();
            (!matching.is_empty()).then_some(matching)
        })
        .unwrap_or(cards);
    if let Some(card) = fight.random_card(
        &candidates,
        <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION,
    ) {
        // `SwipePower.Steal`, then `PowerCmd.Apply`. A Swipe the fight refuses
        // takes the card with nobody to give it back.
        let stolen = fight
            .deck_version(card.id())
            .unwrap_or_else(|| unreachable!("the candidates are deck copies"));
        fight.remove_card(card);
        fight.apply_to_actor(V::Power::SWIPE_POWER, PowerAmount::Counted(1));
        let actor = fight.actor();
        fight.write_power(actor, V::Power::SWIPE_POWER, |state| state.steal(stolen));
    }
    fight.attack_opponents();
}

/// `CurseOfKnowledgeBranch`: a curse while any set is left, a slap once the
/// three are cast.
pub(super) fn knowledge_demon_branch<V: crate::Build>(
    model: &crate::behaviour::monsters::Model<V>,
    at: u8,
) -> Result<u8, sts2_core::ContentError> {
    let crate::behaviour::monsters::Model::KnowledgeDemon { curses } = *model else {
        unimplemented!("a Knowledge Demon holding {model:?}")
    };
    let wanted = match usize::from(curses) < CURSE_SETS {
        true => V::MoveId::CURSE_OF_KNOWLEDGE_MOVE,
        false => V::MoveId::SLAP_MOVE,
    };
    crate::behaviour::monsters::state_of::<V>(V::Monster::KNOWLEDGE_DEMON, wanted, at)
}

/// `_curseOfKnowledgeSets.Count`.
const CURSE_SETS: usize = 3;

/// The Knowledge Demon's moves.
///
/// `CurseOfKnowledge` offers the player `Disintegration` and one more card of
/// the set its counter stands at. The card chosen applies its power, which
/// `resume` carries out.
pub(super) fn knowledge_demon<V: crate::Build, F: Fight<V>>(
    move_id: V::MoveId,
    step: u8,
    fight: &mut F,
) -> sts2_core::Played<V> {
    use sts2_core::CardIdent;

    // `ThrowingPlayerChoiceContext`'s `PowerCmd.Apply<StrengthPower>`.
    const PONDER_STRENGTH: Number = 2;
    const DEADLY_PONDER_STRENGTH: Number = 3;
    // `30 * Players.Count`, and a fight here has one player.
    const PONDER_HEAL: Number = 30;

    let deadly = fight.ascension().has(AscensionLevel::DeadlyEnemies);
    match move_id {
        __c0 if __c0 == V::MoveId::CURSE_OF_KNOWLEDGE_MOVE => {
            // `CurseOfKnowledgeCounter++`, after the choice in the game.
            // Nothing reads it before the enemy turn is over.
            let set = fight.model(|model| {
                let crate::behaviour::monsters::Model::KnowledgeDemon { curses } = model else {
                    unimplemented!("a Knowledge Demon holding {model:?}")
                };
                let set = *curses;
                *curses = set + 1;
                set
            });
            let other = match set {
                0 => V::Card::MIND_ROT,
                1 => V::Card::SLOTH,
                2 => V::Card::WASTE_AWAY,
                _ => unimplemented!("a Knowledge Demon with no set left at {set}"),
            };
            sts2_core::Played::Choosing {
                offered: [
                    CardIdent::new(V::Card::DISINTEGRATION),
                    CardIdent::new(other),
                ]
                .into_iter()
                .collect(),
                // `FromChooseACardScreen` without `canSkip`.
                skippable: false,
                at: Resume::CurseOfKnowledge { set }.into(),
            }
        }
        // `IsBurnt` picks the demon's animations alone.
        __c0 if __c0 == V::MoveId::KNOWLEDGE_OVERWHELMING_MOVE || __c0 == V::MoveId::SLAP_MOVE => {
            fight.attack_opponents();
            sts2_core::Played::Done
        }
        __c0 if __c0 == V::MoveId::PONDER_MOVE => {
            if crate::behaviour::monsters::attack_first::<V, _>(fight, step) {
                return sts2_core::Played::Waiting {
                    at: Resume::Step(1).into(),
                };
            }
            let actor = fight.actor();
            fight.heal(actor, PONDER_HEAL);
            let strength = match deadly {
                true => DEADLY_PONDER_STRENGTH,
                false => PONDER_STRENGTH,
            };
            fight.apply_power(
                actor,
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(strength),
            );
            sts2_core::Played::Done
        }
        move_id => unimplemented!("what {move_id:?} does when a Knowledge Demon performs it"),
    }
}

/// `IChoosable.OnChosen` for the card the player took, which applies its
/// power to them. A skipped choice does nothing.
pub(super) fn resume<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: Option<sts2_core::CardIdent<V>>,
) -> sts2_core::Played<V> {
    // `_disintegrationDamageValues`.
    const DISINTEGRATION: [Number; CURSE_SETS] = [6, 7, 8];

    let Resume::CurseOfKnowledge { set } = at else {
        unreachable!("{at:?} resumes a move, not a choice")
    };
    let Some(chosen) = chosen else {
        return sts2_core::Played::Done;
    };
    let (power, amount) = match chosen.card {
        __c0 if __c0 == V::Card::DISINTEGRATION => (
            V::Power::DISINTEGRATION_POWER,
            DISINTEGRATION[usize::from(set)],
        ),
        __c0 if __c0 == V::Card::MIND_ROT => (V::Power::MIND_ROT_POWER, 1),
        __c0 if __c0 == V::Card::SLOTH => (V::Power::SLOTH_POWER, 3),
        __c0 if __c0 == V::Card::WASTE_AWAY => (V::Power::WASTE_AWAY_POWER, 1),
        card => unimplemented!("{card:?} chosen from a curse of knowledge"),
    };
    let actor = fight.actor();
    fight.apply_power(actor, power, PowerAmount::Counted(amount));
    sts2_core::Played::Done
}

/// `HatchMove`: Hatch goes, then every other power but Minion, then `Hatch`:
/// its hit points set to a draw off the niche stream. `draw` takes the bounds
/// `HatchlingMinHp` and `HatchlingMaxHp`, which each build draws between its
/// own way.
pub(crate) fn hatch<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    draw: impl FnOnce(&mut F, i32, i32) -> i32,
) {
    use sts2_core::{CreatureView as _, Table as _};

    let egg = fight.actor();
    fight.remove_power(egg, V::Power::HATCH_POWER);
    let others: Vec<V::Power> = fight
        .creatures()
        .find(|creature| creature.id() == egg)
        .map(|creature| {
            V::Power::ALL
                .iter()
                .copied()
                .filter(|&power| power != V::Power::MINION_POWER)
                .filter(|&power| creature.power(power).is_some())
                .collect()
        })
        .unwrap_or_default();
    for power in others {
        fight.remove_power(egg, power);
    }
    let (min, max) = match fight.ascension().has(AscensionLevel::ToughEnemies) {
        true => (20, 23),
        false => (19, 22),
    };
    let hp = draw(fight, min, max);
    fight.set_max_and_current_hp(egg, num_traits::cast(hp).expect("a hatchling's health"));
}

/// The build's numbers for the rules above.
macro_rules! lists {
    ($build:ty, v0_107_1) => {
        /// Louse Progenitor's Grow from Deadly Enemies on, which this build
        /// leaves at its base.
        const LOUSE_PROGENITOR_DEADLY_GROW: crate::behaviour::monsters::Number = 5;
    };
    ($build:ty, v0_111_0) => {
        /// Louse Progenitor's `GrowStrength` from Deadly Enemies on.
        const LOUSE_PROGENITOR_DEADLY_GROW: crate::behaviour::monsters::Number = 7;
    };
}
pub(crate) use lists;
