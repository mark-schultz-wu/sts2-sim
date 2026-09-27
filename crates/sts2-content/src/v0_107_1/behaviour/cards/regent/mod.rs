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
}

pub(super) fn on_play<F: Fight<V>>(card: RegentCard, fight: &mut F) -> Played<V> {
    match card {
        RegentCard::Glitterstream => common::glitterstream(fight),
        RegentCard::Begone => common::begone(),
        RegentCard::CollisionCourse => common::collision_course(fight),
        RegentCard::Radiate => uncommon::radiate(fight),
        RegentCard::LunarBlast => uncommon::lunar_blast(fight),
        RegentCard::HeavenlyDrill => heavenly_drill(fight),
        RegentCard::ManifestAuthority => manifest_authority(fight),
        card => card_plays::regent::on_play(card, fight),
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
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match card {
        RegentCard::KinglyKick => uncommon::kingly_kick(id, hook, asked),
        RegentCard::KinglyPunch => uncommon::kingly_punch(id, printed, hook, asked),
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
    }
}
