//! The Regent's cards the generator leaves to a hand port, filed by rarity.

use sts2_core::{Fight, Played};

use super::super::super::generated::card_plays;
use super::super::super::generated::cards::{CardVar, RegentCard};
use super::V;

mod common;
mod uncommon;

/// Where a Regent card stopped inside its own play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    /// `Begone`, after it selected the card to turn into a Minion Strike.
    BegoneTransform,
    /// `Glimmer`, after its owner picked the cards to put back.
    GlimmerPutBack,
    /// `PhotonCut`, after its owner picked the cards to put back.
    PhotonCutPutBack,
}

pub(super) fn on_play<F: Fight<V>>(card: RegentCard, fight: &mut F) -> Played<V> {
    match card {
        RegentCard::Glitterstream => common::glitterstream(fight),
        RegentCard::Begone => common::begone(),
        RegentCard::CollisionCourse => common::collision_course(fight),
        RegentCard::Radiate => uncommon::radiate(fight),
        RegentCard::LunarBlast => uncommon::lunar_blast(fight),
        RegentCard::HeavenlyDrill => heavenly_drill(fight),
        RegentCard::KnockoutBlow => knockout_blow(fight),
        RegentCard::PhotonCut => photon_cut(fight),
        RegentCard::SummonForth => summon_forth(fight),
        // `Glimmer`: its `Cards` drawn, then its `PutBack` of the hand onto
        // the top of the draw pile.
        RegentCard::Glimmer => {
            fight.draw(fight.var(CardVar::Cards));
            let count = usize::try_from(fight.var(CardVar::PutBack))
                .ok()
                .and_then(nonmax::NonMaxUsize::new)
                .expect("a count of cards is not negative");
            Played::Asking {
                pile: sts2_core::Pile::Hand,
                selection: sts2_core::Selection::Exactly(count),
                at: super::super::Resume::Card(super::Resume::Regent(Resume::GlimmerPutBack)),
            }
        }
        RegentCard::ManifestAuthority => manifest_authority(fight),
        card => card_plays::regent::on_play(card, fight),
    }
}

/// Its damage to the target, as one attack.
fn strike<F: Fight<V>>(fight: &mut F) {
    use sts2_core::{Amount, Effect, Scaling};

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

/// `KnockoutBlow`: its damage to the target, then its stars where the hit
/// killed it.
fn knockout_blow<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::CreatureView as _;

    let target = fight.target();
    strike(fight);
    let standing = fight
        .creatures()
        .any(|creature| creature.id() == target && creature.alive());
    if !standing {
        fight.gain_stars(fight.var(CardVar::Stars));
    }
    Played::Done
}

/// `PhotonCut`: its damage to the target, its `Cards` drawn, then its
/// `PutBack` of the hand onto the top of the draw pile.
fn photon_cut<F: Fight<V>>(fight: &mut F) -> Played<V> {
    strike(fight);
    fight.draw(fight.var(CardVar::Cards));
    let count = usize::try_from(fight.var(CardVar::PutBack))
        .ok()
        .and_then(nonmax::NonMaxUsize::new)
        .expect("a count of cards is not negative");
    Played::Asking {
        pile: sts2_core::Pile::Hand,
        selection: sts2_core::Selection::Exactly(count),
        at: super::super::Resume::Card(super::Resume::Regent(Resume::PhotonCutPutBack)),
    }
}

/// `SummonForth`: every Sovereign Blade of its owner's outside the hand into
/// the hand, then its `Forge`.
fn summon_forth<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use super::super::super::generated::cards::{Cards, TokenCard};
    use sts2_core::Pile;

    let blade = Cards::Token(TokenCard::SovereignBlade);
    let blades: Vec<_> = [Pile::Draw, Pile::Discard, Pile::Exhaust]
        .into_iter()
        .flat_map(|pile| fight.cards_in(pile))
        .filter(|&card| fight.card_ident(card).card == blade)
        .collect();
    fight.act_on(sts2_core::PileAct::IntoHand, &blades);
    fight.forge(blade, CardVar::Damage, fight.var(CardVar::Forge));
    Played::Done
}

/// `MakeItSo.AfterCardPlayedLate`: after a Skill of its owner's, into hand
/// from outside it. That is once the Skills finished this turn come to a
/// whole multiple of its `Cards`. A fight here has one player.
fn make_it_so<P: sts2_core::Printed<V> + ?Sized>(
    printed: &P,
    history: sts2_core::History<'_, V>,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    use sts2_core::{CardKind, Rules as _};

    let sts2_core::EventHook::CardPlayedLate { card, .. } = hook else {
        unimplemented!("what MakeItSo does for {hook:?}");
    };
    if V::card_kind(card) != Some(CardKind::Skill) {
        return;
    }
    let skills = history
        .finished_this_turn()
        .filter(|&card| V::card_kind(card) == Some(CardKind::Skill))
        .count();
    let skills = super::Number::try_from(skills).expect("a turn's plays fit a game number");
    if skills % printed.face(CardVar::Cards) == 0 {
        asked.push(sts2_core::HookEffect::AddSelfToHand { only_from: None });
    }
}

/// `HeavenlyDrill`: its damage to the target, once for each energy its X
/// spent, and twice that from its `Energy` on.
fn heavenly_drill<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::{Amount, Effect, Scaling};

    let mut hits = fight.x_value();
    if hits >= fight.var(CardVar::Energy) {
        hits *= 2;
    }
    let target = fight.target();
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

/// `ManifestAuthority`: its block, then one colorless card off
/// `CombatCardGeneration` into hand, upgraded where it is.
fn manifest_authority<F: Fight<V>>(fight: &mut F) -> Played<V> {
    use sts2_core::{Amount, Effect, Rules as _, RunStreamKinds as _, Scaling};

    let block = fight.resolve(Effect::GainBlock {
        recipient: fight.actor(),
        amount: Amount::Var(CardVar::Block),
        scaling: Scaling::Dynamic,
    });
    fight.apply(block);
    let mut cards = V::combat_colorless_cards().to_vec();
    let Ok([card]) = fight.generate_cards::<1>(
        &mut cards,
        <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION,
    ) else {
        return Played::Done;
    };
    let upgraded = fight.upgraded();
    for at in fight.add_generated_cards(card, sts2_core::Pile::Hand, 1) {
        if upgraded {
            fight.modify_card(
                at,
                sts2_core::Change::Upgrade,
                sts2_core::Lasting::ThisFight,
            );
        }
    }
    Played::Done
}

/// Deliver a notification to one Regent card in the fight.
pub(super) fn event_hook<P: sts2_core::Printed<V> + ?Sized>(
    card: RegentCard,
    id: sts2_core::CardId<V>,
    printed: &P,
    history: sts2_core::History<'_, V>,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match card {
        RegentCard::KinglyKick => uncommon::kingly_kick(id, hook, asked),
        RegentCard::KinglyPunch => uncommon::kingly_punch(id, printed, hook, asked),
        RegentCard::MakeItSo => make_it_so(printed, history, hook, asked),
        card => unimplemented!("what {card:?} does for {hook:?}"),
    }
}

/// The rest of a play that stopped, once the fight has its answer.
pub(super) fn resume_play<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        Resume::BegoneTransform => common::begone_transform(fight, chosen),
        // `CardPileCmd.Add` onto the top of the draw pile.
        Resume::GlimmerPutBack | Resume::PhotonCutPutBack => {
            fight.act_on(sts2_core::PileAct::OntoDrawTop, chosen.as_slice());
            Played::Done
        }
    }
}
