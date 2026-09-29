//! The potions every character's pool draws from.

use sts2_core::{
    Amount, CardTable as _, Change, CostPool, CreatureView as _, Effect, Fight, Lasting, Pile,
    PileAct, Played, PowerAmount, RunStreamKinds as _, Scaling,
};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{CardType, Cards, combat_colorless_cards};
use super::super::super::generated::potions::Potion;
use super::super::super::generated::powers::Power;
use super::Number;

/// `BeetleJuice`'s `RepeatVar(4)`, the hits Shrink lasts.
const BEETLE_JUICE: Number = 4;
/// `BlockPotion`'s `BlockVar(12, ValueProp.Unpowered)`.
const BLOCK: Number = 12;
/// `LuckyTonic`'s `PowerVar<BufferPower>(1)`.
const BUFFER: Number = 1;
/// `Clarity`'s `CardsVar(1)`.
const CLARITY_CARDS: Number = 1;
/// `Clarity`'s `PowerVar<ClarityPower>(3)`.
const CLARITY: Number = 3;
/// `CureAll`'s `CardsVar(2)`.
const CURE_ALL_CARDS: Number = 2;
/// `CureAll`'s `EnergyVar(1)`.
const CURE_ALL_ENERGY: Number = 1;
/// `DexterityPotion`'s `PowerVar<DexterityPower>(2)`.
const DEXTERITY: Number = 2;
/// `BottledPotential`'s `CardsVar(5)`.
const BOTTLED_POTENTIAL: Number = 5;

/// `EnergyPotion`'s `EnergyVar(2)`.
const ENERGY: Number = 2;

/// `ExplosiveAmpoule`'s `DamageVar(10, ValueProp.Unpowered)`.
const EXPLOSIVE: Number = 10;
/// `FirePotion`'s `DamageVar(20, ValueProp.Unpowered)`.
const FIRE: Number = 20;
/// `FlexPotion`'s `PowerVar<StrengthPower>(5)`, lent until the turn ends.
const FLEX: Number = 5;
/// `HeartOfIron`'s `PowerVar<PlatingPower>(7)`.
const HEART_OF_IRON: Number = 7;
/// `LiquidBronze`'s `PowerVar<ThornsPower>(3)`.
const LIQUID_BRONZE: Number = 3;
/// `PowderedDemise`'s `Demise` var.
const POWDERED_DEMISE: Number = 9;
/// `GigantificationPotion`'s `PowerVar<GigantificationPower>(1)`.
const GIGANTIFICATION: Number = 1;
/// `SpeedPotion`'s `PowerVar<DexterityPower>(5)`, lent until the turn ends.
const SPEED: Number = 5;
/// `StableSerum`'s `RepeatVar(2)`, the turns the hand is kept.
const STABLE_SERUM: Number = 2;

/// `StrengthPotion`'s `PowerVar<StrengthPower>(2)`.
const STRENGTH: Number = 2;
/// `SwiftPotion`'s `CardsVar(3)`.
const SWIFT: Number = 3;
/// `DistilledChaos`'s `RepeatVar(3)`.
const DISTILLED_CHAOS: usize = 3;
/// `Duplicator`'s one stack of `DuplicationPower`.
const DUPLICATION: Number = 1;
/// `FyshOil`'s `PowerVar<StrengthPower>(1)` and `PowerVar<DexterityPower>(1)`.
const FYSH_OIL: Number = 1;

/// `ShipInABottle`'s `BlockVar(10, ValueProp.Unpowered)`, which is also the
/// Block Next Turn it applies.
const SHIP_IN_A_BOTTLE: Number = 10;

/// `ShacklingPotion`'s `PowerVar<StrengthPower>(7)`.
const SHACKLING: Number = 7;
/// `MazalethsGift`'s `PowerVar<RitualPower>(1)`.
const MAZALETHS_GIFT: Number = 1;
/// `PotionOfBinding`'s `PowerVar<WeakPower>(1)` and
/// `PowerVar<VulnerablePower>(1)`.
const BINDING: Number = 1;
/// `RadiantTincture`'s `EnergyVar(1)`.
const RADIANT_ENERGY: Number = 1;
/// `RadiantTincture`'s `PowerVar<RadiancePower>(3)`.
const RADIANCE: Number = 3;
/// `RegenPotion`'s `PowerVar<RegenPower>(5)`.
const REGEN: Number = 5;
/// `SneckoOil`'s `CardsVar(7)`.
const SNECKO_OIL_CARDS: Number = 7;
/// `SneckoOil.NextEnergyCost`'s bound: `CombatEnergyCosts.NextInt(4)`.
const SNECKO_OIL_COSTS: u8 = 4;

/// `VulnerablePotion`'s `PowerVar<VulnerablePower>(3)`.
const VULNERABLE: Number = 3;
/// `WeakPotion`'s `PowerVar<WeakPower>(3)`.
const WEAK: Number = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    /// A potion offering three cards, one of which goes into hand free this
    /// turn.
    FreeCardChoice,
    /// `LiquidMemories`, after the card picked from the discard pile.
    LiquidMemories,
    /// `DropletOfPrecognition`, after the card picked from the draw pile.
    DropletOfPrecognition,
    /// `GamblersBrew`, after the cards picked from hand to discard.
    GamblersBrew,
    /// `TouchOfInsanity`, after the card picked to make free.
    TouchOfInsanity,
}

/// What one of the shared potions does when it is drunk.
///
/// # Panics
///
/// A potion of this pool nobody has ported.
pub(super) fn on_use<F: Fight<V>>(potion: Potion, fight: &mut F) -> Played<V> {
    match potion {
        // `AnyPlayer`, and `CreatureCmd.GainBlock` with no card play. The
        // block is unpowered, which no block listener this build fields reads.
        Potion::BlockPotion => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(BLOCK),
                scaling: Scaling::Static,
            });
            fight.apply(block);
            Played::Done
        }
        // `AnyPlayer` again.
        Potion::GigantificationPotion => {
            fight.apply_to_actor(
                Power::GigantificationPower,
                PowerAmount::Counted(GIGANTIFICATION),
            );
            Played::Done
        }
        // `AnyPlayer`, which a run with one player points at that player.
        Potion::DexterityPotion => {
            fight.apply_to_actor(Power::DexterityPower, PowerAmount::Counted(DEXTERITY));
            Played::Done
        }
        // `AnyPlayer` again.
        Potion::StrengthPotion => {
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(STRENGTH));
            Played::Done
        }
        // `AnyPlayer`, and `SpeedPotionPower` lends the Dexterity.
        Potion::SpeedPotion => {
            fight.apply_to_actor(Power::SpeedPotionPower, PowerAmount::Counted(SPEED));
            Played::Done
        }
        // `CreatureCmd.Damage` over `HittableEnemies`, unpowered and from the
        // player.
        Potion::ExplosiveAmpoule => {
            for target in fight.hittable_opponents() {
                let hit = fight.resolve(Effect::Attack {
                    dealer: fight.actor(),
                    target,
                    damage: Amount::Number(EXPLOSIVE),
                    scaling: Scaling::Static,
                });
                fight.apply(hit);
            }
            Played::Done
        }
        Potion::VulnerablePotion => {
            fight.apply_power(
                fight.target(),
                Power::VulnerablePower,
                PowerAmount::Counted(VULNERABLE),
            );
            Played::Done
        }
        Potion::WeakPotion => {
            fight.apply_power(fight.target(), Power::WeakPower, PowerAmount::Counted(WEAK));
            Played::Done
        }
        // `AnyPlayer` again.
        Potion::LuckyTonic => {
            fight.apply_to_actor(Power::BufferPower, PowerAmount::Counted(BUFFER));
            Played::Done
        }
        // `AnyPlayer` again, and a run with one player walks that player's
        // hand. Every card in it that can be upgraded is.
        Potion::BlessingOfTheForge => {
            for card in fight.cards_in(Pile::Hand) {
                fight.modify_card(
                    card,
                    sts2_core::Change::Upgrade,
                    sts2_core::Lasting::ThisFight,
                );
            }
            Played::Done
        }
        Potion::PowerPotion => {
            free_card_of(fight, super::super::character_cards(fight, CardType::Power))
        }
        Potion::AttackPotion => free_card_of(
            fight,
            super::super::character_cards(fight, CardType::Attack),
        ),
        Potion::SkillPotion => {
            free_card_of(fight, super::super::character_cards(fight, CardType::Skill))
        }
        Potion::ColorlessPotion => free_card_of(fight, combat_colorless_cards().to_vec()),
        // `CreatureCmd.Damage` on the target, unpowered and from the player.
        Potion::FirePotion => {
            let target = fight.target();
            let hit = fight.resolve(Effect::Attack {
                dealer: fight.actor(),
                target,
                damage: Amount::Number(FIRE),
                scaling: Scaling::Static,
            });
            fight.apply(hit);
            Played::Done
        }
        // `AnyPlayer` again, for each of these.
        Potion::SwiftPotion => {
            fight.draw(SWIFT);
            Played::Done
        }
        // `AnyPlayer`: the whole hand onto the bottom of the draw pile, a
        // shuffle, then its `Cards` drawn.
        Potion::BottledPotential => {
            let hand = fight.cards_in(Pile::Hand);
            fight.act_on(sts2_core::PileAct::IntoDraw, &hand);
            fight.shuffle();
            fight.draw(BOTTLED_POTENTIAL);
            Played::Done
        }
        Potion::EnergyPotion => {
            fight.modify_energy(ENERGY);
            Played::Done
        }
        Potion::CureAll => {
            fight.modify_energy(CURE_ALL_ENERGY);
            fight.draw(CURE_ALL_CARDS);
            Played::Done
        }
        Potion::Clarity => {
            fight.draw(CLARITY_CARDS);
            fight.apply_to_actor(Power::ClarityPower, PowerAmount::Counted(CLARITY));
            Played::Done
        }
        // `FlexPotionPower` lends the Strength.
        Potion::FlexPotion => {
            fight.apply_to_actor(Power::FlexPotionPower, PowerAmount::Counted(FLEX));
            Played::Done
        }
        Potion::StableSerum => {
            fight.apply_to_actor(Power::RetainHandPower, PowerAmount::Counted(STABLE_SERUM));
            Played::Done
        }
        Potion::HeartOfIron => {
            fight.apply_to_actor(Power::PlatingPower, PowerAmount::Counted(HEART_OF_IRON));
            Played::Done
        }
        Potion::LiquidBronze => {
            fight.apply_to_actor(Power::ThornsPower, PowerAmount::Counted(LIQUID_BRONZE));
            Played::Done
        }
        Potion::BeetleJuice => {
            fight.apply_power(
                fight.target(),
                Power::ShrinkPower,
                PowerAmount::Counted(BEETLE_JUICE),
            );
            Played::Done
        }
        Potion::PowderedDemise => {
            fight.apply_power(
                fight.target(),
                Power::DemisePower,
                PowerAmount::Counted(POWDERED_DEMISE),
            );
            Played::Done
        }
        // `AnyPlayer`: twice what its block stands at, unpowered.
        Potion::Fortifier => {
            let actor = fight.actor();
            let block = fight
                .creatures()
                .find(|creature| creature.id() == actor)
                .map(|creature| creature.block())
                .expect("the player drinking stands in the fight");
            let gained = fight.resolve(Effect::GainBlock {
                recipient: actor,
                amount: Amount::Number(block * 2),
                scaling: Scaling::Static,
            });
            fight.apply(gained);
            Played::Done
        }
        // `AnyPlayer` again, for each of these.
        Potion::FyshOil => {
            fight.apply_to_actor(Power::StrengthPower, PowerAmount::Counted(FYSH_OIL));
            fight.apply_to_actor(Power::DexterityPower, PowerAmount::Counted(FYSH_OIL));
            Played::Done
        }
        Potion::MazalethsGift => {
            fight.apply_to_actor(Power::RitualPower, PowerAmount::Counted(MAZALETHS_GIFT));
            Played::Done
        }
        Potion::RadiantTincture => {
            fight.modify_energy(RADIANT_ENERGY);
            fight.apply_to_actor(Power::RadiancePower, PowerAmount::Counted(RADIANCE));
            Played::Done
        }
        Potion::RegenPotion => {
            fight.apply_to_actor(Power::RegenPower, PowerAmount::Counted(REGEN));
            Played::Done
        }
        // The top three cards of the draw pile, auto-played.
        Potion::DistilledChaos => {
            fight.auto_play_from_draw(DISTILLED_CHAOS, false);
            Played::Done
        }
        Potion::Duplicator => {
            fight.apply_to_actor(Power::DuplicationPower, PowerAmount::Counted(DUPLICATION));
            Played::Done
        }
        // `AnyPlayer`: its cards drawn, then each card in hand whose printed
        // energy cost is a number gets a cost drawn off `CombatEnergyCosts`
        // for this turn or until played, in hand order.
        Potion::SneckoOil => {
            fight.draw(SNECKO_OIL_CARDS);
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
        Potion::EntropicBrew => Played::Done,
        // One card from the discard pile, free this turn, into hand.
        Potion::LiquidMemories => Played::Asking {
            pile: Pile::Discard,
            selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
            at: super::super::Resume::Potion(super::Resume::Shared(Resume::LiquidMemories)),
        },
        // Any number of cards from hand, discarded, and as many drawn.
        // `Self`: one card of the hand that costs energy or stars, with its
        // own modifiers or with every one.
        Potion::TouchOfInsanity => {
            let costs = |cost| matches!(cost, sts2_core::Cost::Fixed(cost) if cost.get() > 0);
            let cards = fight
                .cards_in(Pile::Hand)
                .into_iter()
                .filter(|&card| {
                    let (energy, stars) = fight.card_costs(card, false);
                    let (all_energy, all_stars) = fight.card_costs(card, true);
                    costs(energy) || costs(stars) || costs(all_energy) || costs(all_stars)
                })
                .collect();
            Played::AskingAmong {
                cards,
                selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
                at: super::super::Resume::Potion(super::Resume::Shared(Resume::TouchOfInsanity)),
            }
        }
        Potion::GamblersBrew => Played::Asking {
            pile: Pile::Hand,
            selection: sts2_core::Selection::UpTo(
                nonmax::NonMaxUsize::new(super::ANY_NUMBER).expect("a count below the maximum"),
            ),
            at: super::super::Resume::Potion(super::Resume::Shared(Resume::GamblersBrew)),
        },
        // One card from the draw pile into hand.
        Potion::DropletOfPrecognition => Played::Asking {
            pile: Pile::Draw,
            selection: sts2_core::Selection::Exactly(nonmax::NonMaxUsize::ONE),
            at: super::super::Resume::Potion(super::Resume::Shared(Resume::DropletOfPrecognition)),
        },
        // `HittableEnemies`, taken once: Weak on every one, then Vulnerable
        // on every one. The game reads each amount off the other's var, and
        // both are one.
        Potion::PotionOfBinding => {
            let targets = fight.hittable_opponents();
            for &target in &targets {
                fight.apply_power(target, Power::WeakPower, PowerAmount::Counted(BINDING));
            }
            for &target in &targets {
                fight.apply_power(
                    target,
                    Power::VulnerablePower,
                    PowerAmount::Counted(BINDING),
                );
            }
            Played::Done
        }
        // `AnyPlayer`: `CreatureCmd.GainBlock`, unpowered, then Block Next
        // Turn at the same amount, from the drinker.
        Potion::ShipInABottle => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Number(SHIP_IN_A_BOTTLE),
                scaling: Scaling::Static,
            });
            fight.apply(block);
            fight.apply_to_actor(
                Power::BlockNextTurnPower,
                PowerAmount::Counted(SHIP_IN_A_BOTTLE),
            );
            Played::Done
        }
        // `PowerCmd.Apply` over `HittableEnemies`, taken once.
        Potion::ShacklingPotion => {
            for target in fight.hittable_opponents() {
                fight.apply_power(
                    target,
                    Power::ShacklingPotionPower,
                    PowerAmount::Counted(SHACKLING),
                );
            }
            Played::Done
        }
        // `GetDistinctForCombat` of one Attack, one Skill and one Power from
        // the character's pool, each off `CombatCardGeneration`. Each is
        // free this turn and lands in hand.
        Potion::OrobicAcid => {
            let mut made = Vec::with_capacity(3);
            for kind in [CardType::Attack, CardType::Skill, CardType::Power] {
                let mut cards = super::super::character_cards(fight, kind);
                let Ok([card]) = fight.generate_cards::<1>(
                    &mut cards,
                    <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION,
                ) else {
                    unimplemented!("a character pool with no {kind:?}");
                };
                made.push(card);
            }
            for card in made {
                for at in fight.add_generated_cards(card, Pile::Hand, 1) {
                    super::super::free_this_turn(fight, at);
                }
            }
            Played::Done
        }
        // `CreatureCmd.GainMaxHp` on its target by its `MaxHpVar(5)`. A fight
        // here has one player, who drinks it.
        Potion::FruitJuice => {
            fight.gain_max_hp(fight.actor(), super::FRUIT_JUICE);
            Played::Done
        }
        potion => unimplemented!("what {potion:?} does when it is drunk"),
    }
}

/// `PowerPotion`, `AttackPotion`, `SkillPotion` and `ColorlessPotion`: three
/// distinct cards of `cards`, off `CombatCardGeneration`. The player takes one
/// into hand, free this turn, or none.
fn free_card_of<F: Fight<V>>(fight: &mut F, mut cards: Vec<Cards>) -> Played<V> {
    let offered: [_; sts2_core::CHOICES] = fight
        .generate_cards(
            &mut cards,
            <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION,
        )
        .unwrap_or_else(|_| unimplemented!("a pool of {} cards offering three", cards.len()));
    Played::Choosing {
        offered: offered.into_iter().collect(),
        skippable: true,
        at: super::super::Resume::Potion(super::Resume::Shared(Resume::FreeCardChoice)),
    }
}

pub(super) fn resume<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    answer: sts2_core::Taken<V>,
) -> Played<V> {
    match (at, answer) {
        (Resume::FreeCardChoice, sts2_core::Taken::Chosen(chosen)) => {
            if let Some(card) = chosen {
                for at in fight.add_generated_cards(card, Pile::Hand, 1) {
                    super::super::free_this_turn(fight, at);
                }
            }
            Played::Done
        }
        // `SetToFreeThisTurn`, then `CardPileCmd.Add` into hand.
        (Resume::LiquidMemories, sts2_core::Taken::Selected(picked)) => {
            for &at in picked.as_slice() {
                super::super::free_this_turn(fight, at);
            }
            fight.act_on(PileAct::IntoHand, picked.as_slice());
            Played::Done
        }
        // `SetToFreeThisCombat`: both costs at nothing for the rest of the
        // fight.
        (Resume::TouchOfInsanity, sts2_core::Taken::Selected(picked)) => {
            for &card in picked.as_slice() {
                for pool in [sts2_core::CostPool::Energy, sts2_core::CostPool::Star] {
                    fight.modify_card(
                        card,
                        sts2_core::Change::SetCost(pool, 0),
                        sts2_core::Lasting::ThisFight,
                    );
                }
            }
            Played::Done
        }
        // `CardCmd.DiscardAndDraw` for as many as were picked.
        (Resume::GamblersBrew, sts2_core::Taken::Selected(picked)) => {
            let count = Number::try_from(picked.as_slice().len())
                .expect("a hand's size fits a game number");
            fight.discard_and_draw(picked.as_slice(), count);
            Played::Done
        }
        (Resume::DropletOfPrecognition, sts2_core::Taken::Selected(picked)) => {
            fight.act_on(PileAct::IntoHand, picked.as_slice());
            Played::Done
        }
        (at, answer) => unimplemented!("{at:?} resumed with {answer:?}"),
    }
}
