//! `PrecariousShears`: two cards out of the deck, and a cut, as it is picked
//! up.

use sts2_core::{DeckAct, EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::Number;

/// `CardsVar(2)`.
const CARDS: usize = 2;

/// `DamageVar(16, ValueProp.Unpowered)`.
const DAMAGE: Number = 16;

/// `AfterObtained`: `CardSelectCmd.FromDeckForRemoval`, each card it takes
/// out of the deck, then `CreatureCmd.Damage` on its owner.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained = hook {
        asked.extend([
            HookEffect::SelectFromDeck {
                act: DeckAct::Remove,
                count: CARDS,
            },
            HookEffect::DamageOwner(DAMAGE),
        ]);
    }
}
