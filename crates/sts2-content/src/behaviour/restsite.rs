//! What a rest site offers, and what choosing an option does.
//!
//! Each build's `RestKinds` impl, beside its generated enum, names the
//! options and hands these the numbers its game sets.

use sts2_core::HookEffect;

use crate::ids::{EnchantmentIds as _, RelicIds as _, RestSiteOptionIds};

/// `CookRestSiteOption`'s `_cardsToRemove`.
const COOK_CARDS: usize = 2;

/// `HEAL` and `SMITH`, `MEND` in a run of several players, and what
/// `TryModifyRestSiteOptions` adds.
///
/// `IterateHookListeners` walks the deck ahead of the belt, so each
/// Byrdonis Egg adds `HATCH` before any relic adds its option.
pub(crate) fn offered<V: crate::Build>(
    players: usize,
    added: &[V::RestSiteOption],
) -> Vec<V::RestSiteOption> {
    let mut offered = vec![
        <V::RestSiteOption as RestSiteOptionIds>::HEAL,
        <V::RestSiteOption as RestSiteOptionIds>::SMITH,
    ];
    if players > 1 {
        offered.push(<V::RestSiteOption as RestSiteOptionIds>::MEND);
    }
    offered.extend_from_slice(added);
    offered
}

/// `HatchRestSiteOption.OnSelect`: `RelicCmd.Obtain<Byrdpip>`.
/// `DigRestSiteOption.OnSelect`: `RelicCmd.Obtain` of the next relic off
/// the front of the bag.
/// `LiftRestSiteOption.OnSelect`: Girya's `TimesLifted` one higher.
/// `CloneRestSiteOption.OnSelect`: a clone of each deck card enchanted
/// with Clone, added to the deck.
/// `CookRestSiteOption.OnSelect`: two cards removed from the deck, asked
/// for with `RequireManualConfirmation`, then `cook_max_hp` more maximum
/// health, the build's `_maxHpGain`.
pub(crate) fn selected<V: crate::Build>(
    option: V::RestSiteOption,
    cook_max_hp: i32,
) -> Vec<HookEffect<V>> {
    match option {
        __c0 if __c0 == <V::RestSiteOption as RestSiteOptionIds>::HATCH => {
            vec![HookEffect::ObtainRelic(V::Relic::BYRDPIP)]
        }
        __c0 if __c0 == <V::RestSiteOption as RestSiteOptionIds>::DIG => {
            vec![HookEffect::ObtainNextRelic(sts2_core::RelicPull::Rolled)]
        }
        __c0 if __c0 == <V::RestSiteOption as RestSiteOptionIds>::LIFT => {
            vec![HookEffect::RestOptionOfRelic(V::Relic::GIRYA)]
        }
        __c0 if __c0 == <V::RestSiteOption as RestSiteOptionIds>::KINDLE => {
            vec![HookEffect::RestOptionOfRelic(V::Relic::PUMPKIN_CANDLE)]
        }
        __c0 if __c0 == <V::RestSiteOption as RestSiteOptionIds>::CLONE => {
            vec![HookEffect::CloneEnchantedIntoDeck(V::Enchantment::CLONE)]
        }
        __c0 if __c0 == <V::RestSiteOption as RestSiteOptionIds>::COOK => vec![
            HookEffect::SelectFromDeckConfirmed {
                act: sts2_core::DeckAct::Remove,
                min: COOK_CARDS,
                max: COOK_CARDS,
            },
            HookEffect::GainMaxHp(cook_max_hp),
        ],
        option => unimplemented!("what choosing {option:?} at a rest site does"),
    }
}
