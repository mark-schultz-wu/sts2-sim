//! The Necrobinder's cards, where the generator leaves them.

use sts2_core::{
    Amount, Change, CostPool, Effect, EventHook, Fight, HookEffect, Lasting, Pile, PileAct, Played,
    Scaling, Selection,
};

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

/// `Snap`: Osty's damage to the target, where Osty stands, then one card of
/// the hand that does not Retain made to.
fn snap<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    osty_hit::<V, _>(fight, V::CardVar::OSTY_DAMAGE);
    let cards = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&card| !fight.card_keyword(card, CardKeyword::Retain))
        .collect();
    Played::AskingAmong {
        cards,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: crate::behaviour::cards::Resume::Necrobinder(Resume::SnapRetain).into(),
    }
}

/// `Fetch`: where Osty stands, its damage to the target, then its `Cards`
/// drawn unless this copy finished a play already this turn.
fn fetch<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
fn drain_power<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
fn reave<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
fn putrefy<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    let amount = sts2_core::PowerAmount::Counted(fight.var(V::CardVar::POWER));
    fight.apply_power(target, V::Power::WEAK_POWER, amount);
    fight.apply_power(target, V::Power::VULNERABLE_POWER, amount);
    Played::Done
}

/// `Spur`: its `Summon` of Osty, then its `Heal` on Osty.
fn spur<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
fn sacrifice<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
fn eradicate<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
fn grave_warden<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
fn negative_pulse<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
fn severance<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    strike::<V, _>(fight);
    let soul = V::Card::SOUL;
    fight.add_at_random(soul, Pile::Draw);
    fight.add_generated_cards(sts2_core::CardIdent::new(soul), Pile::Discard, 1);
    fight.add_generated_cards(sts2_core::CardIdent::new(soul), Pile::Hand, 1);
    Played::Done
}

/// `BoneShards`: where Osty stands, its damage from Osty to every enemy, its
/// block, then Osty killed where it still lives.
fn bone_shards<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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

/// `BlightStrike`: its damage to the target, then as much Doom on it as the
/// attack's hits dealt, block included.
fn blight_strike<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
        return wait::<V>(Resume::BlightStrikeHit);
    }
    blight_strike_hit::<V, _>(fight, &results)
}

/// The rest of `BlightStrike`, once its attack is over: as much Doom on the
/// target as `results` dealt.
fn blight_strike_hit<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    results: &sts2_core::AttackResults<V>,
) -> Played<V> {
    let doom = results.iter().map(|result| result.total).sum();
    fight.apply_power(
        fight.target(),
        V::Power::DOOM_POWER,
        sts2_core::PowerAmount::Counted(doom),
    );
    Played::Done
}

/// `Misery`: the target's debuffs as they stand, then its damage to the
/// target. Each other hittable enemy then takes each of those debuffs: onto
/// the instance it stacks with through `PowerCmd.ModifyAmount`, else as a
/// new one through `PowerCmd.Apply`. Both come from the debuff's applier.
fn misery<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
fn dirge<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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

/// `SculptingStrike`: its damage to the target, then one card of the hand
/// that is not Ethereal of its own made Ethereal.
fn sculpting_strike<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
        return wait::<V>(Resume::SculptingStrikeHit);
    }
    sculpting_strike_hit::<V, _>(fight)
}

/// The rest of `SculptingStrike`, once its attack is over: one card of the
/// hand that is not Ethereal picked, to become so.
fn sculpting_strike_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let ethereal = CardKeyword::Ethereal;
    let cards = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&card| !fight.card_local_keyword(card, ethereal))
        .collect();
    Played::AskingAmong {
        cards,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: crate::behaviour::cards::Resume::Necrobinder(Resume::SculptingStrikeEthereal).into(),
    }
}

pub(super) fn on_play<V: crate::Build, F: Fight<V>>(card: V::Card, fight: &mut F) -> Played<V> {
    match card {
        // `Seance`: its `Cards` of the draw pile picked, to turn into Souls.
        __c0 if __c0 == V::Card::SEANCE => {
            let count = usize::try_from(fight.var(V::CardVar::CARDS))
                .ok()
                .and_then(nonmax::NonMaxUsize::new)
                .expect("a count of cards is not negative");
            Played::Asking {
                pile: Pile::Draw,
                selection: Selection::Exactly(count),
                at: crate::behaviour::cards::Resume::Necrobinder(Resume::SeanceTransforms).into(),
            }
        }
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
        // `Hang`: its damage to the target, then Hang on it: as much as it
        // already carries, and at least two.
        __c0 if __c0 == V::Card::HANG => {
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
                return wait::<V>(Resume::HangHit);
            }
            hang_hit::<V, _>(fight)
        }
        // `TheScythe`: its damage to the target, then `BuffFromPlay` of its
        // `Increase` on this copy and on its deck version.
        __c0 if __c0 == V::Card::THE_SCYTHE => {
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
                return wait::<V>(Resume::TheScytheHit);
            }
            the_scythe_hit::<V, _>(fight)
        }
        // `Cleanse`: its `Summon` of Osty, then one card of the draw pile
        // picked, to exhaust.
        __c0 if __c0 == V::Card::CLEANSE => {
            fight.summon_osty(fight.var(V::CardVar::SUMMON));
            Played::Asking {
                pile: Pile::Draw,
                selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
                at: crate::behaviour::cards::Resume::Necrobinder(Resume::CleanseExhausts).into(),
            }
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
        // `HighFive`: where Osty stands, its damage from Osty to every enemy,
        // then its Vulnerable on each enemy that can be hit.
        __c0 if __c0 == V::Card::HIGH_FIVE => {
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
            if fight.waiting() {
                return wait::<V>(Resume::HighFiveHit);
            }
            high_five_hit::<V, _>(fight)
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
        __c0 if __c0 == V::Card::GRAVEBLAST => graveblast::<V, _>(fight),
        __c0 if __c0 == V::Card::SCULPTING_STRIKE => sculpting_strike::<V, _>(fight),
        __c0 if __c0 == V::Card::SNAP => snap::<V, _>(fight),
        // `Transfigure`: one card of the hand picked, to cost one more and
        // play once more.
        __c0 if __c0 == V::Card::TRANSFIGURE => Played::Asking {
            pile: Pile::Hand,
            selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
            at: crate::behaviour::cards::Resume::Necrobinder(Resume::TransfigureReplay).into(),
        },
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
        __c0 if __c0 == V::Card::BLIGHT_STRIKE => blight_strike::<V, _>(fight),
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
        // `Friendship`: its Strength off its owner, then its Energy as
        // Friendship.
        __c0 if __c0 == V::Card::DREDGE => dredge::<V, _>(fight),
        // `Eidolon`: every card of the hand exhausted, one at a time, then one
        // Intangible where that came to nine or more.
        __c0 if __c0 == V::Card::EIDOLON => {
            let hand = fight.cards_in(Pile::Hand);
            let count = u8::try_from(hand.len()).expect("a hand's worth fits a u8");
            eidolon::<V, _>(fight, &hand, count)
        }
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
        card => crate::behaviour::cards::generated_from::<V, _>(card, fight, 0),
    }
}

/// Where a Necrobinder card stopped inside its own play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume<V: crate::Build> {
    /// `Seance`, after its owner picked the cards of the draw pile to turn
    /// into Souls.
    SeanceTransforms,
    /// `Cleanse`, after its owner picked the card of the draw pile to
    /// exhaust.
    CleanseExhausts,
    /// `Dredge`, after its owner picked the cards to take back.
    DredgeTakeBack,
    /// `Graveblast`, after its owner picked the card to take back.
    GraveblastTakeBack,
    /// `SculptingStrike`, after its owner picked the card to make Ethereal.
    SculptingStrikeEthereal,
    /// `Snap`, after its owner picked the card to Retain.
    SnapRetain,
    /// `Transfigure`, after its owner picked the card to replay.
    TransfigureReplay,
    /// After a question inside the card's attack was answered.
    HangHit,
    TheScytheHit,
    SculptingStrikeHit,
    BlightStrikeHit,
    HighFiveHit,
    /// `HighFive`, after a question inside one of its Vulnerables. `held` is
    /// the enemies it took after the attack, and `at` the one to go on from.
    HighFive {
        held: crate::behaviour::cards::Held,
        at: u8,
    },
    /// `Eidolon`, after a question inside its exhausts was answered, with the
    /// cards left to exhaust and how many the hand held.
    Eidolon {
        left: crate::behaviour::cards::Left<V>,
        count: u8,
    },
}

/// The rest of a Necrobinder card's play that stood still on a question its
/// own command raised.
pub(super) fn resume_waited<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V>,
) -> Played<V> {
    match at {
        Resume::HangHit => hang_hit::<V, _>(fight),
        Resume::TheScytheHit => the_scythe_hit::<V, _>(fight),
        Resume::SculptingStrikeHit => sculpting_strike_hit::<V, _>(fight),
        Resume::BlightStrikeHit => {
            let results = fight.waited_attack_results();
            blight_strike_hit::<V, _>(fight, &results)
        }
        Resume::HighFiveHit => high_five_hit::<V, _>(fight),
        Resume::HighFive { held, at } => high_five_vulnerable::<V, _>(fight, held, at),
        Resume::Eidolon { left, count } => {
            let left: Vec<_> = left.cards().collect();
            eidolon::<V, _>(fight, &left, count)
        }
        at => unreachable!("{at:?} stops on a pick, not on a question it waited for"),
    }
}

/// The rest of the play waits at `at`, where a command left a question and
/// more commands follow.
fn wait<V: crate::Build>(at: Resume<V>) -> Played<V> {
    Played::Waiting {
        at: crate::behaviour::cards::Resume::Necrobinder(at).into(),
    }
}

pub(super) fn resume_play<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V>,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        // `CardCmd.TransformTo<Soul>` of each one taken.
        Resume::SeanceTransforms => {
            let soul = sts2_core::CardIdent::new(V::Card::SOUL);
            for &card in chosen.as_slice() {
                fight.transform(card, soul);
            }
            Played::Done
        }
        // `CardCmd.Exhaust` of the one taken.
        Resume::CleanseExhausts => {
            fight.act_on(PileAct::Exhaust, chosen.as_slice());
            Played::Done
        }
        // `CardPileCmd.Add` of those taken, into hand.
        Resume::GraveblastTakeBack | Resume::DredgeTakeBack => {
            fight.act_on(PileAct::IntoHand, chosen.as_slice());
            Played::Done
        }
        // One more energy on its cost for the fight, where it has a cost that
        // is not X, and one more replay.
        Resume::TransfigureReplay => {
            for &card in chosen.as_slice() {
                if let sts2_core::Cost::Fixed(_) =
                    sts2_core::CardTable::cost(fight.card_ident(card).card)
                {
                    fight.modify_card(
                        card,
                        Change::ChangeCost(sts2_core::CostPool::Energy, 1),
                        Lasting::ThisFight,
                    );
                }
                fight.add_replays(sts2_core::Writing::One(card), 1);
            }
            Played::Done
        }
        // `CardCmd.ApplyKeyword(cardModel, CardKeyword.Retain)`.
        Resume::SnapRetain => {
            for &card in chosen.as_slice() {
                let retain = Change::AddKeyword(CardKeyword::Retain);
                fight.modify_card(card, retain, Lasting::ThisFight);
            }
            Played::Done
        }
        // `CardCmd.ApplyKeyword(cardModel, CardKeyword.Ethereal)`.
        Resume::SculptingStrikeEthereal => {
            for &card in chosen.as_slice() {
                let ethereal = Change::AddKeyword(CardKeyword::Ethereal);
                fight.modify_card(card, ethereal, Lasting::ThisFight);
            }
            Played::Done
        }
        at => unreachable!("{at:?} waits on a question, not a pick"),
    }
}

/// `Eidolon`'s `_intangibleThreshold`.
const EIDOLON_CARDS: usize = 9;

/// The rest of `Hang`, once its attack is over: Hang on the target, as much
/// as it already carries, and at least two.
fn hang_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
    Played::Done
}

/// The rest of `TheScythe`, once its attack is over: `BuffFromPlay` of its
/// `Increase` on this copy and on its deck version.
fn the_scythe_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let increase = fight.var(V::CardVar::INCREASE);
    fight.modify_this(
        sts2_core::Change::Own(crate::behaviour::cards::CardWrite::BuffFromPlay(increase)),
        sts2_core::Lasting::ThisFight,
    );
    fight.write_deck_version(crate::behaviour::cards::DeckWrite::BuffFromPlay(increase));
    Played::Done
}

/// The rest of `HighFive`, once Osty's attack is over: its Vulnerable on each
/// enemy that can be hit then.
fn high_five_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let held = crate::behaviour::cards::Held::of(&fight.hittable_opponents());
    high_five_vulnerable::<V, _>(fight, held, 0)
}

/// `HighFive`'s Vulnerable on each of `held` from the one at `from` on. A
/// question left before one waits.
fn high_five_vulnerable<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    held: crate::behaviour::cards::Held,
    from: u8,
) -> Played<V> {
    let vulnerable = sts2_core::PowerAmount::Counted(fight.var(V::CardVar::VULNERABLE_POWER));
    for (at, target) in held.creatures().enumerate().skip(usize::from(from)) {
        if fight.waiting() {
            let at = u8::try_from(at).expect("a fight's creatures fit a u8");
            return wait::<V>(Resume::HighFive { held, at });
        }
        fight.apply_power(target, V::Power::VULNERABLE_POWER, vulnerable);
    }
    Played::Done
}

/// `Eidolon` over the cards `cards`: each exhausted, then one Intangible
/// where the hand held nine or more.
fn eidolon<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    cards: &[sts2_core::CardAt<V>],
    count: u8,
) -> Played<V> {
    for (at, &card) in cards.iter().enumerate() {
        fight.act_on(PileAct::Exhaust, &[card]);
        if fight.waiting() {
            let left = crate::behaviour::cards::Left::of(&cards[at + 1..]);
            return wait::<V>(Resume::Eidolon { left, count });
        }
    }
    if usize::from(count) >= EIDOLON_CARDS {
        fight.apply_to_actor(
            V::Power::INTANGIBLE_POWER,
            sts2_core::PowerAmount::Counted(1),
        );
    }
    Played::Done
}

/// `Dredge`: its `Cards` from the discard pile picked into hand, as many as
/// the hand has room for.
fn dredge<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let cards: usize =
        num_traits::cast(fight.var(V::CardVar::CARDS)).expect("a count of cards is not negative");
    let room = sts2_core::MAX_CARDS_IN_HAND.saturating_sub(fight.cards_in(Pile::Hand).len());
    match nonmax::NonMaxUsize::new(cards.min(room)) {
        Some(count) if count.get() > 0 => Played::Asking {
            pile: Pile::Discard,
            selection: Selection::Exactly(count),
            at: crate::behaviour::cards::Resume::Necrobinder(Resume::DredgeTakeBack).into(),
        },
        _ => Played::Done,
    }
}

/// `Graveblast`: its damage to the target, then one card from the discard
/// pile picked into hand.
fn graveblast<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
    Played::Asking {
        pile: Pile::Discard,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: crate::behaviour::cards::Resume::Necrobinder(Resume::GraveblastTakeBack).into(),
    }
}

/// `DamageCmd.Attack(damage).FromOsty(Owner.Osty, this).Targeting(target)`,
/// unless `Osty.CheckMissingWithAnim` finds no Osty. Osty is the attacker, so
/// the hooks an attack raises name Osty, and the card is what they read as
/// the source.
fn osty_attack<V: crate::Build, F: Fight<V>>(fight: &mut F, damage: V::CardVar) -> Played<V> {
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
