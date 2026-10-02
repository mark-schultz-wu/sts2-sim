//! The Necrobinder's cards, where the generator leaves them.

use sts2_core::{
    Amount, Change, CostPool, Effect, EventHook, Fight, HookEffect, Lasting, Pile, PileAct, Played,
    Scaling, Selection,
};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardKeyword, CardVar, NecrobinderCard};
use super::super::super::generated::monsters::Monster;
use super::super::super::generated::powers::Power;
use super::V;

/// Its damage to the target, as one attack.
fn strike<F: Fight<V>>(fight: &mut F) {
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
}

/// `Snap`: Osty's damage to the target, where Osty stands, then one card of
/// the hand that does not Retain made to.
fn snap<F: Fight<V>>(fight: &mut F) -> Played<V> {
    osty_hit(fight, CardVar::OstyDamage);
    let cards = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&card| !fight.card_keyword(card, CardKeyword::Retain))
        .collect();
    Played::AskingAmong {
        cards,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: super::super::Resume::Card(super::Resume::Necrobinder(Resume::SnapRetain)),
    }
}

/// `Fetch`: where Osty stands, its damage to the target, then its `Cards`
/// drawn unless this copy finished a play already this turn.
fn fetch<F: Fight<V>>(fight: &mut F) -> Played<V> {
    if fight.osty().is_none() {
        return Played::Done;
    }
    osty_hit(fight, CardVar::OstyDamage);
    let this = fight.playing();
    let played =
        fight.history().this_turn.iter().any(
            |entry| matches!(entry, sts2_core::HistoryEntry::Finished(done) if done.id == this),
        );
    if !played {
        fight.draw(fight.var(CardVar::Cards));
    }
    Played::Done
}

/// `DrainPower`: its damage to the target, then its `Cards` of the discard
/// pile's upgradable cards taken at random off `CombatCardSelection`, each
/// upgraded.
fn drain_power<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::CardTable as _;

    strike(fight);
    let mut upgradable: Vec<_> = fight
        .cards_in(Pile::Discard)
        .into_iter()
        .filter(|&card| {
            let ident = fight.card_ident(card);
            ident.upgrade < ident.card.max_upgrade()
        })
        .collect();
    let count = usize::try_from(fight.var(CardVar::Cards)).expect("a count is not negative");
    for card in fight.take_random_cards(&mut upgradable, count) {
        fight.modify_card(card, Change::Upgrade, Lasting::ThisFight);
    }
    Played::Done
}

/// `Reave`: its damage to the target, then its `Cards` of Souls into the
/// draw pile at random, each upgraded where Reave is.
fn reave<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use super::super::super::generated::cards::{Cards, TokenCard};

    strike(fight);
    let upgraded = fight.upgraded();
    for _ in 0..fight.var(CardVar::Cards) {
        let Some(soul) = fight.add_at_random(Cards::Token(TokenCard::Soul), Pile::Draw) else {
            break;
        };
        if upgraded {
            fight.modify_card(soul, Change::Upgrade, Lasting::ThisFight);
        }
    }
    Played::Done
}

/// `Putrefy`: its `Power` as Weak, then as Vulnerable, on the target.
fn putrefy<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use super::super::super::generated::powers::Power;

    let target = fight.target();
    let amount = sts2_core::PowerAmount::Counted(fight.var(CardVar::Power));
    fight.apply_power(target, Power::WeakPower, amount);
    fight.apply_power(target, Power::VulnerablePower, amount);
    Played::Done
}

/// `Spur`: its `Summon` of Osty, then its `Heal` on Osty.
fn spur<F: Fight<V>>(fight: &mut F) -> Played<V> {
    fight.summon_osty(fight.var(CardVar::Summon));
    if let Some(osty) = fight.osty() {
        fight.heal(osty, fight.var(CardVar::Heal));
    }
    Played::Done
}

/// `Sacrifice`: where Osty stands, its calculated block worked out, then
/// Osty killed, then the block gained.
fn sacrifice<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let Some(osty) = fight.osty() else {
        return Played::Done;
    };
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(CardVar::CalculatedBlock),
        scaling: Scaling::Dynamic,
    });
    fight.kill(osty);
    fight.apply(block);
    Played::Done
}

/// `Eradicate`: its damage to the target, once for each energy its X spent,
/// as one attack.
fn eradicate<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    let hits = fight.x_value();
    fight.attack(Scaling::Dynamic, |fight| {
        for _ in 0..hits {
            let attack = fight.resolve(Effect::Attack {
                dealer: fight.actor(),
                target,
                damage: Amount::Var(CardVar::Damage),
                scaling: Scaling::Dynamic,
            });
            fight.apply(attack);
        }
    });
    Played::Done
}

/// `GraveWarden`: its block, then its `Cards` of Souls into the draw pile at
/// random.
fn grave_warden<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use super::super::super::generated::cards::{Cards, TokenCard};

    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(CardVar::Block),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    for _ in 0..fight.var(CardVar::Cards) {
        if fight
            .add_at_random(Cards::Token(TokenCard::Soul), Pile::Draw)
            .is_none()
        {
            break;
        }
    }
    Played::Done
}

/// `NegativePulse`: its block, then its Doom on each enemy that can be hit.
fn negative_pulse<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use super::super::super::generated::powers::Power;

    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(CardVar::Block),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    let doom = sts2_core::PowerAmount::Counted(fight.var(CardVar::DoomPower));
    for target in fight.hittable_opponents() {
        fight.apply_power(target, Power::DoomPower, doom);
    }
    Played::Done
}

/// `Severance`: its damage to the target, then a Soul into the draw pile at
/// random, one into the discard pile and one into hand.
fn severance<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use super::super::super::generated::cards::{Cards, TokenCard};

    strike(fight);
    let soul = Cards::Token(TokenCard::Soul);
    fight.add_at_random(soul, Pile::Draw);
    fight.add_generated_cards(sts2_core::CardIdent::new(soul), Pile::Discard, 1);
    fight.add_generated_cards(sts2_core::CardIdent::new(soul), Pile::Hand, 1);
    Played::Done
}

/// `BoneShards`: where Osty stands, its damage from Osty to every enemy, its
/// block, then Osty killed where it still lives.
fn bone_shards<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let Some(osty) = fight.osty() else {
        return Played::Done;
    };
    fight.attack_from(osty, Scaling::Dynamic, |fight| {
        for target in fight.opponents() {
            let hit = fight.resolve(Effect::Attack {
                dealer: osty,
                target,
                damage: Amount::Var(CardVar::OstyDamage),
                scaling: Scaling::Dynamic,
            });
            fight.apply(hit);
        }
    });
    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(CardVar::Block),
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
fn blight_strike<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use super::super::super::generated::powers::Power;

    let target = fight.target();
    let results = fight.attack(Scaling::Dynamic, |fight| {
        let attack = fight.resolve(Effect::Attack {
            dealer: fight.actor(),
            target,
            damage: Amount::Var(CardVar::Damage),
            scaling: Scaling::Dynamic,
        });
        fight.apply(attack);
    });
    let doom = results.iter().map(|result| result.total).sum();
    fight.apply_power(
        target,
        Power::DoomPower,
        sts2_core::PowerAmount::Counted(doom),
    );
    Played::Done
}

/// `Misery`: the target's debuffs as they stand, then its damage to the
/// target. Each other hittable enemy then takes each of those debuffs: onto
/// the instance it stacks with through `PowerCmd.ModifyAmount`, else as a
/// new one through `PowerCmd.Apply`. Both come from the debuff's applier.
fn misery<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use super::super::super::generated::powers::{Power, PowerInstanceType};
    use sts2_core::{PowerAmount, Table as _};

    let target = fight.target();
    let debuffs: Vec<_> = fight
        .powers_on(target)
        .into_iter()
        .filter(|held| super::debuff_for_amount(held.power, held.amount))
        .collect();
    strike(fight);
    for enemy in fight.hittable_opponents() {
        if enemy == target {
            continue;
        }
        for held in &debuffs {
            let power = held.power;
            // `ITemporaryPower.IgnoreNextInstance`, which the copy is given.
            if super::super::powers::lends(power).is_some() {
                unimplemented!("Misery copying {power:?}, which ignores its next instance");
            }
            // `ClonePreservingMutability` builds its internal data anew, which
            // is all a noted play is. Imbalanced's off-balance is the Bowlbug
            // Rock's `IsOffBalance`, or a stun, which the port keeps on the
            // power and the game's model holds nothing of.
            if !held.fresh && !super::super::powers::clone_starts_anew(power) {
                unimplemented!("Misery copying {power:?}, whose model carries state");
            }
            let PowerAmount::Counted(amount) = held.amount else {
                unimplemented!("Misery copying an endless {power:?}");
            };
            match Power::data(power).instance {
                None if fight.powers_on(enemy).iter().any(|on| on.power == power) => {
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
fn dirge<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use super::super::super::generated::cards::{Cards, TokenCard};

    let x = fight.x_value();
    for _ in 0..x {
        fight.summon_osty(fight.var(CardVar::Summon));
    }
    let upgraded = fight.upgraded();
    for _ in 0..x {
        let Some(soul) = fight.add_at_random(Cards::Token(TokenCard::Soul), Pile::Draw) else {
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
fn sculpting_strike<F: Fight<V>>(fight: &mut F) -> Played<V> {
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
    let ethereal = CardKeyword::Ethereal;
    let cards = fight
        .cards_in(Pile::Hand)
        .into_iter()
        .filter(|&card| !fight.card_keyword(card, ethereal))
        .collect();
    Played::AskingAmong {
        cards,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: super::super::Resume::Card(super::Resume::Necrobinder(Resume::SculptingStrikeEthereal)),
    }
}

pub(super) fn on_play<F: Fight<V>>(card: NecrobinderCard, fight: &mut F) -> Played<V> {
    match card {
        // `Seance`: its `Cards` of the draw pile picked, to turn into Souls.
        NecrobinderCard::Seance => {
            let count = usize::try_from(fight.var(CardVar::Cards))
                .ok()
                .and_then(nonmax::NonMaxUsize::new)
                .expect("a count of cards is not negative");
            Played::Asking {
                pile: Pile::Draw,
                selection: Selection::Exactly(count),
                at: super::super::Resume::Card(super::Resume::Necrobinder(
                    Resume::SeanceTransforms,
                )),
            }
        }
        // `DeathsDoor`: its block, and its `Repeat` times more where its owner
        // applied Doom this turn.
        NecrobinderCard::DeathsDoor => {
            let owner = fight.actor();
            let doomed = fight
                .history()
                .received_this_turn()
                .any(|(power, applier)| power == Power::DoomPower && applier == Some(owner));
            let gains = 1 + if doomed {
                fight.var(CardVar::Repeat)
            } else {
                0
            };
            for _ in 0..gains {
                let block = fight.resolve(Effect::GainBlock {
                    recipient: owner,
                    amount: Amount::Var(CardVar::Block),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(block);
            }
            Played::Done
        }
        // `Hang`: its damage to the target, then Hang on it: as much as it
        // already carries, and at least two.
        NecrobinderCard::Hang => {
            use sts2_core::CreatureView as _;

            let target = fight.target();
            fight.attack(Scaling::Dynamic, |fight| {
                let hit = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(CardVar::Damage),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(hit);
            });
            let carried = fight
                .creatures()
                .find(|creature| creature.id() == target)
                .and_then(|creature| creature.power(Power::HangPower));
            let carried = match carried {
                Some(sts2_core::PowerAmount::Counted(amount)) => amount,
                Some(sts2_core::PowerAmount::Endless) => unimplemented!("an endless Hang"),
                None => 0,
            };
            let added = carried.max(HANG_AT_LEAST).min((HANG_MOST - carried).max(0));
            fight.apply_power(
                target,
                Power::HangPower,
                sts2_core::PowerAmount::Counted(added),
            );
            Played::Done
        }
        // `TheScythe`: its damage to the target, then `BuffFromPlay` of its
        // `Increase` on this copy and on its deck version.
        NecrobinderCard::TheScythe => {
            let target = fight.target();
            fight.attack(Scaling::Dynamic, |fight| {
                let hit = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Var(CardVar::Damage),
                    scaling: Scaling::Dynamic,
                });
                fight.apply(hit);
            });
            let increase = fight.var(CardVar::Increase);
            fight.modify_this(
                sts2_core::Change::Raise(CardVar::Damage, increase),
                sts2_core::Lasting::ThisFight,
            );
            fight.write_deck_version(super::DeckWrite::BuffFromPlay(increase));
            Played::Done
        }
        // `Cleanse`: its `Summon` of Osty, then one card of the draw pile
        // picked, to exhaust.
        NecrobinderCard::Cleanse => {
            fight.summon_osty(fight.var(CardVar::Summon));
            Played::Asking {
                pile: Pile::Draw,
                selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
                at: super::super::Resume::Card(super::Resume::Necrobinder(Resume::CleanseExhausts)),
            }
        }
        // `EndOfDays`: its Doom on each enemy that can be hit, then
        // `DoomKill` of those it leaves doomed.
        NecrobinderCard::EndOfDays => {
            use sts2_core::CreatureView as _;

            let doom = sts2_core::PowerAmount::Counted(fight.var(CardVar::DoomPower));
            for target in fight.hittable_opponents() {
                fight.apply_power(target, Power::DoomPower, doom);
            }
            let doomed: Vec<_> = fight
                .hittable_opponents()
                .into_iter()
                .filter(|&target| {
                    fight.creatures().any(|creature| {
                        creature.id() == target
                            && match creature.power(Power::DoomPower) {
                                Some(sts2_core::PowerAmount::Counted(doom)) => {
                                    super::Number::from(creature.hp()) <= doom
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
        NecrobinderCard::HighFive => {
            let Some(osty) = fight.osty() else {
                return Played::Done;
            };
            fight.attack_from(osty, Scaling::Dynamic, |fight| {
                for target in fight.opponents() {
                    let hit = fight.resolve(Effect::Attack {
                        dealer: osty,
                        target,
                        damage: Amount::Var(CardVar::OstyDamage),
                        scaling: Scaling::Dynamic,
                    });
                    fight.apply(hit);
                }
            });
            let vulnerable = sts2_core::PowerAmount::Counted(fight.var(CardVar::VulnerablePower));
            for target in fight.hittable_opponents() {
                fight.apply_power(target, Power::VulnerablePower, vulnerable);
            }
            Played::Done
        }
        NecrobinderCard::Unleash | NecrobinderCard::Protector => {
            osty_attack(fight, CardVar::CalculatedDamage)
        }
        NecrobinderCard::RightHandHand => osty_attack(fight, CardVar::OstyDamage),
        NecrobinderCard::Poke | NecrobinderCard::Flatten => osty_attack(fight, CardVar::OstyDamage),
        NecrobinderCard::Graveblast => graveblast(fight),
        NecrobinderCard::SculptingStrike => sculpting_strike(fight),
        NecrobinderCard::Snap => snap(fight),
        // `Transfigure`: one card of the hand picked, to cost one more and
        // play once more.
        NecrobinderCard::Transfigure => Played::Asking {
            pile: Pile::Hand,
            selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
            at: super::super::Resume::Card(super::Resume::Necrobinder(Resume::TransfigureReplay)),
        },
        // `Squeeze`: its calculated damage from Osty to the target, where
        // Osty stands.
        NecrobinderCard::Squeeze => {
            osty_hit(fight, CardVar::CalculatedDamage);
            Played::Done
        }
        // `SicEm`: Osty's damage to the target, where Osty stands, then its
        // power on the target.
        NecrobinderCard::SicEm => {
            osty_hit(fight, CardVar::OstyDamage);
            let target = fight.target();
            fight.apply_power(
                target,
                Power::SicEmPower,
                sts2_core::PowerAmount::Counted(fight.var(CardVar::SicEmPower)),
            );
            Played::Done
        }
        NecrobinderCard::Fetch => fetch(fight),
        NecrobinderCard::DrainPower => drain_power(fight),
        NecrobinderCard::Reave => reave(fight),
        NecrobinderCard::Putrefy => putrefy(fight),
        NecrobinderCard::Spur => spur(fight),
        NecrobinderCard::Sacrifice => sacrifice(fight),
        NecrobinderCard::Eradicate => eradicate(fight),
        NecrobinderCard::GraveWarden => grave_warden(fight),
        NecrobinderCard::Dirge => dirge(fight),
        // `PullFromBelow`: its damage to the target, as many times as its
        // calculated hits, counted before the first.
        NecrobinderCard::PullFromBelow => {
            let target = fight.target();
            let hits = fight.var(CardVar::CalculatedHits);
            fight.attack(Scaling::Dynamic, |fight| {
                for _ in 0..hits {
                    let hit = fight.resolve(Effect::Attack {
                        dealer: fight.actor(),
                        target,
                        damage: Amount::Var(CardVar::Damage),
                        scaling: Scaling::Dynamic,
                    });
                    fight.apply(hit);
                }
            });
            Played::Done
        }
        // `Rattle`: where Osty stands, its damage from Osty to the target, as
        // many times as its calculated hits, counted before the first.
        NecrobinderCard::Rattle => {
            let Some(osty) = fight.osty() else {
                return Played::Done;
            };
            let target = fight.target();
            let hits = fight.var(CardVar::CalculatedHits);
            fight.attack_from(osty, Scaling::Dynamic, |fight| {
                for _ in 0..hits {
                    let hit = fight.resolve(Effect::Attack {
                        dealer: osty,
                        target,
                        damage: Amount::Var(CardVar::OstyDamage),
                        scaling: Scaling::Dynamic,
                    });
                    fight.apply(hit);
                }
            });
            Played::Done
        }
        NecrobinderCard::BlightStrike => blight_strike(fight),
        NecrobinderCard::Misery => misery(fight),
        // `NoEscape`: its calculated Doom on the target.
        NecrobinderCard::NoEscape => {
            use super::super::super::generated::powers::Power;

            let target = fight.target();
            let doom = sts2_core::PowerAmount::Counted(fight.var(CardVar::CalculatedDoom));
            fight.apply_power(target, Power::DoomPower, doom);
            Played::Done
        }
        // `Undeath`: its block, then a clone of itself into the discard pile.
        NecrobinderCard::Undeath => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(CardVar::Block),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            fight.clone_this(Pile::Discard);
            Played::Done
        }
        // `SharedFate`: its Strength off its owner, then off the target.
        NecrobinderCard::SharedFate => {
            use super::super::super::generated::powers::Power;

            let own = fight.var(CardVar::PlayerStrengthLoss);
            fight.apply_to_actor(Power::StrengthPower, sts2_core::PowerAmount::Counted(-own));
            let theirs = fight.var(CardVar::EnemyStrengthLoss);
            let target = fight.target();
            fight.apply_power(
                target,
                Power::StrengthPower,
                sts2_core::PowerAmount::Counted(-theirs),
            );
            Played::Done
        }
        NecrobinderCard::BoneShards => bone_shards(fight),
        NecrobinderCard::NegativePulse => negative_pulse(fight),
        NecrobinderCard::Severance => severance(fight),
        // `Friendship`: its Strength off its owner, then its Energy as
        // Friendship.
        NecrobinderCard::Dredge => dredge(fight),
        // `Eidolon`: every card of the hand exhausted, one at a time, then one
        // Intangible where that came to nine or more.
        NecrobinderCard::Eidolon => {
            use super::super::super::generated::powers::Power;

            let hand = fight.cards_in(Pile::Hand);
            fight.act_on(PileAct::Exhaust, &hand);
            if hand.len() >= EIDOLON_CARDS {
                fight.apply_to_actor(Power::IntangiblePower, sts2_core::PowerAmount::Counted(1));
            }
            Played::Done
        }
        // `CaptureSpirit`: its damage to the target, unblockable and
        // unpowered, then its `Cards` of Souls into the draw pile at random.
        NecrobinderCard::CaptureSpirit => {
            use super::super::super::generated::cards::{Cards, TokenCard};

            let damage = fight.resolve(Effect::Attack {
                dealer: fight.actor(),
                target: fight.target(),
                damage: Amount::Var(CardVar::Damage),
                scaling: Scaling::Static,
            });
            fight.apply(damage);
            for _ in 0..fight.var(CardVar::Cards) {
                if fight
                    .add_at_random(Cards::Token(TokenCard::Soul), Pile::Draw)
                    .is_none()
                {
                    break;
                }
            }
            Played::Done
        }
        NecrobinderCard::Friendship => {
            use super::super::super::generated::powers::Power;

            let strength = fight.var(CardVar::StrengthPower);
            fight.apply_to_actor(
                Power::StrengthPower,
                sts2_core::PowerAmount::Counted(-strength),
            );
            let energy = fight.var(CardVar::Energy);
            fight.apply_to_actor(
                Power::FriendshipPower,
                sts2_core::PowerAmount::Counted(energy),
            );
            Played::Done
        }
        card => card_plays::necrobinder::on_play(card, fight),
    }
}

/// Where a Necrobinder card stopped inside its own play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
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
}

pub(super) fn resume_play<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        // `CardCmd.TransformTo<Soul>` of each one taken.
        Resume::SeanceTransforms => {
            use super::super::super::generated::cards::{Cards, TokenCard};

            let soul = sts2_core::CardIdent::new(Cards::Token(TokenCard::Soul));
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
    }
}

/// `Eidolon`'s `_intangibleThreshold`.
const EIDOLON_CARDS: usize = 9;

/// `Dredge`: its `Cards` from the discard pile picked into hand, as many as
/// the hand has room for.
fn dredge<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let cards: usize =
        num_traits::cast(fight.var(CardVar::Cards)).expect("a count of cards is not negative");
    let room = sts2_core::MAX_CARDS_IN_HAND.saturating_sub(fight.cards_in(Pile::Hand).len());
    match nonmax::NonMaxUsize::new(cards.min(room)) {
        Some(count) if count.get() > 0 => Played::Asking {
            pile: Pile::Discard,
            selection: Selection::Exactly(count),
            at: super::super::Resume::Card(super::Resume::Necrobinder(Resume::DredgeTakeBack)),
        },
        _ => Played::Done,
    }
}

/// `Graveblast`: its damage to the target, then one card from the discard
/// pile picked into hand.
fn graveblast<F: Fight<V>>(fight: &mut F) -> Played<V> {
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
    Played::Asking {
        pile: Pile::Discard,
        selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
        at: super::super::Resume::Card(super::Resume::Necrobinder(Resume::GraveblastTakeBack)),
    }
}

/// `DamageCmd.Attack(damage).FromOsty(Owner.Osty, this).Targeting(target)`,
/// unless `Osty.CheckMissingWithAnim` finds no Osty. Osty is the attacker, so
/// the hooks an attack raises name Osty, and the card is what they read as
/// the source.
fn osty_attack<F: Fight<V>>(fight: &mut F, damage: CardVar) -> Played<V> {
    osty_hit(fight, damage);
    Played::Done
}

/// Osty's damage to the target, where Osty stands.
fn osty_hit<F: Fight<V>>(fight: &mut F, damage: CardVar) {
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
const HANG_AT_LEAST: super::Number = 2;

/// The stack Hang keeps under, `999999999`.
const HANG_MOST: super::Number = 999_999_999;

/// Deliver a notification to one Necrobinder card in the fight.
pub(super) fn event_hook<P: sts2_core::Printed<V> + ?Sized>(
    card: NecrobinderCard,
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
        (NecrobinderCard::RightHandHand, EventHook::CardPlayedLate { energy, .. }) => {
            if i64::from(energy) >= i64::from(printed.face(CardVar::Energy)) {
                asked.push(HookEffect::AddSelfToHand {
                    only_from: Some(Pile::Discard),
                });
            }
        }
        // `Flatten.AfterAttack`, which answers an attack by its owner's Osty
        // alone. A fight here has one player, so any Osty is theirs.
        (
            NecrobinderCard::Flatten,
            EventHook::AfterAttack {
                monster: Some(Monster::Osty),
                ..
            },
        ) => asked.push(HookEffect::ModifySelf {
            change: Change::SetCost(CostPool::Energy, 0),
            lasting: Lasting::ThisTurn,
        }),
        (NecrobinderCard::Flatten, EventHook::AfterAttack { .. }) => {}
        // `Flatten.AfterCardEnteredCombat`, which answers its own entry
        // alone, and then only once Osty has attacked this turn. A fight here
        // has one player, so any Osty is theirs.
        (NecrobinderCard::Flatten, EventHook::CardEnteredCombat { at, .. }) => {
            if at.id() == id
                && history
                    .attacked_this_turn()
                    .any(|monster| monster == Some(Monster::Osty))
            {
                asked.push(HookEffect::ModifySelf {
                    change: Change::SetCost(CostPool::Energy, 0),
                    lasting: Lasting::ThisTurn,
                });
            }
        }
        // `Melancholy.AfterDeath`: any creature's death takes its `Energy` off
        // its cost for the rest of the fight. The sim prevents no death, so
        // every one counts, and a card that hears a hook is in a combat pile.
        (NecrobinderCard::Melancholy, EventHook::AfterDeath { .. }) => {
            asked.push(HookEffect::ModifySelf {
                change: Change::ChangeCost(CostPool::Energy, -printed.face(CardVar::Energy)),
                lasting: Lasting::ThisFight,
            })
        }
        // `BansheesCry.AfterCardPlayed`: its `Energy` off its cost for the
        // rest of the fight for each Ethereal card its owner plays. A fight
        // here has one player.
        (NecrobinderCard::BansheesCry, EventHook::CardPlayed { ethereal: true, .. }) => {
            asked.push(HookEffect::ModifySelf {
                change: Change::ChangeCost(CostPool::Energy, -printed.face(CardVar::Energy)),
                lasting: Lasting::ThisFight,
            })
        }
        (NecrobinderCard::BansheesCry, EventHook::CardPlayed { .. }) => {}
        // `BansheesCry.AfterCardEnteredCombat`, for its own entry alone and
        // not a clone's: its `Energy` off for the rest of the fight for each
        // Ethereal play finished this fight before it came.
        (NecrobinderCard::BansheesCry, EventHook::CardEnteredCombat { at, clone, .. }) => {
            if at.id() == id && !clone {
                let ethereal: super::Number = history
                    .finished_this_fight()
                    .filter(|finished| finished.ethereal)
                    .count()
                    .try_into()
                    .expect("a fight's plays fit a game number");
                if ethereal != 0 {
                    asked.push(HookEffect::ModifySelf {
                        change: Change::ChangeCost(
                            CostPool::Energy,
                            -ethereal * printed.face(CardVar::Energy),
                        ),
                        lasting: Lasting::ThisFight,
                    });
                }
            }
        }
        (card, hook) => unimplemented!("what {card:?} does for {hook:?}"),
    }
}
