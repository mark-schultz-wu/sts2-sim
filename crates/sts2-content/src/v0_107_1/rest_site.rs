//! The names a rule reaches this build's rest-site options by, and the
//! number its Cook gives.
//!
//! Hand-written beside the generated enum, the way `event_options` is. What
//! a rest site offers and what an option does are the shared
//! `behaviour::restsite`.

use sts2_core::{CardIdent, RestKinds};

use super::V0_107_1;
use super::generated::restsite::RestSiteOption;

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

    fn offered(from_belt: &[RestSiteOption], deck: &[CardIdent<V0_107_1>]) -> Vec<RestSiteOption> {
        crate::behaviour::restsite::offered::<V0_107_1>(from_belt, deck)
    }

    fn selected(self) -> Vec<sts2_core::HookEffect<V0_107_1>> {
        crate::behaviour::restsite::selected::<V0_107_1>(self, COOK_MAX_HP)
    }
}
