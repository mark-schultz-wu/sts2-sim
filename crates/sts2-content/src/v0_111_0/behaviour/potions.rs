//! The potions whose rules this build ports for itself, which the shared
//! potions hand over through [`crate::build::OwnRules`].

use sts2_core::{CreatureView as _, Fight, Played, PowerAmount};

use super::super::V0_111_0 as V;
use super::super::generated::potions::Potion;
use super::super::generated::powers::Power;

/// `Ambergris`'s `DynamicVar("HealPercent", 50m)`, which only this build
/// has.
const AMBERGRIS_HEAL_PERCENT: i32 = 50;

/// What a potion of this build's own does when it is drunk in a fight.
///
/// `Ambergris`: `CreatureCmd.Heal` of its target by half its maximum, then
/// one `AmbergrisPower` on it, from its drinker. Its target is any player,
/// and a fight here has one.
pub(crate) fn own_potion_use<F: Fight<V>>(potion: Potion, fight: &mut F) -> Option<Played<V>> {
    match potion {
        Potion::Ambergris => {
            let actor = fight.actor();
            let max_hp: i32 = fight
                .creatures()
                .find(|creature| creature.id() == actor)
                .map(|creature| creature.max_hp())
                .expect("the player drinking stands in the fight");
            fight.heal(actor, max_hp * AMBERGRIS_HEAL_PERCENT / 100);
            fight.apply_to_actor(Power::AmbergrisPower, PowerAmount::Counted(1));
            Some(Played::Done)
        }
        _ => None,
    }
}

/// What a potion of this build's own asks of the run.
///
/// `Ambergris` outside a fight: `CreatureCmd.Heal` by half the maximum, and
/// no power, since no fight is in progress.
pub(crate) fn own_potion_use_on_run(
    potion: Potion,
    run: &impl sts2_core::EventView<V>,
    in_fight: bool,
) -> Option<sts2_core::HookEffects<V>> {
    match (potion, in_fight) {
        (Potion::Ambergris, true) => Some(sts2_core::HookEffects::new()),
        (Potion::Ambergris, false) => Some(smallvec::smallvec![sts2_core::HookEffect::Heal(
            run.max_hp() * AMBERGRIS_HEAL_PERCENT / 100
        )]),
        _ => None,
    }
}
