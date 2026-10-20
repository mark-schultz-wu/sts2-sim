//! `PotionModel.OnUse`, ported.
//!
//! One module per pool, filed the way `generated/potions/` is, so a potion's
//! behaviour sits under the same name as its row. A pool gets a module when
//! its first potion is ported.
//!
//! [`on_use`] panics for a potion nobody has ported, so this tree is also the
//! list of what a run can drink.

use crate::ids::PotionIds as _;

mod defect;
mod event;
mod ironclad;
mod necrobinder;
mod regent;
mod shared;
mod silent;
mod token;

use sts2_core::{Fight, Played, PotionTable, Taken};

use crate::generated::cards::TargetType;
use crate::generated::potions::PotionPool;

/// What a potion's own numbers are held as.
pub(crate) type Number = i32;

/// `CardSelectorPrefs(prompt, 0, 999999999)`: any number of cards.
pub(super) const ANY_NUMBER: usize = 999_999_999;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    Shared(shared::Resume),
    Event(event::Resume),
    Ironclad(ironclad::Resume),
}

/// `ShouldDie`, which only `FairyInABottle` answers in this build: no for
/// its owner, which keeps the player in the fight.
pub(crate) fn decide_hook<V: crate::Build>(
    potion: V::Potion,
    hook: sts2_core::DecisionHook<V>,
) -> sts2_core::HookResponse<bool> {
    match (potion, hook) {
        (__c0, sts2_core::DecisionHook::ShouldDie { creature, player })
            if __c0 == V::Potion::FAIRY_IN_A_BOTTLE =>
        {
            sts2_core::HookResponse::Decision(creature != player)
        }
        (potion, hook) => match hook.model_hook() {
            Some(_) => unimplemented!("what {potion:?} decides for {hook:?}"),
            None => sts2_core::HookResponse::DoesNotImplement,
        },
    }
}

/// `AfterPreventingDeath`, which `FairyInABottle` answers with
/// `OnUseWrapper` on the creature it saved.
pub(crate) fn event_hook<V: crate::Build>(
    potion: V::Potion,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (potion, hook) {
        (__c0, sts2_core::EventHook::DeathPrevented { .. })
            if __c0 == V::Potion::FAIRY_IN_A_BOTTLE =>
        {
            asked.push(sts2_core::HookEffect::UseOnSaved);
        }
        (potion, hook) => unimplemented!("what {potion:?} does for {hook:?}"),
    }
}

/// What a potion does when it is drunk.
///
/// # Panics
///
/// A potion from a pool nobody has ported, and a potion its own pool has not.
pub(crate) fn on_use<V: crate::Build, F: Fight<V>>(potion: V::Potion, fight: &mut F) -> Played<V> {
    match potion.pool() {
        PotionPool::Shared => shared::on_use::<V, _>(potion, fight),
        PotionPool::Silent => silent::on_use::<V, _>(potion, fight),
        PotionPool::Ironclad => ironclad::on_use::<V, _>(potion, fight),
        PotionPool::Defect => defect::on_use::<V, _>(potion, fight),
        PotionPool::Necrobinder => necrobinder::on_use::<V, _>(potion, fight),
        PotionPool::Regent => regent::on_use::<V, _>(potion, fight),
        PotionPool::Event => event::on_use::<V, _>(potion, fight),
        PotionPool::Token => token::on_use::<V, _>(potion, fight),
        _ => unimplemented!("what {potion:?} does when it is drunk"),
    }
}

/// What a potion asks of the run rather than of a fight.
///
/// # Panics
///
/// A potion nobody has ported outside a fight.
pub(crate) fn on_use_on_run<V: crate::Build>(
    potion: V::Potion,
    run: &impl sts2_core::EventView<V>,
    in_fight: bool,
) -> sts2_core::HookEffects<V> {
    use sts2_core::RunStreamKinds as _;

    match (potion, in_fight) {
        // `CreateRandomPotionOutOfCombat` off `CombatPotionGeneration` and
        // `TryToProcure`, while the belt has an open slot.
        (__c0, _) if __c0 == V::Potion::ENTROPIC_BREW => {
            smallvec::smallvec![sts2_core::HookEffect::FillPotionSlots {
                stream: <V as sts2_core::ContentVersion>::RunRngType::COMBAT_POTION_GENERATION,
            }]
        }
        // The rest act on the fight when drunk in one.
        (_, true) => sts2_core::HookEffects::new(),
        // `CreatureCmd.Heal` for a fifth of the maximum, which
        // `SetCurrentHpInternal` truncates.
        (__c0, false) if __c0 == V::Potion::BLOOD_POTION => {
            smallvec::smallvec![sts2_core::HookEffect::Heal(
                Number::from(run.max_hp()) * ironclad::HEAL_PERCENT / 100
            )]
        }
        // `CreatureCmd.GainMaxHp` by its `MaxHpVar(5)`.
        (__c0, false) if __c0 == V::Potion::FRUIT_JUICE => {
            smallvec::smallvec![sts2_core::HookEffect::GainMaxHp(FRUIT_JUICE)]
        }
        (potion, false) => unimplemented!("what {potion:?} does when it is drunk outside a fight"),
    }
}

/// `FruitJuice`'s `MaxHpVar(5)`.
const FRUIT_JUICE: Number = 5;

pub(crate) fn resume<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    answer: Taken<V>,
) -> Played<V> {
    match at {
        Resume::Shared(at) => shared::resume::<V, _>(fight, at, answer),
        Resume::Event(at) => event::resume::<V, _>(fight, at, answer),
        Resume::Ironclad(at) => {
            let Taken::Selected(picked) = answer else {
                unreachable!("an Ironclad potion asks for cards of a pile")
            };
            ironclad::resume::<V, _>(fight, at, picked)
        }
    }
}

/// [`sts2_core::Rules::computed_potion_target`]. Only an event potion works
/// its target out.
pub(crate) fn computed_target<V: crate::Build>(potion: V::Potion, in_combat: bool) -> TargetType {
    event::computed_target::<V>(potion, in_combat)
}

/// [`sts2_core::Rules::computed_potion_check`]. Only an event potion answers
/// from the run.
pub(crate) fn computed_check<V: crate::Build>(potion: V::Potion, in_combat: bool) -> bool {
    event::computed_check::<V>(potion, in_combat)
}
