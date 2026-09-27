//! `Byrdpip`: the Byrdonis Egg hatched, fighting beside the player.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, EventCard, QuestCard};
use super::super::super::generated::monsters::Monster;

/// `AfterObtained` turns each Byrdonis Egg in the deck into a Byrd Swoop, and
/// `BeforeCombatStart` adds the Byrdpip as a pet. The skin it picks draws off
/// its own `Rng`, which no other stream shares.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    match hook {
        EventHook::Obtained => asked.push(HookEffect::TransformInDeck {
            from: Cards::Quest(QuestCard::ByrdonisEgg),
            into: Cards::Event(EventCard::ByrdSwoop),
        }),
        EventHook::CombatStarting => asked.push(HookEffect::AddPet(Monster::Byrdpip)),
        hook => unimplemented!("what Byrdpip does for {hook:?}"),
    }
}
