//! The event cards the generator leaves to a hand port.

use sts2_core::{
    Amount, Change, CostPool, CreatureView as _, Effect, Fight, Lasting, Pile, PileAct, Played,
    PowerAmount, Rules as _, Scaling, Selection,
};

use super::super::super::generated::cards::Cards;
use super::super::super::generated::cards::{CardType, CardVar, EventCard};
use super::super::super::generated::powers::Power;
use super::V;

/// Where an event card stopped inside its own play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    /// `NeowsFury`, after its owner picked the cards to take back.
    NeowsFuryPicks,
    /// `Wish`, after its owner picked the card to take.
    WishPick,
    /// `BrightestFlame`, after a question inside its draw was answered.
    BrightestFlameDrew,
    /// `DualWield`, after its owner picked the card to copy.
    DualWieldPick,
}

pub(super) fn on_play<F: Fight<V>>(card: EventCard, fight: &mut F) -> Played<V> {
    match card {
        EventCard::NeowsFury => neows_fury(fight),
        // `Enlightenment`: every card in hand costs at most one, this turn or
        // until played, or for the fight once upgraded.
        EventCard::Enlightenment => {
            let lasting = match fight.upgraded() {
                true => Lasting::ThisFight,
                false => Lasting::ThisTurnOrUntilPlayed,
            };
            for card in fight.cards_in(Pile::Hand) {
                fight.modify_card(card, Change::LowerCost(CostPool::Energy, 1), lasting);
            }
            Played::Done
        }
        // `ToricToughness`: its block, then its `Turns` of Toric Toughness on
        // its owner, set to give the block it gained.
        EventCard::ToricToughness => {
            use super::super::super::generated::powers::Power;
            use sts2_core::EffectResolution as _;

            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(CardVar::Block),
                scaling: Scaling::Dynamic,
            });
            let gained = block.amount();
            fight.apply(block);
            let turns = sts2_core::PowerAmount::Counted(fight.var(CardVar::Turns));
            fight.apply_to_actor(Power::ToricToughnessPower, turns);
            fight.note_on_power(fight.actor(), Power::ToricToughnessPower, gained);
            Played::Done
        }
        EventCard::Metamorphosis => metamorphosis(fight),
        EventCard::Whistle => whistle(fight),
        EventCard::Apotheosis => apotheosis(fight),
        EventCard::Maul => maul(fight),
        EventCard::MadScienceSapping
        | EventCard::MadScienceViolence
        | EventCard::MadScienceChoking
        | EventCard::MadScienceEnergized
        | EventCard::MadScienceWisdom
        | EventCard::MadScienceChaos
        | EventCard::MadScienceExpertise
        | EventCard::MadScienceCurious
        | EventCard::MadScienceImprovement => mad_science(card, fight),
        // `Wish`: one card of the draw pile into hand.
        EventCard::Wish => Played::Asking {
            pile: Pile::Draw,
            selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
            at: super::super::Resume::Card(super::Resume::Event(Resume::WishPick)),
        },
        // `BrightestFlame`: its Energy, its Cards drawn, then its `MaxHp` off
        // its owner's maximum.
        EventCard::BrightestFlame => {
            fight.modify_energy(fight.var(CardVar::Energy));
            fight.draw(fight.var(CardVar::Cards));
            // A shuffle inside the draw can ask, and the rest waits for it.
            if fight.waiting() {
                return Played::Waiting {
                    at: super::super::Resume::Card(super::Resume::Event(
                        Resume::BrightestFlameDrew,
                    )),
                };
            }
            fight.lose_max_hp(fight.actor(), fight.var(CardVar::MaxHp));
            Played::Done
        }
        // Its damage to the target. The Byrdpip pet only animates the hit.
        EventCard::ByrdSwoop => {
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
            Played::Done
        }
        // `Clash`: its damage to the target. Its `IsPlayable` asks for a
        // hand of Attacks alone.
        EventCard::Clash => {
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
            Played::Done
        }
        // `Distraction`: a Skill of the character's pool, free this turn,
        // into the hand.
        EventCard::Distraction => {
            super::super::free_card_into_hand(
                fight,
                |card| V::card_kind(card) == Some(sts2_core::CardKind::Skill),
                true,
            );
            Played::Done
        }
        // `DualWield`: one Attack or Power of the hand, then its `Cards` of
        // clones of it into the hand.
        EventCard::DualWield => {
            let cards = fight
                .cards_in(Pile::Hand)
                .into_iter()
                .filter(|&at| {
                    matches!(
                        V::card_kind(fight.card_ident(at).card),
                        Some(sts2_core::CardKind::Attack | sts2_core::CardKind::Power)
                    )
                })
                .collect();
            Played::AskingAmong {
                cards,
                selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
                at: super::super::Resume::Card(super::Resume::Event(Resume::DualWieldPick)),
            }
        }
        // `Entrench`: as much block again as its owner holds, unpowered.
        EventCard::Entrench => {
            let actor = fight.actor();
            let held = fight
                .creatures()
                .find(|creature| creature.id() == actor)
                .map_or(0, |creature| creature.block());
            let block = fight.resolve(Effect::GainBlock {
                recipient: actor,
                amount: Amount::Number(held),
                scaling: Scaling::Static,
            });
            fight.apply(block);
            Played::Done
        }
        // `FeedingFrenzy`: its `StrengthPower` as Feeding Frenzy on its owner.
        EventCard::FeedingFrenzy => {
            let amount = fight.var(CardVar::StrengthPower);
            fight.apply_to_actor(Power::FeedingFrenzyPower, PowerAmount::Counted(amount));
            Played::Done
        }
        // `Stack`: as much block as its `CalculatedBlock`, one more for each
        // card in the discard pile.
        EventCard::Stack => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(CardVar::CalculatedBlock),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            Played::Done
        }
        card => super::generated_from(Cards::Event(card), fight, 0),
    }
}

pub(super) fn resume_play<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        // `CardPileCmd.Add(picked, PileType.Hand)`.
        Resume::NeowsFuryPicks | Resume::WishPick => {
            fight.act_on(PileAct::IntoHand, chosen.as_slice());
            Played::Done
        }
        Resume::BrightestFlameDrew => unreachable!("{at:?} waits on a question, not a pick"),
        // `CreateClone` of the card picked, its `Cards` times, each through
        // `AddGeneratedCardToCombat` into the hand.
        Resume::DualWieldPick => {
            if let Some(picked) = chosen.one_or_none() {
                for _ in 0..fight.var(CardVar::Cards) {
                    fight.clone_card(picked, Pile::Hand);
                }
            }
            Played::Done
        }
    }
}

/// `NeowsFury`: its damage to the target, then up to its `Cards` from the
/// discard pile into hand, as many as the hand has room for.
fn neows_fury<F: Fight<V>>(fight: &mut F) -> Played<V> {
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
    let cards: usize =
        num_traits::cast(fight.var(CardVar::Cards)).expect("a count of cards is not negative");
    let room = sts2_core::MAX_CARDS_IN_HAND.saturating_sub(fight.cards_in(Pile::Hand).len());
    match nonmax::NonMaxUsize::new(cards.min(room)) {
        Some(count) if count.get() > 0 => Played::Asking {
            pile: Pile::Discard,
            selection: Selection::UpTo(count),
            at: super::super::Resume::Card(super::Resume::Event(Resume::NeowsFuryPicks)),
        },
        _ => Played::Done,
    }
}

/// `Metamorphosis`: its `Cards` attacks from the character's pool, each
/// `NextItem` off `CombatCardGeneration`, free this fight and into the draw
/// pile at a random place.
fn metamorphosis<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let attacks = super::super::character_cards(fight, CardType::Attack);
    for _ in 0..fight.var(CardVar::Cards) {
        let Some(card) = fight.generate_card(&attacks) else {
            unimplemented!("a character pool with no attack")
        };
        if let Some(at) = fight.add_at_random(card, Pile::Draw) {
            for pool in [CostPool::Energy, CostPool::Star] {
                fight.modify_card(
                    at,
                    Change::SetCost(pool, num_traits::zero()),
                    Lasting::ThisFight,
                );
            }
        }
    }
    Played::Done
}

/// Its damage to `target`, `hits` times, as one attack.
fn hit<F: Fight<V>>(fight: &mut F, target: sts2_core::CombatId, hits: usize) {
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
}

/// `MadScience`: what its type does, then its rider.
///
/// An attack hits the target once, or `ViolenceHits` times with Violence,
/// each hit its own command. A skill gains its block. A power applies what
/// its rider names to its owner. Sapping and Choking then debuff the target,
/// Energized gives energy, Wisdom draws, and Chaos adds a card of the
/// character's pool, free this turn.
fn mad_science<F: Fight<V>>(card: EventCard, fight: &mut F) -> Played<V> {
    use super::super::super::generated::powers::Power;
    use sts2_core::PowerAmount;

    let counted = |fight: &F, key| PowerAmount::Counted(fight.var(key));
    match card {
        EventCard::MadScienceSapping
        | EventCard::MadScienceViolence
        | EventCard::MadScienceChoking => {
            let target = fight.target();
            let hits = match card {
                EventCard::MadScienceViolence => {
                    num_traits::cast(fight.var(CardVar::ViolenceHits)).expect("a few hits")
                }
                _ => 1,
            };
            for _ in 0..hits {
                hit(fight, target, 1);
            }
            match card {
                EventCard::MadScienceSapping => {
                    let weak = counted(fight, CardVar::SappingWeak);
                    fight.apply_power(target, Power::WeakPower, weak);
                    let vulnerable = counted(fight, CardVar::SappingVulnerable);
                    fight.apply_power(target, Power::VulnerablePower, vulnerable);
                }
                EventCard::MadScienceChoking => {
                    let strangle = counted(fight, CardVar::ChokingDamage);
                    fight.apply_power(target, Power::StranglePower, strangle);
                }
                _ => {}
            }
        }
        EventCard::MadScienceEnergized
        | EventCard::MadScienceWisdom
        | EventCard::MadScienceChaos => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(CardVar::Block),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            match card {
                EventCard::MadScienceEnergized => {
                    fight.modify_energy(fight.var(CardVar::EnergizedEnergy))
                }
                EventCard::MadScienceWisdom => {
                    fight.draw(fight.var(CardVar::WisdomCards));
                }
                _ => super::super::free_card_into_hand(fight, |_| true, false),
            }
        }
        EventCard::MadScienceExpertise => {
            let strength = counted(fight, CardVar::ExpertiseStrength);
            fight.apply_to_actor(Power::StrengthPower, strength);
            let dexterity = counted(fight, CardVar::ExpertiseDexterity);
            fight.apply_to_actor(Power::DexterityPower, dexterity);
        }
        EventCard::MadScienceCurious => {
            let curious = counted(fight, CardVar::CuriousReduction);
            fight.apply_to_actor(Power::CuriousPower, curious);
        }
        EventCard::MadScienceImprovement => {
            fight.apply_to_actor(Power::ImprovementPower, PowerAmount::Counted(1));
        }
        card => unreachable!("{card:?} is not a Mad Science"),
    }
    Played::Done
}

/// `Whistle`: its damage to the target, then `CreatureCmd.Stun` on it with
/// no move named after.
fn whistle<F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    hit(fight, target, 1);
    fight.stun(
        target,
        &super::super::powers::STUNNED,
        sts2_core::StunResume::Interrupted,
    );
    Played::Done
}

/// `Maul`: its damage to the target twice, then its `Increase` onto the
/// damage of every Maul its owner holds in the fight, itself included.
fn maul<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use super::super::super::generated::cards::Cards;

    let target = fight.target();
    hit(fight, target, 2);
    let raise = Change::Own(super::CardWrite::BuffFromPlay(fight.var(CardVar::Increase)));
    // `AllCards`: the hand, the draw, discard and exhaust piles, then the
    // play pile, which holds the Maul being played.
    for pile in [Pile::Hand, Pile::Draw, Pile::Discard, Pile::Exhaust] {
        for card in fight.cards_in(pile) {
            if fight.card_ident(card).card == Cards::Event(EventCard::Maul) {
                fight.modify_card(card, raise, Lasting::ThisFight);
            }
        }
    }
    fight.modify_this(raise, Lasting::ThisFight);
    Played::Done
}

/// `Apotheosis`: every other card its owner holds in the fight upgraded,
/// where it can be.
fn apotheosis<F: Fight<V>>(fight: &mut F) -> Played<V> {
    // `AllCards` less itself: the hand, the draw, discard, exhaust and play
    // piles, where a card that auto-played it stands. `CardCmd.Upgrade`
    // leaves a card at its highest alone.
    let this = fight.playing();
    for pile in [
        Pile::Hand,
        Pile::Draw,
        Pile::Discard,
        Pile::Exhaust,
        Pile::Play,
    ] {
        for card in fight.cards_in(pile) {
            if card.id() != this {
                fight.modify_card(card, Change::Upgrade, Lasting::ThisFight);
            }
        }
    }
    Played::Done
}

/// The rest of an event card's play that stood still on a question its own
/// command raised.
pub(super) fn resume_waited<F: Fight<V>>(fight: &mut F, at: Resume) -> Played<V> {
    match at {
        Resume::BrightestFlameDrew => {
            fight.lose_max_hp(fight.actor(), fight.var(CardVar::MaxHp));
            Played::Done
        }
        at => unreachable!("{at:?} stops on a pick, not on a question it waited for"),
    }
}
