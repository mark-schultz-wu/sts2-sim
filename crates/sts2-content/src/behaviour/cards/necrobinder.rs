//! The Necrobinder's cards, where the generator leaves them.

use crate::behaviour::Played;
use crate::behaviour::cards::Number;
use crate::behaviour::paused::{Ask, Lent};
use sts2_core::{
    Amount, Change, CostPool, Effect, EventHook, Fight, FightKind, HookEffect, Lasting, Pile,
    PileAct, Scaling, Selection,
};

/// Cards a play took from the hand, which it goes through after a wait.
type Hand<V> = smallvec::SmallVec<[sts2_core::CardAt<V>; sts2_core::MAX_CARDS_IN_HAND]>;

/// One card, as a pick asks for it.
const ONE: Selection = Selection::Exactly(nonmax::NonMaxUsize::ONE);

use crate::generated::cards::CardKeyword;
use crate::ids::{CardIds as _, CardVarIds as _, PowerIds as _};

/// Its damage to the target, as one attack.
pub(crate) fn strike<V: crate::Build, F: Fight<V>>(fight: &mut F) {
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

/// `Fetch`: where Osty stands, its damage to the target, then its `Cards`
/// drawn unless this copy finished a play already this turn.
fn fetch<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    if fight.osty().is_none() {
        return Played::Done;
    }
    osty_hit::<V, _>(fight, V::CardVar::OSTY_DAMAGE);
    let this = fight.playing();
    let played =
        fight.history().this_turn.iter().any(
            |entry| matches!(entry, sts2_core::HistoryEntry::Finished(done) if done.id == this),
        );
    if !played {
        fight.draw(fight.var(V::CardVar::CARDS));
    }
    Played::Done
}

/// `DrainPower`: its damage to the target, then its `Cards` of the discard
/// pile's upgradable cards taken at random off `CombatCardSelection`, each
/// upgraded.
fn drain_power<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    use sts2_core::CardTable as _;

    strike::<V, _>(fight);
    let mut upgradable: Vec<_> = fight
        .cards_in(Pile::Discard)
        .into_iter()
        .filter(|&card| {
            let ident = fight.card_ident(card);
            ident.upgrade < ident.card.max_upgrade()
        })
        .collect();
    let count = usize::try_from(fight.var(V::CardVar::CARDS)).expect("a count is not negative");
    for card in fight.take_random_cards(&mut upgradable, count) {
        fight.modify_card(card, Change::Upgrade, Lasting::ThisFight);
    }
    Played::Done
}

/// `Reave`: its damage to the target, then its `Cards` of Souls into the
/// draw pile at random, each upgraded where Reave is.
fn reave<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    strike::<V, _>(fight);
    let upgraded = fight.upgraded();
    for _ in 0..fight.var(V::CardVar::CARDS) {
        let Some(soul) = fight.add_at_random(V::Card::SOUL, Pile::Draw) else {
            break;
        };
        if upgraded {
            fight.modify_card(soul, Change::Upgrade, Lasting::ThisFight);
        }
    }
    Played::Done
}

/// `Putrefy`: its `Power` as Weak, then as Vulnerable, on the target.
fn putrefy<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let target = fight.target();
    let amount = sts2_core::PowerAmount::Counted(fight.var(V::CardVar::POWER));
    fight.apply_power(target, V::Power::WEAK_POWER, amount);
    fight.apply_power(target, V::Power::VULNERABLE_POWER, amount);
    Played::Done
}

/// `Spur`: its `Summon` of Osty, then its `Heal` on Osty.
fn spur<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    fight.summon_osty(fight.var(V::CardVar::SUMMON));
    if let Some(osty) = fight.osty() {
        fight.heal(
            osty,
            fight.var(<V::CardVar as crate::ids::CardVarIds>::HEAL),
        );
    }
    Played::Done
}

/// `Sacrifice`: where Osty stands, its calculated block worked out, then
/// Osty killed, then the block gained.
fn sacrifice<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let Some(osty) = fight.osty() else {
        return Played::Done;
    };
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(V::CardVar::CALCULATED_BLOCK),
        scaling: Scaling::Dynamic,
    });
    fight.kill(osty);
    fight.apply(block);
    Played::Done
}

/// `Eradicate`: its damage to the target, once for each energy its X spent,
/// as one attack.
fn eradicate<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let target = fight.target();
    let hits = fight.x_value();
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
    Played::Done
}

/// `GraveWarden`: its block, then its `Cards` of Souls into the draw pile at
/// random.
fn grave_warden<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(V::CardVar::BLOCK),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    for _ in 0..fight.var(V::CardVar::CARDS) {
        if fight.add_at_random(V::Card::SOUL, Pile::Draw).is_none() {
            break;
        }
    }
    Played::Done
}

/// `NegativePulse`: its block, then its Doom on each enemy that can be hit.
fn negative_pulse<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(V::CardVar::BLOCK),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    let doom = sts2_core::PowerAmount::Counted(fight.var(V::CardVar::DOOM_POWER));
    for target in fight.hittable_opponents() {
        fight.apply_power(target, V::Power::DOOM_POWER, doom);
    }
    Played::Done
}

/// `Severance`: its damage to the target, then a Soul into the draw pile at
/// random, one into the discard pile and one into hand.
fn severance<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    strike::<V, _>(fight);
    let soul = V::Card::SOUL;
    fight.add_at_random(soul, Pile::Draw);
    fight.add_generated_cards(sts2_core::CardIdent::new(soul), Pile::Discard, 1);
    fight.add_generated_cards(sts2_core::CardIdent::new(soul), Pile::Hand, 1);
    Played::Done
}

/// `BoneShards`: where Osty stands, its damage from Osty to every enemy, its
/// block, then Osty killed where it still lives.
fn bone_shards<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let Some(osty) = fight.osty() else {
        return Played::Done;
    };
    fight.attack_from(osty, Scaling::Dynamic, |fight| {
        let targets = fight.opponents();
        // One `CreatureCmd.Damage` over them all.
        fight.damage_command(|fight| {
            for target in targets {
                let hit = fight.resolve(Effect::Attack {
                    dealer: osty,
                    target,
                    damage: Amount::Var(V::CardVar::OSTY_DAMAGE),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(hit);
            }
        });
    });
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(V::CardVar::BLOCK),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    if let Some(osty) = fight.osty() {
        fight.kill(osty);
    }
    Played::Done
}

/// `Misery`: the target's debuffs as they stand, then its damage to the
/// target. Each other hittable enemy then takes each of those debuffs: onto
/// the instance it stacks with through `PowerCmd.ModifyAmount`, else as a
/// new one through `PowerCmd.Apply`. Both come from the debuff's applier.
fn misery<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    use crate::generated::powers::PowerInstanceType;
    use sts2_core::{PowerAmount, Table as _};

    let target = fight.target();
    let debuffs: Vec<_> = fight
        .powers_on(target)
        .into_iter()
        .filter(|held| crate::behaviour::cards::debuff_for_amount::<V>(held.power, held.amount))
        .collect();
    strike::<V, _>(fight);
    for enemy in fight.hittable_opponents() {
        if enemy == target {
            continue;
        }
        for held in &debuffs {
            let power = held.power;
            // `DoHackyThingsForSpecificPowers`: an `ITemporaryPower` ignores
            // its next instance, which is the lent stat the copy would move
            // again.
            let temporary = crate::behaviour::powers::lends::<V>(power).is_some();
            // `ClonePreservingMutability` builds its internal data anew, which
            // is all a noted play is. Imbalanced's off-balance is the Bowlbug
            // Rock's `IsOffBalance`, or a stun, which the port keeps on the
            // power and the game's model holds nothing of.
            if !held.fresh && !crate::behaviour::powers::clone_starts_anew::<V>(power) {
                unimplemented!("Misery copying {power:?}, whose model carries state");
            }
            let PowerAmount::Counted(amount) = held.amount else {
                unimplemented!("Misery copying an endless {power:?}");
            };
            match power.data().instance {
                Some(_) if temporary => {
                    unimplemented!("Misery copying {power:?}, an instanced temporary power")
                }
                None if fight.powers_on(enemy).iter().any(|on| on.power == power) => {
                    if temporary {
                        fight.write_power(enemy, power, |state| state.ignore_next_instance());
                    }
                    fight.modify_power_amount(enemy, power, amount, held.applier);
                }
                Some(PowerInstanceType::InstancedPerApplier)
                    if fight
                        .powers_on(enemy)
                        .iter()
                        .any(|on| on.power == power && on.applier == held.applier) =>
                {
                    fight.modify_power_amount(enemy, power, amount, held.applier);
                }
                None if temporary => {
                    let mut copy = <crate::behaviour::powers::State<V> as sts2_core::PowerModel<
                        V,
                    >>::new(power);
                    copy.ignore_next_instance();
                    fight.apply_power_copy(enemy, copy, held.amount, held.applier);
                }
                None
                | Some(PowerInstanceType::Instanced | PowerInstanceType::InstancedPerApplier) => {
                    fight.apply_power_from(enemy, power, held.amount, held.applier);
                }
            }
        }
    }
    Played::Done
}

/// `Dirge`: its `Summon` of Osty once for each energy its X spent. Then as
/// many Souls go into the draw pile at random, upgraded where Dirge is.
fn dirge<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V, F::Kind> {
    let x = fight.x_value();
    for _ in 0..x {
        fight.summon_osty(fight.var(V::CardVar::SUMMON));
    }
    let upgraded = fight.upgraded();
    for _ in 0..x {
        let Some(soul) = fight.add_at_random(V::Card::SOUL, Pile::Draw) else {
            break;
        };
        if upgraded {
            fight.modify_card(soul, Change::Upgrade, Lasting::ThisFight);
        }
    }
    Played::Done
}

pub(super) fn on_play<'a, V: crate::Build, K: FightKind<V>>(
    card: V::Card,
    fight: &mut K::Fight<'a>,
) -> Played<V, K> {
    match card {
        // `DeathsDoor`: its block, and its `Repeat` times more where its owner
        // applied Doom this turn.
        __c0 if __c0 == V::Card::DEATHS_DOOR => {
            let owner = fight.actor();
            let doomed = fight
                .history()
                .received_this_turn()
                .any(|(power, applier)| power == V::Power::DOOM_POWER && applier == Some(owner));
            let gains = 1 + if doomed {
                fight.var(V::CardVar::REPEAT)
            } else {
                0
            };
            for _ in 0..gains {
                let block = fight.resolve(Effect::GainBlock {
                    recipient: owner,
                    amount: Amount::Var(V::CardVar::BLOCK),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(block);
            }
            Played::Done
        }
        // `EndOfDays`: its Doom on each enemy that can be hit, then
        // `DoomKill` of those it leaves doomed.
        __c0 if __c0 == V::Card::END_OF_DAYS => {
            use sts2_core::CreatureView as _;

            let doom = sts2_core::PowerAmount::Counted(fight.var(V::CardVar::DOOM_POWER));
            for target in fight.hittable_opponents() {
                fight.apply_power(target, V::Power::DOOM_POWER, doom);
            }
            let doomed: Vec<_> = fight
                .hittable_opponents()
                .into_iter()
                .filter(|&target| {
                    fight.creatures().any(|creature| {
                        creature.id() == target
                            && match creature.power(V::Power::DOOM_POWER) {
                                Some(sts2_core::PowerAmount::Counted(doom)) => {
                                    crate::behaviour::cards::Number::from(creature.hp()) <= doom
                                }
                                Some(sts2_core::PowerAmount::Endless) => {
                                    unimplemented!("an endless Doom")
                                }
                                None => false,
                            }
                    })
                })
                .collect();
            fight.doom_kill(&doomed);
            Played::Done
        }
        __c0 if __c0 == V::Card::UNLEASH || __c0 == V::Card::PROTECTOR => {
            osty_attack::<V, _>(fight, V::CardVar::CALCULATED_DAMAGE)
        }
        __c0 if __c0 == V::Card::RIGHT_HAND_HAND => {
            osty_attack::<V, _>(fight, V::CardVar::OSTY_DAMAGE)
        }
        __c0 if __c0 == V::Card::POKE || __c0 == V::Card::FLATTEN => {
            osty_attack::<V, _>(fight, V::CardVar::OSTY_DAMAGE)
        }
        // `Squeeze`: its calculated damage from Osty to the target, where
        // Osty stands.
        __c0 if __c0 == V::Card::SQUEEZE => {
            osty_hit::<V, _>(fight, V::CardVar::CALCULATED_DAMAGE);
            Played::Done
        }
        // `SicEm`: Osty's damage to the target, where Osty stands, then its
        // power on the target.
        __c0 if __c0 == V::Card::SIC_EM => {
            osty_hit::<V, _>(fight, V::CardVar::OSTY_DAMAGE);
            let target = fight.target();
            fight.apply_power(
                target,
                V::Power::SIC_EM_POWER,
                sts2_core::PowerAmount::Counted(fight.var(V::CardVar::SIC_EM_POWER)),
            );
            Played::Done
        }
        __c0 if __c0 == V::Card::FETCH => fetch::<V, _>(fight),
        __c0 if __c0 == V::Card::DRAIN_POWER => drain_power::<V, _>(fight),
        __c0 if __c0 == V::Card::REAVE => reave::<V, _>(fight),
        __c0 if __c0 == V::Card::PUTREFY => putrefy::<V, _>(fight),
        __c0 if __c0 == V::Card::SPUR => spur::<V, _>(fight),
        __c0 if __c0 == V::Card::SACRIFICE => sacrifice::<V, _>(fight),
        __c0 if __c0 == V::Card::ERADICATE => eradicate::<V, _>(fight),
        __c0 if __c0 == V::Card::GRAVE_WARDEN => grave_warden::<V, _>(fight),
        __c0 if __c0 == V::Card::DIRGE => dirge::<V, _>(fight),
        // `PullFromBelow`: its damage to the target, as many times as its
        // calculated hits, counted before the first.
        __c0 if __c0 == V::Card::PULL_FROM_BELOW => {
            let target = fight.target();
            let hits = fight.var(V::CardVar::CALCULATED_HITS);
            fight.attack(Scaling::Dynamic, |fight| {
                for _ in 0..hits {
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
        // `Rattle`: where Osty stands, its damage from Osty to the target, as
        // many times as its calculated hits, counted before the first.
        __c0 if __c0 == V::Card::RATTLE => {
            let Some(osty) = fight.osty() else {
                return Played::Done;
            };
            let target = fight.target();
            let hits = fight.var(V::CardVar::CALCULATED_HITS);
            fight.attack_from(osty, Scaling::Dynamic, |fight| {
                for _ in 0..hits {
                    let hit = fight.resolve(Effect::Attack {
                        dealer: osty,
                        target,
                        damage: Amount::Var(V::CardVar::OSTY_DAMAGE),
                        scaling: Scaling::Dynamic,
                    });
                    fight.apply(hit);
                }
            });
            Played::Done
        }
        __c0 if __c0 == V::Card::MISERY => misery::<V, _>(fight),
        // `NoEscape`: its calculated Doom on the target.
        __c0 if __c0 == V::Card::NO_ESCAPE => {
            let target = fight.target();
            let doom = sts2_core::PowerAmount::Counted(fight.var(V::CardVar::CALCULATED_DOOM));
            fight.apply_power(target, V::Power::DOOM_POWER, doom);
            Played::Done
        }
        // `Undeath`: its block, then a clone of itself into the discard pile.
        __c0 if __c0 == V::Card::UNDEATH => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(V::CardVar::BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            fight.clone_this(Pile::Discard);
            Played::Done
        }
        // `SharedFate`: its Strength off its owner, then off the target.
        __c0 if __c0 == V::Card::SHARED_FATE => {
            let own = fight.var(V::CardVar::PLAYER_STRENGTH_LOSS);
            fight.apply_to_actor(
                V::Power::STRENGTH_POWER,
                sts2_core::PowerAmount::Counted(-own),
            );
            let theirs = fight.var(V::CardVar::ENEMY_STRENGTH_LOSS);
            let target = fight.target();
            fight.apply_power(
                target,
                V::Power::STRENGTH_POWER,
                sts2_core::PowerAmount::Counted(-theirs),
            );
            Played::Done
        }
        __c0 if __c0 == V::Card::BONE_SHARDS => bone_shards::<V, _>(fight),
        __c0 if __c0 == V::Card::NEGATIVE_PULSE => negative_pulse::<V, _>(fight),
        __c0 if __c0 == V::Card::SEVERANCE => severance::<V, _>(fight),
        // `CaptureSpirit`: its damage to the target, unblockable and
        // unpowered, then its `Cards` of Souls into the draw pile at random.
        // The damage meets no block.
        __c0 if __c0 == V::Card::CAPTURE_SPIRIT => {
            let damage = fight.resolve(Effect::Attack {
                dealer: fight.actor(),
                target: fight.target(),
                damage: Amount::Var(V::CardVar::DAMAGE),
                scaling: Scaling::Static,
            });
            fight.apply_unblockable(damage);
            for _ in 0..fight.var(V::CardVar::CARDS) {
                if fight.add_at_random(V::Card::SOUL, Pile::Draw).is_none() {
                    break;
                }
            }
            Played::Done
        }
        // `Friendship`: its Strength off its owner, then its Energy as
        // Friendship.
        __c0 if __c0 == V::Card::FRIENDSHIP => {
            let strength = fight.var(V::CardVar::STRENGTH_POWER);
            fight.apply_to_actor(
                V::Power::STRENGTH_POWER,
                sts2_core::PowerAmount::Counted(-strength),
            );
            let energy = fight.var(V::CardVar::ENERGY);
            fight.apply_to_actor(
                V::Power::FRIENDSHIP_POWER,
                sts2_core::PowerAmount::Counted(energy),
            );
            Played::Done
        }
        __c0 if __c0 == V::Card::SEANCE
            || __c0 == V::Card::HANG
            || __c0 == V::Card::THE_SCYTHE
            || __c0 == V::Card::CLEANSE
            || __c0 == V::Card::HIGH_FIVE
            || __c0 == V::Card::GRAVEBLAST
            || __c0 == V::Card::SCULPTING_STRIKE
            || __c0 == V::Card::SNAP
            || __c0 == V::Card::TRANSFIGURE
            || __c0 == V::Card::BLIGHT_STRIKE
            || __c0 == V::Card::DREDGE
            || __c0 == V::Card::EIDOLON =>
        {
            start::<V, K>(card, fight)
        }
        card => crate::behaviour::cards::generated::<V, K>(card, fight),
    }
}

/// A Necrobinder card's play that can stop on a question: from its start to
/// its end, yielding each question it stops on. Each step is lent the fight,
/// and after a question the answer.
#[diapause::coroutine(yield = Ask<V>, resume = for<'f, 'a> Lent<'f, 'a, V, K>)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub fn paused<V: crate::Build, K: FightKind<V>>(card: V::Card) {
    match card {
        // `Seance`: its `Cards` of the draw pile picked, each
        // `CardCmd.TransformTo<Soul>`.
        __c0 if __c0 == V::Card::SEANCE => {
            let count = usize::try_from(resumed!().fight.var(V::CardVar::CARDS))
                .ok()
                .and_then(nonmax::NonMaxUsize::new)
                .expect("a count of cards is not negative");
            yield_!(Ask::Asking {
                pile: Pile::Draw,
                selection: Selection::Exactly(count),
            });
            let Lent { fight, taken } = resumed!();
            let soul = sts2_core::CardIdent::new(V::Card::SOUL);
            for &picked in crate::behaviour::paused::selected::<V>(taken).as_slice() {
                fight.transform(picked, soul);
            }
        }
        // `Hang`: its damage to the target, then Hang on it: as much as it
        // already carries, and at least two.
        __c0 if __c0 == V::Card::HANG => {
            strike::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            hang_hit::<V, _>(resumed!().fight);
        }
        // `TheScythe`: its damage to the target, then `BuffFromPlay` of its
        // `Increase` on this copy and on its deck version.
        __c0 if __c0 == V::Card::THE_SCYTHE => {
            strike::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            the_scythe_hit::<V, _>(resumed!().fight);
        }
        // `Cleanse`: its `Summon` of Osty, then one card of the draw pile
        // picked, for `CardCmd.Exhaust`.
        __c0 if __c0 == V::Card::CLEANSE => {
            let fight = &mut *resumed!().fight;
            fight.summon_osty(fight.var(V::CardVar::SUMMON));
            yield_!(Ask::Asking {
                pile: Pile::Draw,
                selection: ONE,
            });
            let Lent { fight, taken } = resumed!();
            fight.act_on(
                PileAct::Exhaust,
                crate::behaviour::paused::selected::<V>(taken).as_slice(),
            );
        }
        // `HighFive`: where Osty stands, its damage from Osty to every enemy,
        // then its Vulnerable on each enemy that can be hit then. A question
        // left before one Vulnerable waits.
        __c0 if __c0 == V::Card::HIGH_FIVE => {
            if high_five_attack::<V, _>(resumed!().fight) {
                if resumed!().fight.waiting() {
                    yield_!(Ask::Waiting);
                }
                let held: crate::behaviour::cards::Held =
                    crate::behaviour::cards::Held::of(&resumed!().fight.hittable_opponents());
                let count: usize = held.creatures().count();
                let mut vulnerable: Number = resumed!().fight.var(V::CardVar::VULNERABLE_POWER);
                for at in 0..count {
                    if resumed!().fight.waiting() {
                        yield_!(Ask::Waiting);
                        vulnerable = resumed!().fight.var(V::CardVar::VULNERABLE_POWER);
                    }
                    let target = held.creatures().nth(at).expect("one of those held");
                    resumed!().fight.apply_power(
                        target,
                        V::Power::VULNERABLE_POWER,
                        sts2_core::PowerAmount::Counted(vulnerable),
                    );
                }
            }
        }
        // `Graveblast`: its damage to the target, then one card from the
        // discard pile picked into hand, as `CardPileCmd.Add`.
        __c0 if __c0 == V::Card::GRAVEBLAST => {
            strike::<V, _>(resumed!().fight);
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
        // `SculptingStrike`: its damage to the target, then one card of the
        // hand that is not Ethereal of its own picked, for
        // `CardCmd.ApplyKeyword(cardModel, CardKeyword.Ethereal)`.
        __c0 if __c0 == V::Card::SCULPTING_STRIKE => {
            strike::<V, _>(resumed!().fight);
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
            }
            let cards: Hand<V> =
                without_keyword::<V, _>(resumed!().fight, CardKeyword::Ethereal, true);
            yield_!(Ask::AskingAmong {
                cards,
                selection: ONE,
            });
            add_keyword::<V, K>(resumed!(), CardKeyword::Ethereal);
        }
        // `Snap`: Osty's damage to the target, where Osty stands, then one
        // card of the hand that does not Retain picked, for
        // `CardCmd.ApplyKeyword(cardModel, CardKeyword.Retain)`.
        __c0 if __c0 == V::Card::SNAP => {
            osty_hit::<V, _>(resumed!().fight, V::CardVar::OSTY_DAMAGE);
            let cards: Hand<V> =
                without_keyword::<V, _>(resumed!().fight, CardKeyword::Retain, false);
            yield_!(Ask::AskingAmong {
                cards,
                selection: ONE,
            });
            add_keyword::<V, K>(resumed!(), CardKeyword::Retain);
        }
        // `Transfigure`: one card of the hand picked, to cost one more for
        // the fight, where it has a cost that is not X, and to play once
        // more.
        __c0 if __c0 == V::Card::TRANSFIGURE => {
            yield_!(Ask::Asking {
                pile: Pile::Hand,
                selection: ONE,
            });
            let Lent { fight, taken } = resumed!();
            for &picked in crate::behaviour::paused::selected::<V>(taken).as_slice() {
                if let sts2_core::Cost::Fixed(_) =
                    sts2_core::CardTable::cost(fight.card_ident(picked).card)
                {
                    fight.modify_card(
                        picked,
                        Change::ChangeCost(CostPool::Energy, 1),
                        Lasting::ThisFight,
                    );
                }
                fight.add_replays(sts2_core::Writing::One(picked), 1);
            }
        }
        // `BlightStrike`: its damage to the target, then as much Doom on it
        // as the attack's hits dealt, block included.
        __c0 if __c0 == V::Card::BLIGHT_STRIKE => {
            let mut dealt: Number = {
                let fight = &mut *resumed!().fight;
                let target = fight.target();
                let results = fight.attack(Scaling::Dynamic, |attacking| {
                    let attack = attacking.resolve(Effect::Attack {
                        dealer: attacking.actor(),
                        target,
                        damage: Amount::Var(V::CardVar::DAMAGE),
                        scaling: Scaling::Dynamic,
                    });
                    attacking.apply(attack);
                });
                total::<V>(&results)
            };
            if resumed!().fight.waiting() {
                yield_!(Ask::Waiting);
                dealt = total::<V>(&resumed!().fight.waited_attack_results());
            }
            let fight = &mut *resumed!().fight;
            fight.apply_power(
                fight.target(),
                V::Power::DOOM_POWER,
                sts2_core::PowerAmount::Counted(dealt),
            );
        }
        // `Dredge`: its `Cards` from the discard pile picked into hand, as
        // many as the hand has room for, as `CardPileCmd.Add`.
        __c0 if __c0 == V::Card::DREDGE => {
            let room: Option<nonmax::NonMaxUsize> = dredge_room::<V, _>(resumed!().fight);
            if let Some(count) = room {
                yield_!(Ask::Asking {
                    pile: Pile::Discard,
                    selection: Selection::Exactly(count),
                });
                let Lent { fight, taken } = resumed!();
                fight.act_on(
                    PileAct::IntoHand,
                    crate::behaviour::paused::selected::<V>(taken).as_slice(),
                );
            }
        }
        // `Eidolon`: every card of the hand exhausted, one at a time, then
        // one Intangible where that came to nine or more. A hook's question
        // inside an exhaust stands the rest still.
        __c0 if __c0 == V::Card::EIDOLON => {
            let cards: Hand<V> = resumed!().fight.cards_in(Pile::Hand).into_iter().collect();
            let count: usize = cards.len();
            for at in 0..count {
                resumed!().fight.act_on(PileAct::Exhaust, &[cards[at]]);
                if resumed!().fight.waiting() {
                    yield_!(Ask::Waiting);
                }
            }
            if count >= EIDOLON_CARDS {
                resumed!().fight.apply_to_actor(
                    V::Power::INTANGIBLE_POWER,
                    sts2_core::PowerAmount::Counted(1),
                );
            }
        }
        card => unreachable!("{card:?} never stops on a question"),
    }
}

crate::behaviour::paused::paused_play!(Card, crate::behaviour::cards::Resume::Necrobinder);

/// The cards in hand without `keyword`, read locally where `local` says.
fn without_keyword<V: crate::Build, F: Fight<V>>(
    fight: &F,
    keyword: CardKeyword,
    local: bool,
) -> Hand<V> {
    fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&card| {
            !if local {
                fight.card_local_keyword(card, keyword)
            } else {
                fight.card_keyword(card, keyword)
            }
        })
        .collect()
}

/// `CardCmd.ApplyKeyword` of `keyword` to each card a pick took.
fn add_keyword<V: crate::Build, K: FightKind<V>>(
    lent: &mut Lent<'_, '_, V, K>,
    keyword: CardKeyword,
) {
    let Lent { fight, taken } = lent;
    for &picked in crate::behaviour::paused::selected::<V>(taken).as_slice() {
        fight.modify_card(picked, Change::AddKeyword(keyword), Lasting::ThisFight);
    }
}

/// What the hits of an attack dealt, block included.
fn total<V: crate::Build>(results: &sts2_core::AttackResults<V>) -> Number {
    results.iter().map(|result| result.total).sum()
}

/// How many cards `Dredge` takes: its `Cards`, or as many as the hand has
/// room for. `None` where that is none.
fn dredge_room<V: crate::Build, F: Fight<V>>(fight: &F) -> Option<nonmax::NonMaxUsize> {
    let cards: usize =
        num_traits::cast(fight.var(V::CardVar::CARDS)).expect("a count of cards is not negative");
    let room = sts2_core::MAX_CARDS_IN_HAND.saturating_sub(fight.cards_in(Pile::Hand).len());
    nonmax::NonMaxUsize::new(cards.min(room)).filter(|count| count.get() > 0)
}

/// `HighFive`'s attack: where Osty stands, its damage from Osty to every
/// enemy. Whether Osty stood.
fn high_five_attack<V: crate::Build, F: Fight<V>>(fight: &mut F) -> bool {
    let Some(osty) = fight.osty() else {
        return false;
    };
    fight.attack_from(osty, Scaling::Dynamic, |fight| {
        let targets = fight.opponents();
        // One `CreatureCmd.Damage` over them all.
        fight.damage_command(|fight| {
            for target in targets {
                let hit = fight.resolve(Effect::Attack {
                    dealer: osty,
                    target,
                    damage: Amount::Var(V::CardVar::OSTY_DAMAGE),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(hit);
            }
        });
    });
    true
}

/// `Eidolon`'s `_intangibleThreshold`.
const EIDOLON_CARDS: usize = 9;

/// The rest of `Hang`, once its attack is over: Hang on the target, as much
/// as it already carries, and at least two.
fn hang_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    use sts2_core::CreatureView as _;

    let target = fight.target();
    let carried = fight
        .creatures()
        .find(|creature| creature.id() == target)
        .and_then(|creature| creature.power(V::Power::HANG_POWER));
    let carried = match carried {
        Some(sts2_core::PowerAmount::Counted(amount)) => amount,
        Some(sts2_core::PowerAmount::Endless) => unimplemented!("an endless Hang"),
        None => 0,
    };
    let added = carried.max(HANG_AT_LEAST).min((HANG_MOST - carried).max(0));
    fight.apply_power(
        target,
        V::Power::HANG_POWER,
        sts2_core::PowerAmount::Counted(added),
    );
}

/// The rest of `TheScythe`, once its attack is over: `BuffFromPlay` of its
/// `Increase` on this copy and on its deck version.
fn the_scythe_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) {
    let increase = fight.var(V::CardVar::INCREASE);
    fight.modify_this(
        sts2_core::Change::Own(crate::behaviour::cards::CardWrite::BuffFromPlay(increase)),
        sts2_core::Lasting::ThisFight,
    );
    fight.write_deck_version(crate::behaviour::cards::DeckWrite::BuffFromPlay(increase));
}

/// `DamageCmd.Attack(damage).FromOsty(Owner.Osty, this).Targeting(target)`,
/// unless `Osty.CheckMissingWithAnim` finds no Osty. Osty is the attacker, so
/// the hooks an attack raises name Osty, and the card is what they read as
/// the source.
fn osty_attack<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    damage: V::CardVar,
) -> Played<V, F::Kind> {
    osty_hit::<V, _>(fight, damage);
    Played::Done
}

/// Osty's damage to the target, where Osty stands.
fn osty_hit<V: crate::Build, F: Fight<V>>(fight: &mut F, damage: V::CardVar) {
    let Some(osty) = fight.osty() else {
        return;
    };
    let target = fight.target();
    fight.attack_from(osty, Scaling::Dynamic, |fight| {
        let hit = fight.resolve(Effect::Attack {
            dealer: osty,
            target,
            damage: Amount::Var(damage),
            scaling: Scaling::Dynamic,
        });
        fight.apply(hit);
    });
}

/// `Hang`'s `Math.Max(2, powerAmount)`: the least Hang it adds.
const HANG_AT_LEAST: crate::behaviour::cards::Number = 2;

/// The stack Hang keeps under, `999999999`.
const HANG_MOST: crate::behaviour::cards::Number = 999_999_999;

/// Deliver a notification to one Necrobinder card in the fight.
pub(super) fn event_hook<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    card: V::Card,
    id: sts2_core::CardId<V>,
    printed: &P,
    history: sts2_core::History<'_, V>,
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (card, hook) {
        // `RightHandHand.AfterCardPlayedLate`: back into hand from the discard
        // pile after a play of its owner's that spent its `Energy` or more. A
        // fight here has one player.
        (__c0, EventHook::CardPlayedLate { energy, .. }) if __c0 == V::Card::RIGHT_HAND_HAND => {
            if i64::from(energy) >= i64::from(printed.face(V::CardVar::ENERGY)) {
                asked.push(HookEffect::AddSelfToHand {
                    only_from: Some(Pile::Discard),
                });
            }
        }
        // `Flatten.AfterAttack`, which answers an attack by its owner's Osty
        // alone. A fight here has one player, so any Osty is theirs.
        (
            __c0,
            EventHook::AfterAttack {
                monster: Some(__c1),
                ..
            },
        ) if (__c0 == V::Card::FLATTEN && __c1 == <V::Monster as crate::ids::MonsterIds>::OSTY) => {
            asked.push(HookEffect::ModifySelf {
                change: Change::SetCost(CostPool::Energy, 0),
                lasting: Lasting::ThisTurn,
            })
        }
        (__c0, EventHook::AfterAttack { .. }) if __c0 == V::Card::FLATTEN => {}
        // `Flatten.AfterCardEnteredCombat`, which answers its own entry
        // alone, and then only once Osty has attacked this turn. A fight here
        // has one player, so any Osty is theirs.
        (__c0, EventHook::CardEnteredCombat { at, .. }) if __c0 == V::Card::FLATTEN => {
            if at.id() == id
                && history
                    .attacked_this_turn()
                    .any(|monster| monster == Some(<V::Monster as crate::ids::MonsterIds>::OSTY))
            {
                asked.push(HookEffect::ModifySelf {
                    change: Change::SetCost(CostPool::Energy, 0),
                    lasting: Lasting::ThisTurn,
                });
            }
        }
        // `Melancholy.AfterDeath`: a creature's death takes its `Energy` off
        // its cost for the rest of the fight, unless the creature's removal
        // was prevented. A card that hears a hook is in a combat pile.
        (
            __c0,
            EventHook::AfterDeath {
                removal_prevented: false,
                ..
            },
        ) if __c0 == V::Card::MELANCHOLY => asked.push(HookEffect::ModifySelf {
            change: Change::ChangeCost(CostPool::Energy, -printed.face(V::CardVar::ENERGY)),
            lasting: Lasting::ThisFight,
        }),
        (__c0, EventHook::AfterDeath { .. }) if __c0 == V::Card::MELANCHOLY => {}
        // `BansheesCry.AfterCardPlayed`: its `Energy` off its cost for the
        // rest of the fight for each Ethereal card its owner plays. A fight
        // here has one player.
        (__c0, EventHook::CardPlayed { ethereal: true, .. }) if __c0 == V::Card::BANSHEES_CRY => {
            asked.push(HookEffect::ModifySelf {
                change: Change::ChangeCost(CostPool::Energy, -printed.face(V::CardVar::ENERGY)),
                lasting: Lasting::ThisFight,
            })
        }
        (__c0, EventHook::CardPlayed { .. }) if __c0 == V::Card::BANSHEES_CRY => {}
        // `BansheesCry.AfterCardEnteredCombat`, for its own entry alone and
        // not a clone's: its `Energy` off for the rest of the fight for each
        // Ethereal play finished this fight before it came.
        (__c0, EventHook::CardEnteredCombat { at, clone, .. }) if __c0 == V::Card::BANSHEES_CRY => {
            if at.id() == id && !clone {
                let ethereal: crate::behaviour::cards::Number = history
                    .finished_this_fight()
                    .filter(|finished| finished.ethereal)
                    .count()
                    .try_into()
                    .expect("a fight's plays fit a game number");
                if ethereal != 0 {
                    asked.push(HookEffect::ModifySelf {
                        change: Change::ChangeCost(
                            CostPool::Energy,
                            -ethereal * printed.face(V::CardVar::ENERGY),
                        ),
                        lasting: Lasting::ThisFight,
                    });
                }
            }
        }
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
}
