//! Abyssal Baths: Max HP for damage that grows each time, or a heal.

use crate::ids::{EventIds as _, EventOptionIds as _};
use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, Room};

/// `MaxHpVar(2)`.
const MAX_HP: i32 = 2;

/// `DamageVar(3)`, unblockable and unpowered, before any immersion.
const DAMAGE: i32 = 3;

/// `HealVar(10)`.
const HEAL: i32 = 10;

#[derive(Debug, Clone)]
pub struct State {
    /// Whether the player went in, which turns the page from `Immerse` and
    /// `Abstain` to `Linger` and `ExitBaths`.
    bathing: bool,
    /// The damage the next immersion deals, one more after each.
    damage: i32,
    /// The option taken on the page it stands on.
    chosen: Option<NonMaxU8>,
}

pub struct AbyssalBaths;

impl<V: crate::Build> EventBehavior<V> for AbyssalBaths {
    type State = State;

    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    fn enter<R: sts2_core::rng::GameRng>(&self, _run: &impl EventView<V>, _rng: &mut R) -> State {
        State {
            bathing: false,
            damage: DAMAGE,
            chosen: None,
        }
    }

    /// The initial page, or the one every immersion leads to. `LingerCount`
    /// and the death warning pick the text alone, and the options are the
    /// same.
    fn observe(&self, state: &State, _run: &impl EventView<V>) -> Room<V> {
        let options = match state.bathing {
            false => [
                (V::EventOption::ABYSSAL_BATHS_IMMERSE, false),
                (V::EventOption::ABYSSAL_BATHS_ABSTAIN, false),
            ],
            true => [
                (V::EventOption::ABYSSAL_BATHS_ALL_LINGER, false),
                (V::EventOption::ABYSSAL_BATHS_ALL_EXIT_BATHS, false),
            ],
        };
        crate::behaviour::events::page::<V>(V::Event::ABYSSAL_BATHS, &options, state.chosen)
    }

    /// `WillKillPlayer`, for `Immerse` and `Linger`: the damage, less the maximum the dip gives.
    fn kills(&self, state: &State, option: u8, run: &impl EventView<V>) -> bool {
        option == 0 && run.hp() <= state.damage - MAX_HP
    }

    /// `Immerse` and `Linger`: `OnImmerse`, its Max HP and then its damage,
    /// one more next time, and the bath page again. `Abstain` heals, and
    /// `ExitBaths` leaves. Each of those finishes the event.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        if option != 0 {
            state.chosen = NonMaxU8::new(option);
            return match state.bathing {
                false => smallvec::smallvec![HookEffect::Heal(HEAL), HookEffect::FinishEvent],
                true => smallvec::smallvec![HookEffect::FinishEvent],
            };
        }
        let damage = state.damage;
        state.damage += 1;
        state.bathing = true;
        smallvec::smallvec![
            HookEffect::GainMaxHp(MAX_HP),
            HookEffect::DamageOwner(damage)
        ]
    }
}
