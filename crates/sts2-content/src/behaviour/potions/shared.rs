//! The potions every character's pool draws from.

use crate::behaviour::Played;
use crate::vars::PotionVars as _;
use sts2_core::{
    Amount, CardTable as _, Change, CostPool, CreatureView as _, Effect, Fight, Lasting, Pile,
    PileAct, PowerAmount, RunStreamKinds as _, Scaling,
};

use crate::behaviour::potions::Number;
use crate::ids::{PotionIds as _, PowerIds as _};
use sts2_core::CardKind;

/// `FairyInABottle`'s heal, three tenths of the maximum.
const FAIRY_PERCENT: Number = 30;

/// `Duplicator`'s one stack of `DuplicationPower`.
const DUPLICATION: Number = 1;

/// `SneckoOil.NextEnergyCost`'s bound: `CombatEnergyCosts.NextInt(4)`.
const SNECKO_OIL_COSTS: u8 = 4;

/// What one of the shared potions does when it is drunk.
///
/// # Panics
///
/// A potion of this pool nobody has ported.
pub(super) fn on_use<'a, V: crate::Build, K: sts2_core::FightKind<V>>(
    potion: V::Potion,
    fight: &mut K::Fight<'a>,
) -> Played<V, K> {
    match potion {
        // `CreatureCmd.Heal` on the creature it saved, by
        // `Math.Max(MaxHp * 0.3m, 1m)`, which `SetCurrentHpInternal`
        // truncates.
        __c0 if __c0 == V::Potion::FAIRY_IN_A_BOTTLE => {
            let saved = fight.actor();
            let max_hp: Number = fight
                .creatures()
                .find(|creature| creature.id() == saved)
                .map(|creature| Number::from(creature.max_hp()))
                .expect("the creature a fairy saves stands in the fight");
            fight.heal(saved, (max_hp * FAIRY_PERCENT / 100).max(1));
            Played::Done
        }
        // `AnyPlayer`, and `CreatureCmd.GainBlock` with no card play. The
        // block is unpowered, which no block listener this build fields reads.
        __c0 if __c0 == V::Potion::BLOCK_POTION => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.target(),
                amount: Amount::Number(V::Potion::BLOCK_POTION_BLOCK),
                scaling: Scaling::Static,
            });
            fight.apply(block);
            Played::Done
        }
        // `AnyPlayer` again.
        __c0 if __c0 == V::Potion::GIGANTIFICATION_POTION => {
            fight.apply_power(
                fight.target(),
                V::Power::GIGANTIFICATION_POWER,
                PowerAmount::Counted(V::Potion::GIGANTIFICATION_POTION_GIGANTIFICATION_POWER),
            );
            Played::Done
        }
        // `AnyPlayer`, which a run with one player points at that player.
        __c0 if __c0 == V::Potion::DEXTERITY_POTION => {
            fight.apply_power(
                fight.target(),
                V::Power::DEXTERITY_POWER,
                PowerAmount::Counted(V::Potion::DEXTERITY_POTION_DEXTERITY_POWER),
            );
            Played::Done
        }
        // `AnyPlayer` again.
        __c0 if __c0 == V::Potion::STRENGTH_POTION => {
            fight.apply_power(
                fight.target(),
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(V::Potion::STRENGTH_POTION_STRENGTH_POWER),
            );
            Played::Done
        }
        // `AnyPlayer`, and `SpeedPotionPower` lends the Dexterity.
        __c0 if __c0 == V::Potion::SPEED_POTION => {
            fight.apply_power(
                fight.target(),
                V::Power::SPEED_POTION_POWER,
                PowerAmount::Counted(V::Potion::SPEED_POTION_DEXTERITY_POWER),
            );
            Played::Done
        }
        // `CreatureCmd.Damage` over `HittableEnemies`, unpowered and from the
        // player.
        __c0 if __c0 == V::Potion::EXPLOSIVE_AMPOULE => {
            let targets = fight.hittable_opponents();
            // One `CreatureCmd.Damage` over them all.
            fight.damage_command(|fight| {
                for target in targets {
                    let hit = fight.resolve(Effect::Attack {
                        dealer: fight.actor(),
                        target,
                        damage: Amount::Number(V::Potion::EXPLOSIVE_AMPOULE_DAMAGE),
                        scaling: Scaling::Static,
                    });
                    fight.apply(hit);
                }
            });
            Played::Done
        }
        __c0 if __c0 == V::Potion::VULNERABLE_POTION => {
            fight.apply_power(
                fight.target(),
                V::Power::VULNERABLE_POWER,
                PowerAmount::Counted(V::Potion::VULNERABLE_POTION_VULNERABLE_POWER),
            );
            Played::Done
        }
        __c0 if __c0 == V::Potion::WEAK_POTION => {
            fight.apply_power(
                fight.target(),
                V::Power::WEAK_POWER,
                PowerAmount::Counted(V::Potion::WEAK_POTION_WEAK_POWER),
            );
            Played::Done
        }
        // `AnyPlayer` again.
        __c0 if __c0 == V::Potion::LUCKY_TONIC => {
            fight.apply_power(
                fight.target(),
                V::Power::BUFFER_POWER,
                PowerAmount::Counted(V::Potion::LUCKY_TONIC_BUFFER_POWER),
            );
            Played::Done
        }
        // `AnyPlayer` again, and a run with one player walks that player's
        // hand. Every card in it that can be upgraded is.
        __c0 if __c0 == V::Potion::BLESSING_OF_THE_FORGE => {
            for card in fight.cards_in(Pile::Hand) {
                fight.modify_card(
                    card,
                    sts2_core::Change::Upgrade,
                    sts2_core::Lasting::ThisFight,
                );
            }
            Played::Done
        }
        // `CreatureCmd.Damage` on the target, unpowered and from the player.
        __c0 if __c0 == V::Potion::FIRE_POTION => {
            let target = fight.target();
            let hit = fight.resolve(Effect::Attack {
                dealer: fight.actor(),
                target,
                damage: Amount::Number(V::Potion::FIRE_POTION_DAMAGE),
                scaling: Scaling::Static,
            });
            fight.apply(hit);
            Played::Done
        }
        // `AnyPlayer` again, for each of these.
        __c0 if __c0 == V::Potion::SWIFT_POTION => {
            fight.draw(V::Potion::SWIFT_POTION_CARDS);
            Played::Done
        }
        // `AnyPlayer`: the whole hand onto the bottom of the draw pile, a
        // shuffle, then its `Cards` drawn.
        __c0 if __c0 == V::Potion::BOTTLED_POTENTIAL => {
            let hand = fight.cards_in(Pile::Hand);
            fight.act_on(sts2_core::PileAct::IntoDraw, &hand);
            fight.shuffle();
            fight.draw(V::Potion::BOTTLED_POTENTIAL_CARDS);
            Played::Done
        }
        __c0 if __c0 == V::Potion::ENERGY_POTION => {
            fight.modify_energy(V::Potion::ENERGY_POTION_ENERGY);
            Played::Done
        }
        __c0 if __c0 == V::Potion::CURE_ALL => {
            fight.modify_energy(V::Potion::CURE_ALL_ENERGY);
            fight.draw(V::Potion::CURE_ALL_CARDS);
            Played::Done
        }
        __c0 if __c0 == V::Potion::CLARITY => {
            fight.draw(V::Potion::CLARITY_CARDS);
            fight.apply_power(
                fight.target(),
                V::Power::CLARITY_POWER,
                PowerAmount::Counted(V::Potion::CLARITY_CLARITY_POWER),
            );
            Played::Done
        }
        // `FlexPotionPower` lends the Strength.
        __c0 if __c0 == V::Potion::FLEX_POTION => {
            fight.apply_power(
                fight.target(),
                V::Power::FLEX_POTION_POWER,
                PowerAmount::Counted(V::Potion::FLEX_POTION_STRENGTH_POWER),
            );
            Played::Done
        }
        __c0 if __c0 == V::Potion::STABLE_SERUM => {
            fight.apply_power(
                fight.target(),
                V::Power::RETAIN_HAND_POWER,
                PowerAmount::Counted(V::Potion::STABLE_SERUM_REPEAT),
            );
            Played::Done
        }
        __c0 if __c0 == V::Potion::HEART_OF_IRON => {
            fight.apply_power(
                fight.target(),
                V::Power::PLATING_POWER,
                PowerAmount::Counted(V::Potion::HEART_OF_IRON_PLATING_POWER),
            );
            Played::Done
        }
        __c0 if __c0 == V::Potion::LIQUID_BRONZE => {
            fight.apply_power(
                fight.target(),
                V::Power::THORNS_POWER,
                PowerAmount::Counted(V::Potion::LIQUID_BRONZE_THORNS_POWER),
            );
            Played::Done
        }
        __c0 if __c0 == V::Potion::BEETLE_JUICE => {
            fight.apply_power(
                fight.target(),
                V::Power::SHRINK_POWER,
                PowerAmount::Counted(V::Potion::BEETLE_JUICE_REPEAT),
            );
            Played::Done
        }
        __c0 if __c0 == V::Potion::POWDERED_DEMISE => {
            fight.apply_power(
                fight.target(),
                V::Power::DEMISE_POWER,
                PowerAmount::Counted(V::Potion::POWDERED_DEMISE_DEMISE),
            );
            Played::Done
        }
        // `AnyPlayer`: twice what its block stands at, unpowered.
        __c0 if __c0 == V::Potion::FORTIFIER => {
            let target = fight.target();
            let block = fight
                .creatures()
                .find(|creature| creature.id() == target)
                .map(|creature| creature.block())
                .expect("the player drinking stands in the fight");
            let gained = fight.resolve(Effect::GainBlock {
                recipient: target,
                amount: Amount::Number(block * 2),
                scaling: Scaling::Static,
            });
            fight.apply(gained);
            Played::Done
        }
        // `AnyPlayer` again, for each of these.
        __c0 if __c0 == V::Potion::FYSH_OIL => {
            fight.apply_power(
                fight.target(),
                V::Power::STRENGTH_POWER,
                PowerAmount::Counted(V::Potion::FYSH_OIL_STRENGTH_POWER),
            );
            fight.apply_power(
                fight.target(),
                V::Power::DEXTERITY_POWER,
                PowerAmount::Counted(V::Potion::FYSH_OIL_STRENGTH_POWER),
            );
            Played::Done
        }
        __c0 if __c0 == V::Potion::MAZALETHS_GIFT => {
            fight.apply_power(
                fight.target(),
                V::Power::RITUAL_POWER,
                PowerAmount::Counted(V::Potion::MAZALETHS_GIFT_RITUAL_POWER),
            );
            Played::Done
        }
        __c0 if __c0 == V::Potion::RADIANT_TINCTURE => {
            fight.modify_energy(V::Potion::RADIANT_TINCTURE_ENERGY);
            fight.apply_power(
                fight.target(),
                V::Power::RADIANCE_POWER,
                PowerAmount::Counted(V::Potion::RADIANT_TINCTURE_RADIANCE_POWER),
            );
            Played::Done
        }
        __c0 if __c0 == V::Potion::REGEN_POTION => {
            fight.apply_power(
                fight.target(),
                V::Power::REGEN_POWER,
                PowerAmount::Counted(V::Potion::REGEN_POTION_REGEN_POWER),
            );
            Played::Done
        }
        // The top three cards of the draw pile, auto-played.
        __c0 if __c0 == V::Potion::DISTILLED_CHAOS => {
            fight.auto_play_from_draw(V::Potion::DISTILLED_CHAOS_REPEAT as usize, false);
            Played::Done
        }
        __c0 if __c0 == V::Potion::DUPLICATOR => {
            fight.apply_power(
                fight.target(),
                V::Power::DUPLICATION_POWER,
                PowerAmount::Counted(DUPLICATION),
            );
            Played::Done
        }
        // `AnyPlayer`: its cards drawn, then each card in hand whose printed
        // energy cost is a number gets a cost drawn off `CombatEnergyCosts`
        // for this turn or until played, in hand order.
        __c0 if __c0 == V::Potion::SNECKO_OIL => {
            fight.draw(V::Potion::SNECKO_OIL_CARDS);
            for card in fight.cards_in(Pile::Hand) {
                if let sts2_core::Cost::Fixed(_) = fight.card_ident(card).card.cost() {
                    let cost = fight.next_energy_cost(SNECKO_OIL_COSTS);
                    fight.modify_card(
                        card,
                        Change::SetCost(CostPool::Energy, cost),
                        Lasting::ThisTurnOrUntilPlayed,
                    );
                }
            }
            Played::Done
        }
        // Its belt is the run's, which `on_use_on_run` fills.
        __c0 if __c0 == V::Potion::ENTROPIC_BREW => Played::Done,
        // `HittableEnemies`, taken once: Weak on every one, then Vulnerable
        // on every one. The game reads each amount off the other's var, and
        // both are one.
        __c0 if __c0 == V::Potion::POTION_OF_BINDING => {
            let targets = fight.hittable_opponents();
            for &target in &targets {
                fight.apply_power(
                    target,
                    V::Power::WEAK_POWER,
                    PowerAmount::Counted(V::Potion::POTION_OF_BINDING_WEAK_POWER),
                );
            }
            for &target in &targets {
                fight.apply_power(
                    target,
                    V::Power::VULNERABLE_POWER,
                    PowerAmount::Counted(V::Potion::POTION_OF_BINDING_WEAK_POWER),
                );
            }
            Played::Done
        }
        // `AnyPlayer`: `CreatureCmd.GainBlock`, unpowered, then Block Next
        // Turn at the same amount, from the drinker.
        __c0 if __c0 == V::Potion::SHIP_IN_A_BOTTLE => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.target(),
                amount: Amount::Number(V::Potion::SHIP_IN_A_BOTTLE_BLOCK),
                scaling: Scaling::Static,
            });
            fight.apply(block);
            fight.apply_power(
                fight.target(),
                V::Power::BLOCK_NEXT_TURN_POWER,
                PowerAmount::Counted(V::Potion::SHIP_IN_A_BOTTLE_BLOCK),
            );
            Played::Done
        }
        // `PowerCmd.Apply` over `HittableEnemies`, taken once.
        __c0 if __c0 == V::Potion::SHACKLING_POTION => {
            for target in fight.hittable_opponents() {
                fight.apply_power(
                    target,
                    V::Power::SHACKLING_POTION_POWER,
                    PowerAmount::Counted(V::Potion::SHACKLING_POTION_STRENGTH_POWER),
                );
            }
            Played::Done
        }
        // `GetDistinctForCombat` of one Attack, one Skill and one Power from
        // the character's pool, each off `CombatCardGeneration`. Each is
        // free this turn and lands in hand.
        __c0 if __c0 == V::Potion::OROBIC_ACID => {
            let mut made = Vec::with_capacity(3);
            for kind in [CardKind::Attack, CardKind::Skill, CardKind::Power] {
                let mut cards = crate::behaviour::character_cards(fight, kind);
                let Ok([card]) = fight.generate_cards::<1>(
                    &mut cards,
                    <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION,
                ) else {
                    unreachable!("every character's pool holds a card of each kind, {kind:?} too");
                };
                made.push(card);
            }
            for card in made {
                for at in fight.add_generated_cards(card, Pile::Hand, 1) {
                    crate::behaviour::free_this_turn(fight, at);
                }
            }
            Played::Done
        }
        // `CreatureCmd.GainMaxHp` on its target by its `MaxHpVar(5)`. A fight
        // here has one player, who drinks it.
        __c0 if __c0 == V::Potion::FRUIT_JUICE => {
            fight.gain_max_hp(fight.target(), V::Potion::FRUIT_JUICE_MAX_HP);
            Played::Done
        }
        __c0 if __c0 == V::Potion::POWER_POTION
            || __c0 == V::Potion::ATTACK_POTION
            || __c0 == V::Potion::SKILL_POTION
            || __c0 == V::Potion::COLORLESS_POTION
            || __c0 == V::Potion::LIQUID_MEMORIES
            || __c0 == V::Potion::TOUCH_OF_INSANITY
            || __c0 == V::Potion::GAMBLERS_BREW
            || __c0 == V::Potion::DROPLET_OF_PRECOGNITION =>
        {
            start::<V, K>(potion, fight)
        }
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}

/// A shared potion that stops on a question: from its use to its end,
/// yielding each question it stops on. Each step is lent the fight, and
/// after a question the answer.
#[diapause::coroutine(
    yield = crate::behaviour::paused::Ask<V>,
    resume = for<'f, 'a> crate::behaviour::paused::Lent<'f, 'a, V, K>
)]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub fn paused<V: crate::Build, K: sts2_core::FightKind<V>>(potion: V::Potion) {
    match potion {
        // `PowerPotion`, `AttackPotion`, `SkillPotion` and
        // `ColorlessPotion`: three distinct cards of the character's Powers,
        // Attacks or Skills, or of the colorless cards, off
        // `CombatCardGeneration`. The player takes one into hand, free this
        // turn, or none.
        __c0 if __c0 == V::Potion::POWER_POTION
            || __c0 == V::Potion::ATTACK_POTION
            || __c0 == V::Potion::SKILL_POTION
            || __c0 == V::Potion::COLORLESS_POTION =>
        {
            let offered: sts2_core::Offered<V> = free_card_offer::<V, _>(resumed!().fight, potion);
            yield_!(crate::behaviour::paused::Ask::Choosing {
                offered,
                skippable: true,
            });
            let crate::behaviour::paused::Lent { fight, taken } = resumed!();
            if let Some(card) = crate::behaviour::paused::chosen::<V>(taken) {
                for at in fight.add_generated_cards(card, Pile::Hand, 1) {
                    crate::behaviour::free_this_turn(&mut **fight, at);
                }
            }
        }
        // `LiquidMemories`: one card from the discard pile, free this turn,
        // into hand: `SetToFreeThisTurn`, then `CardPileCmd.Add` into hand.
        __c0 if __c0 == V::Potion::LIQUID_MEMORIES => {
            yield_!(crate::behaviour::paused::Ask::Asking {
                pile: Pile::Discard,
                selection: ONE,
            });
            let crate::behaviour::paused::Lent { fight, taken } = resumed!();
            let picked = crate::behaviour::paused::selected::<V>(taken);
            for &at in picked.as_slice() {
                crate::behaviour::free_this_turn(&mut **fight, at);
            }
            fight.act_on(PileAct::IntoHand, picked.as_slice());
        }
        // `TouchOfInsanity`: one card of the hand that costs energy or stars,
        // with its own modifiers or with every one, then
        // `SetToFreeThisCombat`: both costs at nothing for the rest of the
        // fight.
        __c0 if __c0 == V::Potion::TOUCH_OF_INSANITY => {
            yield_!(crate::behaviour::paused::Ask::AskingAmong {
                pile: Pile::Hand,
                filter: crate::behaviour::CardFilter::CostsEnergyOrStars,
                selection: ONE,
            });
            let crate::behaviour::paused::Lent { fight, taken } = resumed!();
            for &card in crate::behaviour::paused::selected::<V>(taken).as_slice() {
                for pool in [CostPool::Energy, CostPool::Star] {
                    fight.modify_card(card, Change::SetCost(pool, 0), Lasting::ThisFight);
                }
            }
        }
        // `GamblersBrew`: any number of cards from hand, then
        // `CardCmd.DiscardAndDraw` for as many as were picked.
        __c0 if __c0 == V::Potion::GAMBLERS_BREW => {
            yield_!(crate::behaviour::paused::Ask::Asking {
                pile: Pile::Hand,
                selection: sts2_core::Selection::UpTo(
                    nonmax::NonMaxUsize::new(crate::behaviour::potions::ANY_NUMBER)
                        .expect("a count below the maximum"),
                ),
            });
            let crate::behaviour::paused::Lent { fight, taken } = resumed!();
            let picked = crate::behaviour::paused::selected::<V>(taken);
            let count = Number::try_from(picked.as_slice().len())
                .expect("a hand's size fits a game number");
            fight.discard_and_draw(picked.as_slice(), count);
        }
        // `DropletOfPrecognition`: one card from the draw pile into hand.
        __c0 if __c0 == V::Potion::DROPLET_OF_PRECOGNITION => {
            yield_!(crate::behaviour::paused::Ask::Asking {
                pile: Pile::Draw,
                selection: ONE,
            });
            let crate::behaviour::paused::Lent { fight, taken } = resumed!();
            fight.act_on(
                PileAct::IntoHand,
                crate::behaviour::paused::selected::<V>(taken).as_slice(),
            );
        }
        potion => unreachable!("{potion:?} never stops on a question"),
    }
}

crate::behaviour::paused::paused_play!(V::Potion, crate::behaviour::potions::Resume::Shared);

/// One card, as a pick asks for it.
const ONE: sts2_core::Selection = sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE);

/// The offer of `PowerPotion`, `AttackPotion`, `SkillPotion` or
/// `ColorlessPotion`: three distinct cards off `CombatCardGeneration`, of the
/// character's Powers, Attacks or Skills, or of the colorless cards.
fn free_card_offer<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    potion: V::Potion,
) -> sts2_core::Offered<V> {
    let mut cards = match potion {
        __c0 if __c0 == V::Potion::POWER_POTION => {
            crate::behaviour::character_cards(fight, CardKind::Power)
        }
        __c0 if __c0 == V::Potion::ATTACK_POTION => {
            crate::behaviour::character_cards(fight, CardKind::Attack)
        }
        __c0 if __c0 == V::Potion::SKILL_POTION => {
            crate::behaviour::character_cards(fight, CardKind::Skill)
        }
        _ => V::combat_colorless_cards(fight.multiplayer()).to_vec(),
    };
    let offered: [_; sts2_core::CHOICES] = fight
        .generate_cards(
            &mut cards,
            <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION,
        )
        .unwrap_or_else(|_| unimplemented!("a pool of {} cards offering three", cards.len()));
    offered.into_iter().collect()
}
