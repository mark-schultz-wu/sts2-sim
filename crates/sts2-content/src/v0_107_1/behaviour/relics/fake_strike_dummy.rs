//! `FakeStrikeDummy`: one more damage on each powered hit from a Strike.

use sts2_core::{CardTable as _, EventHook, Modify, ModifyHook};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::CardTag;
use super::Number;

/// `DynamicVar("ExtraDamage", 1)`.
const EXTRA_DAMAGE: Number = 1;

/// `ModifyDamageAdditive`: its extra damage on a powered hit from a card
/// tagged Strike. A fight here has one player, so every card is its owner's.
pub(super) fn modify_hook(hook: ModifyHook<V>) -> Modify<Number> {
    match hook {
        ModifyHook::DamageAdditive {
            card: Some(source),
            powered: true,
            ..
        } if source.card.tagged(CardTag::Strike) => Modify::Added(EXTRA_DAMAGE),
        ModifyHook::DamageAdditive { .. } => Modify::Nothing,
        hook => unimplemented!("what FakeStrikeDummy does for {hook:?}"),
    }
}

/// Its one hook is `ModifyDamageAdditive`, which the fight asks of it rather
/// than tells it. `AfterObtained` reaches every relic, and its own does
/// nothing.
pub(super) fn event_hook(hook: EventHook<V>) {
    match hook {
        EventHook::Obtained => {}
        hook => unimplemented!("what FakeStrikeDummy does for {hook:?}"),
    }
}
