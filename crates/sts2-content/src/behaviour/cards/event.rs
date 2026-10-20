//! The event cards the generator leaves to a hand port.

use crate::ids::{CardIds as _, CardVarIds as _, PowerIds as _};
use sts2_core::{
    Amount, Change, CostPool, CreatureView as _, Effect, Fight, Lasting, Pile, PileAct, Played,
    PowerAmount, Scaling, Selection,
};

/// Where an event card stopped inside its own play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume<V: crate::Build> {
    /// `NeowsFury`, after its owner picked the cards to take back.
    NeowsFuryPicks,
    /// `Wish`, after its owner picked the card to take.
    WishPick,
    /// `BrightestFlame`, after a question inside its draw was answered.
    BrightestFlameDrew,
    /// `DualWield`, after its owner picked the card to copy.
    DualWieldPick,
    /// `Maul`, after a question inside its attack was answered.
    MaulHit,
    /// A Mad Science attack, after a question inside one of its hits, with
    /// `left` hits still to land.
    MadScienceHit { card: V::Card, left: u8 },
    /// v0.111.0's `Abundance`, after the Power chosen from the three it
    /// offered.
    AbundanceChoice,
}

/// The rest of the play waits at `at`, where a command left a question and
/// more commands follow.
fn wait<V: crate::Build>(at: Resume<V>) -> Played<V> {
    Played::Waiting {
        at: crate::behaviour::cards::Resume::Event(at).into(),
    }
}

pub(super) fn on_play<V: crate::Build, F: Fight<V>>(card: V::Card, fight: &mut F) -> Played<V> {
    match card {
        __c0 if __c0 == V::Card::NEOWS_FURY => neows_fury::<V, _>(fight),
        // `Enlightenment`: every card in hand costs at most one, this turn or
        // until played, or for the fight once upgraded.
        __c0 if __c0 == V::Card::ENLIGHTENMENT => {
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
        __c0 if __c0 == V::Card::TORIC_TOUGHNESS => {
            use sts2_core::EffectResolution as _;

            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(V::CardVar::BLOCK),
                scaling: Scaling::Dynamic,
            });
            let gained = block.amount();
            fight.apply(block);
            let turns = sts2_core::PowerAmount::Counted(fight.var(V::CardVar::TURNS));
            fight.apply_to_actor(V::Power::TORIC_TOUGHNESS_POWER, turns);
            fight.note_on_power(fight.actor(), V::Power::TORIC_TOUGHNESS_POWER, gained);
            Played::Done
        }
        __c0 if __c0 == V::Card::METAMORPHOSIS => metamorphosis::<V, _>(fight),
        __c0 if __c0 == V::Card::WHISTLE => whistle::<V, _>(fight),
        __c0 if __c0 == V::Card::APOTHEOSIS => apotheosis::<V, _>(fight),
        __c0 if __c0 == V::Card::MAUL => maul::<V, _>(fight),
        __c0 if __c0 == V::Card::MAD_SCIENCE_SAPPING
            || __c0 == V::Card::MAD_SCIENCE_VIOLENCE
            || __c0 == V::Card::MAD_SCIENCE_CHOKING
            || __c0 == V::Card::MAD_SCIENCE_ENERGIZED
            || __c0 == V::Card::MAD_SCIENCE_WISDOM
            || __c0 == V::Card::MAD_SCIENCE_CHAOS
            || __c0 == V::Card::MAD_SCIENCE_EXPERTISE
            || __c0 == V::Card::MAD_SCIENCE_CURIOUS
            || __c0 == V::Card::MAD_SCIENCE_IMPROVEMENT =>
        {
            mad_science::<V, _>(card, fight)
        }
        // `Wish`: one card of the draw pile into hand.
        __c0 if __c0 == V::Card::WISH => Played::Asking {
            pile: Pile::Draw,
            selection: Selection::Exactly(nonmax::NonMaxUsize::ONE),
            at: crate::behaviour::cards::Resume::Event(Resume::WishPick).into(),
        },
        // `BrightestFlame`: its Energy, its Cards drawn, then its `MaxHp` off
        // its owner's maximum.
        __c0 if __c0 == V::Card::BRIGHTEST_FLAME => {
            fight.modify_energy(fight.var(V::CardVar::ENERGY));
            fight.draw(fight.var(V::CardVar::CARDS));
            // A shuffle inside the draw can ask, and the rest waits for it.
            if fight.waiting() {
                return Played::Waiting {
                    at: crate::behaviour::cards::Resume::Event(Resume::BrightestFlameDrew).into(),
                };
            }
            fight.lose_max_hp(fight.actor(), fight.var(V::CardVar::MAX_HP));
            Played::Done
        }
        // Its damage to the target. The Byrdpip pet only animates the hit.
        __c0 if __c0 == V::Card::BYRD_SWOOP => {
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
            Played::Done
        }
        // `Clash`: its damage to the target. Its `IsPlayable` asks for a
        // hand of Attacks alone.
        __c0 if __c0 == V::Card::CLASH => {
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
            Played::Done
        }
        // `Distraction`: a Skill of the character's pool, free this turn,
        // into the hand.
        __c0 if __c0 == V::Card::DISTRACTION => {
            crate::behaviour::free_card_into_hand(
                fight,
                |card| V::card_kind(card) == Some(sts2_core::CardKind::Skill),
                true,
            );
            Played::Done
        }
        // `DualWield`: one Attack or Power of the hand, then its `Cards` of
        // clones of it into the hand.
        __c0 if __c0 == V::Card::DUAL_WIELD => {
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
                at: crate::behaviour::cards::Resume::Event(Resume::DualWieldPick).into(),
            }
        }
        // `Entrench`: as much block again as its owner holds, unpowered.
        __c0 if __c0 == V::Card::ENTRENCH => {
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
        __c0 if __c0 == V::Card::FEEDING_FRENZY => {
            let amount = fight.var(V::CardVar::STRENGTH_POWER);
            fight.apply_to_actor(V::Power::FEEDING_FRENZY_POWER, PowerAmount::Counted(amount));
            Played::Done
        }
        // `Stack`: as much block as its `CalculatedBlock`, one more for each
        // card in the discard pile.
        __c0 if __c0 == V::Card::STACK => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(V::CardVar::CALCULATED_BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            Played::Done
        }
        card => crate::behaviour::cards::generated_from::<V, _>(card, fight, 0),
    }
}

/// The rest of a play that stopped on a choose-a-card screen.
pub(super) fn resume_chosen<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V>,
    chosen: Option<sts2_core::CardIdent<V>>,
) -> Played<V> {
    match at {
        // `SetToFreeThisTurn`, then `CardPileCmd.AddGeneratedCardToCombat`
        // into hand.
        Resume::AbundanceChoice => {
            if let Some(card) = chosen {
                for at in fight.add_generated_cards(card, Pile::Hand, 1) {
                    crate::behaviour::free_this_turn(fight, at);
                }
            }
            Played::Done
        }
        at => unreachable!("{at:?} stops on a pick of a pile"),
    }
}

pub(super) fn resume_play<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V>,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        // `CardPileCmd.Add(picked, PileType.Hand)`.
        Resume::NeowsFuryPicks | Resume::WishPick => {
            fight.act_on(PileAct::IntoHand, chosen.as_slice());
            Played::Done
        }
        Resume::BrightestFlameDrew | Resume::MaulHit | Resume::MadScienceHit { .. } => {
            unreachable!("{at:?} waits on a question, not a pick")
        }
        Resume::AbundanceChoice => unreachable!("{at:?} chooses, and picks nothing"),
        // `CreateClone` of the card picked, its `Cards` times, each through
        // `AddGeneratedCardToCombat` into the hand.
        Resume::DualWieldPick => {
            if let Some(picked) = chosen.one_or_none() {
                for _ in 0..fight.var(V::CardVar::CARDS) {
                    fight.clone_card(picked, Pile::Hand);
                }
            }
            Played::Done
        }
    }
}

/// `NeowsFury`: its damage to the target, then up to its `Cards` from the
/// discard pile into hand, as many as the hand has room for.
fn neows_fury<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
    let cards: usize =
        num_traits::cast(fight.var(V::CardVar::CARDS)).expect("a count of cards is not negative");
    let room = sts2_core::MAX_CARDS_IN_HAND.saturating_sub(fight.cards_in(Pile::Hand).len());
    match nonmax::NonMaxUsize::new(cards.min(room)) {
        Some(count) if count.get() > 0 => Played::Asking {
            pile: Pile::Discard,
            selection: Selection::UpTo(count),
            at: crate::behaviour::cards::Resume::Event(Resume::NeowsFuryPicks).into(),
        },
        _ => Played::Done,
    }
}

/// `Metamorphosis`: its `Cards` attacks from the character's pool, each
/// `NextItem` off `CombatCardGeneration`, free this fight and into the draw
/// pile at a random place.
fn metamorphosis<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let attacks = crate::behaviour::character_cards(fight, sts2_core::CardKind::Attack);
    for _ in 0..fight.var(V::CardVar::CARDS) {
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
pub(crate) fn hit<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    target: sts2_core::CombatId,
    hits: usize,
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

/// A Mad Science attack's `left` hits on the target, each its own attack,
/// then the power it carries. A question left before the next waits.
fn mad_science_hits<V: crate::Build, F: Fight<V>>(
    card: V::Card,
    fight: &mut F,
    left: u8,
) -> Played<V> {
    use sts2_core::PowerAmount;

    let target = fight.target();
    for done in 0..left {
        hit::<V, _>(fight, target, 1);
        if fight.waiting() {
            return wait::<V>(Resume::MadScienceHit {
                card,
                left: left - done - 1,
            });
        }
    }
    let counted = |fight: &F, key| PowerAmount::Counted(fight.var(key));
    match card {
        __c0 if __c0 == V::Card::MAD_SCIENCE_SAPPING => {
            let weak = counted(fight, V::CardVar::SAPPING_WEAK);
            fight.apply_power(target, V::Power::WEAK_POWER, weak);
            let vulnerable = counted(fight, V::CardVar::SAPPING_VULNERABLE);
            fight.apply_power(target, V::Power::VULNERABLE_POWER, vulnerable);
        }
        __c0 if __c0 == V::Card::MAD_SCIENCE_CHOKING => {
            let strangle = counted(fight, V::CardVar::CHOKING_DAMAGE);
            fight.apply_power(target, V::Power::STRANGLE_POWER, strangle);
        }
        _ => {}
    }
    Played::Done
}

/// `MadScience`: what its type does, then its rider.
///
/// An attack hits the target once, or `ViolenceHits` times with Violence,
/// each hit its own command. A skill gains its block. A power applies what
/// its rider names to its owner. Sapping and Choking then debuff the target,
/// Energized gives energy, Wisdom draws, and Chaos adds a card of the
/// character's pool, free this turn.
fn mad_science<V: crate::Build, F: Fight<V>>(card: V::Card, fight: &mut F) -> Played<V> {
    use sts2_core::PowerAmount;

    let counted = |fight: &F, key| PowerAmount::Counted(fight.var(key));
    match card {
        __c0 if __c0 == V::Card::MAD_SCIENCE_SAPPING
            || __c0 == V::Card::MAD_SCIENCE_VIOLENCE
            || __c0 == V::Card::MAD_SCIENCE_CHOKING =>
        {
            let hits = match card {
                __c0 if __c0 == V::Card::MAD_SCIENCE_VIOLENCE => {
                    num_traits::cast(fight.var(V::CardVar::VIOLENCE_HITS)).expect("a few hits")
                }
                _ => 1,
            };
            return mad_science_hits::<V, _>(card, fight, hits);
        }
        __c0 if __c0 == V::Card::MAD_SCIENCE_ENERGIZED
            || __c0 == V::Card::MAD_SCIENCE_WISDOM
            || __c0 == V::Card::MAD_SCIENCE_CHAOS =>
        {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(V::CardVar::BLOCK),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            match card {
                __c0 if __c0 == V::Card::MAD_SCIENCE_ENERGIZED => {
                    fight.modify_energy(fight.var(V::CardVar::ENERGIZED_ENERGY))
                }
                __c0 if __c0 == V::Card::MAD_SCIENCE_WISDOM => {
                    fight.draw(fight.var(V::CardVar::WISDOM_CARDS));
                }
                _ => crate::behaviour::free_card_into_hand(fight, |_| true, false),
            }
        }
        __c0 if __c0 == V::Card::MAD_SCIENCE_EXPERTISE => {
            let strength = counted(fight, V::CardVar::EXPERTISE_STRENGTH);
            fight.apply_to_actor(V::Power::STRENGTH_POWER, strength);
            let dexterity = counted(fight, V::CardVar::EXPERTISE_DEXTERITY);
            fight.apply_to_actor(V::Power::DEXTERITY_POWER, dexterity);
        }
        __c0 if __c0 == V::Card::MAD_SCIENCE_CURIOUS => {
            let curious = counted(fight, V::CardVar::CURIOUS_REDUCTION);
            fight.apply_to_actor(V::Power::CURIOUS_POWER, curious);
        }
        __c0 if __c0 == V::Card::MAD_SCIENCE_IMPROVEMENT => {
            fight.apply_to_actor(V::Power::IMPROVEMENT_POWER, PowerAmount::Counted(1));
        }
        card => unreachable!("{card:?} is not a Mad Science"),
    }
    Played::Done
}

/// `Whistle`: its damage to the target, then `CreatureCmd.Stun` on it with
/// no move named after.
fn whistle<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    hit::<V, _>(fight, target, 1);
    fight.stun(
        target,
        <V as crate::build::Lists>::powers_stunned(),
        sts2_core::StunResume::Interrupted,
    );
    Played::Done
}

/// `Maul`: its damage to the target twice, then its `Increase` onto the
/// damage of every Maul its owner holds in the fight, itself included.
fn maul<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let target = fight.target();
    hit::<V, _>(fight, target, 2);
    if fight.waiting() {
        return wait::<V>(Resume::MaulHit);
    }
    maul_hit::<V, _>(fight)
}

/// The rest of `Maul`, once its attack is over: every Maul's damage raised.
fn maul_hit<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
    let raise = Change::Own(crate::behaviour::cards::CardWrite::BuffFromPlay(
        fight.var(V::CardVar::INCREASE),
    ));
    // `AllCards`: the hand, the draw, discard and exhaust piles, then the
    // play pile, which holds the Maul being played.
    for pile in [Pile::Hand, Pile::Draw, Pile::Discard, Pile::Exhaust] {
        for card in fight.cards_in(pile) {
            if fight.card_ident(card).card == V::Card::MAUL {
                fight.modify_card(card, raise, Lasting::ThisFight);
            }
        }
    }
    fight.modify_this(raise, Lasting::ThisFight);
    Played::Done
}

/// `Apotheosis`: every other card its owner holds in the fight upgraded,
/// where it can be.
fn apotheosis<V: crate::Build, F: Fight<V>>(fight: &mut F) -> Played<V> {
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
pub(super) fn resume_waited<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V>,
) -> Played<V> {
    match at {
        Resume::BrightestFlameDrew => {
            fight.lose_max_hp(fight.actor(), fight.var(V::CardVar::MAX_HP));
            Played::Done
        }
        Resume::MaulHit => maul_hit::<V, _>(fight),
        Resume::MadScienceHit { card, left } => mad_science_hits::<V, _>(card, fight, left),
        at => unreachable!("{at:?} stops on a pick, not on a question it waited for"),
    }
}
