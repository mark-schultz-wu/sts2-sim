//! `ScrollBoxes`: a choice of two bundles of cards.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, DefectCard};
use super::super::super::generated::characters::Character;

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
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained { character } = hook {
        asked.push(HookEffect::OfferBundles {
            count: BUNDLES,
            commons: COMMONS,
            uncommons: UNCOMMONS,
            same_card: (character == Character::Defect)
                .then_some((Cards::Defect(DefectCard::Claw), CLAW_CHANCE)),
        });
    }
}
