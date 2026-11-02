//! The names a rule reaches this build's rest-site options by, and the
//! number its Cook gives.
//!
//! Hand-written beside the generated enum, the way `event_options` is. What
//! a rest site offers and what an option does are the shared
//! `behaviour::restsite`.

use sts2_core::RestKinds;

use super::V0_111_0;
use super::generated::restsite::RestSiteOption;

/// `CookRestSiteOption`'s `_maxHpGain`.
const COOK_MAX_HP: i32 = 5;

impl RestKinds<V0_111_0> for RestSiteOption {
    const CLONE: RestSiteOption = RestSiteOption::Clone;
    const COOK: RestSiteOption = RestSiteOption::Cook;
    const DIG: RestSiteOption = RestSiteOption::Dig;
    const HATCH: RestSiteOption = RestSiteOption::Hatch;
    const HEAL: RestSiteOption = RestSiteOption::Heal;
    const KINDLE: RestSiteOption = RestSiteOption::Kindle;
    const LIFT: RestSiteOption = RestSiteOption::Lift;
    const MEND: RestSiteOption = RestSiteOption::Mend;
    const SMITH: RestSiteOption = RestSiteOption::Smith;

    fn offered(players: usize, added: &[RestSiteOption]) -> Vec<RestSiteOption> {
        crate::behaviour::restsite::offered::<V0_111_0>(players, added)
    }

    fn selected(self) -> Vec<sts2_core::HookEffect<V0_111_0>> {
        crate::behaviour::restsite::selected::<V0_111_0>(self, COOK_MAX_HP)
    }
}
