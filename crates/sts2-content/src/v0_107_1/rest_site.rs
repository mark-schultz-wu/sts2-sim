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

    /// `HEAL` and `SMITH`, and the two things that change that.
    ///
    /// `MEND` is the second player's, and a run here has one player. Girya and
    /// the Byrdonis Egg implement `TryModifyRestSiteOptions` in this build.
    /// Girya is not ported, so carrying it stops the run, rather than offering
    /// a list the game does not. Each Byrdonis Egg adds `HATCH`.
    fn offered(relics: &[Relic], deck: &[CardIdent<V0_107_1>]) -> Vec<RestSiteOption> {
        if relics.contains(&Relic::Girya) {
            unimplemented!("Girya changes what a rest site offers");
        }
        let mut offered = vec![RestSiteOption::Heal, RestSiteOption::Smith];
        offered.extend(
            deck.iter()
                .filter(|card| card.card == Cards::Quest(QuestCard::ByrdonisEgg))
                .map(|_| RestSiteOption::Hatch),
        );
        offered
    }

    /// `HatchRestSiteOption.OnSelect`: `RelicCmd.Obtain<Byrdpip>`.
    fn selected(self) -> Vec<sts2_core::HookEffect<V0_107_1>> {
        match self {
            RestSiteOption::Hatch => vec![sts2_core::HookEffect::ObtainRelic(Relic::Byrdpip)],
            option => unimplemented!("what choosing {option:?} at a rest site does"),
        }
    }
}
