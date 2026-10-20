//! `ScrollBoxes`: a choice of two bundles of cards.

use sts2_core::{EventHook, HookEffect};

use crate::ids::{CardIds as _, CharacterIds as _};

/// The bundles it offers.
const BUNDLES: usize = 2;

/// The commons in a bundle, then the uncommons.
const COMMONS: usize = 2;
const UNCOMMONS: usize = 1;

/// `_clawBundleChancePercent`: a Defect's bundle is three Claws one time in a
/// hundred.
const CLAW_CHANCE: u32 = 1;

/// `AfterObtained`: `FromChooseABundleScreen` over `GenerateRandomBundles`,
/// and each card of the bundle chosen into the deck.
pub(super) fn event_hook<V: crate::Build>(
    hook: EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    if let EventHook::Obtained { character, .. } = hook {
        asked.push(HookEffect::OfferBundles {
            count: BUNDLES,
            commons: COMMONS,
            uncommons: UNCOMMONS,
            same_card: (character == V::Character::DEFECT).then_some((V::Card::CLAW, CLAW_CHANCE)),
        });
    }
}
