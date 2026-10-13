//! What a rest site offers, and the names a rule reaches its options by.
//!
//! Hand-written beside the generated enum, the way `event_options` is. The
//! table says which options exist. This says which of them a rest site opens
//! with, and that is behaviour rather than a column.

use sts2_core::{CardIdent, RestKinds};

use super::V0_107_1;
use super::generated::cards::{Cards, QuestCard};
use super::generated::relics::Relic;
use super::generated::restsite::RestSiteOption;

/// `CookRestSiteOption`'s `_cardsToRemove`.
const COOK_CARDS: usize = 2;

/// `CookRestSiteOption`'s `_maxHpGain`.
const COOK_MAX_HP: i32 = 9;

impl RestKinds<V0_107_1> for RestSiteOption {
    const CLONE: RestSiteOption = RestSiteOption::Clone;
    const COOK: RestSiteOption = RestSiteOption::Cook;
    const DIG: RestSiteOption = RestSiteOption::Dig;
    const HATCH: RestSiteOption = RestSiteOption::Hatch;
    const HEAL: RestSiteOption = RestSiteOption::Heal;
    const KINDLE: RestSiteOption = RestSiteOption::Kindle;
    const LIFT: RestSiteOption = RestSiteOption::Lift;
    const MEND: RestSiteOption = RestSiteOption::Mend;
    const SMITH: RestSiteOption = RestSiteOption::Smith;

    /// `HEAL` and `SMITH`, and what `TryModifyRestSiteOptions` adds.
    ///
    /// `MEND` is the second player's, and a run here has one player.
    /// `IterateHookListeners` walks the deck ahead of the belt, so each
    /// Byrdonis Egg adds `HATCH` before any relic adds its option.
    fn offered(from_belt: &[RestSiteOption], deck: &[CardIdent<V0_107_1>]) -> Vec<RestSiteOption> {
        let mut offered = vec![RestSiteOption::Heal, RestSiteOption::Smith];
        offered.extend(
            deck.iter()
                .filter(|card| card.card == Cards::Quest(QuestCard::ByrdonisEgg))
                .map(|_| RestSiteOption::Hatch),
        );
        offered.extend_from_slice(from_belt);
        offered
    }

    /// `HatchRestSiteOption.OnSelect`: `RelicCmd.Obtain<Byrdpip>`.
    /// `DigRestSiteOption.OnSelect`: `RelicCmd.Obtain` of the next relic off
    /// the front of the bag.
    /// `LiftRestSiteOption.OnSelect`: Girya's `TimesLifted` one higher.
    /// `CloneRestSiteOption.OnSelect`: a clone of each deck card enchanted
    /// with Clone, added to the deck.
    /// `CookRestSiteOption.OnSelect`: two cards removed from the deck, asked
    /// for with `RequireManualConfirmation`, then nine more maximum health.
    fn selected(self) -> Vec<sts2_core::HookEffect<V0_107_1>> {
        match self {
            RestSiteOption::Hatch => vec![sts2_core::HookEffect::ObtainRelic(Relic::Byrdpip)],
            RestSiteOption::Dig => vec![sts2_core::HookEffect::ObtainNextRelic(
                sts2_core::RelicPull::Rolled,
            )],
            RestSiteOption::Lift => vec![sts2_core::HookEffect::RestOptionOfRelic(Relic::Girya)],
            RestSiteOption::Kindle => {
                vec![sts2_core::HookEffect::RestOptionOfRelic(
                    Relic::PumpkinCandle,
                )]
            }
            RestSiteOption::Clone => vec![sts2_core::HookEffect::CloneEnchantedIntoDeck(
                super::generated::enchantments::Enchantment::Clone,
            )],
            RestSiteOption::Cook => vec![
                sts2_core::HookEffect::SelectFromDeckConfirmed {
                    act: sts2_core::DeckAct::Remove,
                    min: COOK_CARDS,
                    max: COOK_CARDS,
                },
                sts2_core::HookEffect::GainMaxHp(COOK_MAX_HP),
            ],
            option => unimplemented!("what choosing {option:?} at a rest site does"),
        }
    }
}
