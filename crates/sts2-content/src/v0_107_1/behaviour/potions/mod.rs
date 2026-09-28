//! `PotionModel.OnUse`, ported.
//!
//! One module per pool, filed the way `generated/potions/` is, so a potion's
//! behaviour sits under the same name as its row. A pool gets a module when
//! its first potion is ported.
//!
//! [`on_use`] panics for a potion nobody has ported, so this tree is also the
//! list of what a run can drink.

mod defect;
mod event;
mod ironclad;
mod necrobinder;
mod regent;
mod shared;
mod silent;
mod token;

use sts2_core::{Fight, Played, PotionTable, Taken};

use super::super::V0_107_1 as V;
use super::super::generated::cards::TargetType;
use super::super::generated::potions::{Potion, PotionPool};

/// What a potion's own numbers are held as.
pub(crate) type Number = <V as sts2_core::ContentVersion>::Number;

/// `CardSelectorPrefs(prompt, 0, 999999999)`: any number of cards.
pub(super) const ANY_NUMBER: usize = 999_999_999;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    Shared(shared::Resume),
    Ironclad(ironclad::Resume),
}

/// What a potion does when it is drunk.
///
/// # Panics
///
/// A potion from a pool nobody has ported, and a potion its own pool has not.
pub(crate) fn on_use<F: Fight<V>>(potion: Potion, fight: &mut F) -> Played<V> {
    match potion.pool() {
        PotionPool::Shared => shared::on_use(potion, fight),
        PotionPool::Silent => silent::on_use(potion, fight),
        PotionPool::Ironclad => ironclad::on_use(potion, fight),
        PotionPool::Defect => defect::on_use(potion, fight),
        PotionPool::Necrobinder => necrobinder::on_use(potion, fight),
        PotionPool::Regent => regent::on_use(potion, fight),
        PotionPool::Event => event::on_use(potion, fight),
        PotionPool::Token => token::on_use(potion, fight),
        _ => unimplemented!("what {potion:?} does when it is drunk"),
    }
}

/// What a potion asks of the run rather than of a fight.
///
/// # Panics
///
/// A potion nobody has ported outside a fight.
pub(crate) fn on_use_on_run(
    potion: Potion,
    run: &impl sts2_core::EventView<V>,
    in_fight: bool,
) -> sts2_core::HookEffects<V> {
    use sts2_core::RunStreamKinds as _;

    match (potion, in_fight) {
        // `CreateRandomPotionOutOfCombat` off `CombatPotionGeneration` and
        // `TryToProcure`, while the belt has an open slot.
        (Potion::EntropicBrew, _) => smallvec::smallvec![sts2_core::HookEffect::FillPotionSlots {
            stream: <V as sts2_core::ContentVersion>::RunRngType::COMBAT_POTION_GENERATION,
        }],
        // The rest act on the fight when drunk in one.
        (_, true) => sts2_core::HookEffects::new(),
        // `CreatureCmd.Heal` for a fifth of the maximum, which
        // `SetCurrentHpInternal` truncates.
        (Potion::BloodPotion, false) => smallvec::smallvec![sts2_core::HookEffect::Heal(
            Number::from(run.max_hp()) * ironclad::HEAL_PERCENT / 100
        )],
        // `CreatureCmd.GainMaxHp` by its `MaxHpVar(5)`.
        (Potion::FruitJuice, false) => {
            smallvec::smallvec![sts2_core::HookEffect::GainMaxHp(FRUIT_JUICE)]
        }
        (potion, false) => unimplemented!("what {potion:?} does when it is drunk outside a fight"),
    }
}

/// `FruitJuice`'s `MaxHpVar(5)`.
const FRUIT_JUICE: Number = 5;

pub(crate) fn resume<F: Fight<V>>(fight: &mut F, at: Resume, answer: Taken<V>) -> Played<V> {
    match at {
        Resume::Shared(at) => shared::resume(fight, at, answer),
        Resume::Ironclad(at) => {
            let Taken::Selected(picked) = answer else {
                unreachable!("an Ironclad potion asks for cards of a pile")
            };
            ironclad::resume(fight, at, picked)
        }
    }
}

/// [`sts2_core::Rules::computed_potion_target`]. Only an event potion works
/// its target out.
pub(crate) fn computed_target(potion: Potion, in_combat: bool) -> TargetType {
    event::computed_target(potion, in_combat)
}

/// [`sts2_core::Rules::computed_potion_check`]. Only an event potion answers
/// from the run.
pub(crate) fn computed_check(potion: Potion, in_combat: bool) -> bool {
    event::computed_check(potion, in_combat)
}
