//! `PenNib`: every tenth attack played deals double damage.

use sts2_core::{CardId, EventHook, Modify, ModifyHook};

use crate::behaviour::relics::Number;
use sts2_core::CardKind;

/// `_attacksThreshold`.
const THRESHOLD: u8 = 10;

/// `AttacksPlayed`, its one `[SavedProperty]`, and `AttackToDouble`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State<V: crate::Build> {
    attacks_played: u8,
    to_double: Option<CardId<V>>,
}

impl<V: crate::Build> Default for State<V> {
    fn default() -> Self {
        Self {
            attacks_played: Default::default(),
            to_double: Default::default(),
        }
    }
}

/// `BeforeCardPlayed` counts each attack, and the tenth becomes the one to
/// double. `AfterCardPlayed` lets it go once its play is done.
pub(super) fn event_hook<V: crate::Build>(state: &mut State<V>, hook: EventHook<V>) {
    match hook {
        EventHook::CardPlaying { card, id, .. }
            if <V as sts2_core::Rules>::card_kind(card) == Some(CardKind::Attack) =>
        {
            state.attacks_played = (state.attacks_played + 1) % THRESHOLD;
            if state.attacks_played == 0 {
                state.to_double = Some(id);
            }
        }
        EventHook::CardPlayed { id, .. } if state.to_double == Some(id) => state.to_double = None,
        _ => {}
    }
}

/// `ModifyDamageMultiplicative`: double for a powered hit from the attack it
/// doubles. With none being doubled, double for one previewed out of play
/// while the next attack is the tenth. A card's hit comes from its owner or
/// Osty, as a fight here has one player. An Osty attack with no Osty comes
/// from nobody, which is what `Owner.Osty` is then, so it counts too.
pub(super) fn modify_hook<V: crate::Build>(
    state: &State<V>,
    hook: ModifyHook<V>,
) -> Modify<Number> {
    let ModifyHook::DamageMultiplicative {
        card: Some(source),
        powered: true,
        ..
    } = hook
    else {
        return Modify::Nothing;
    };
    let doubled = match state.to_double {
        None => !source.playing && state.attacks_played == THRESHOLD - 1,
        Some(id) => source.id == id,
    };
    match doubled {
        true => Modify::Scaled(sts2_core::Fixed::new(2, 0)),
        false => Modify::Nothing,
    }
}

/// `DisplayAmount`: the attacks counted towards the next tenth.
pub(super) fn display_amount<V: crate::Build>(state: &State<V>) -> Number {
    Number::from(state.attacks_played)
}
