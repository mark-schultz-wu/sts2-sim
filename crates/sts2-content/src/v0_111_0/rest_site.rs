//! What a rest site offers, and the names a rule reaches its options by.
//!
//! Hand-written beside the generated enum, the way `event_options` is. The
//! table says which options exist. This says which of them a rest site opens
//! with, and that is behaviour rather than a column.

use sts2_core::{CardIdent, RestKinds};

use super::V0_111_0;
use super::generated::restsite::RestSiteOption;

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

    /// `HEAL` and `SMITH`, the way `v0.107.1` opens.
    ///
    /// No rule drives this build yet, so nothing calls this. What
    /// `TryModifyRestSiteOptions` implements here is unread, and a run that
    /// reaches a rest site on this build stops rather than guessing.
    fn offered(
        _from_belt: &[RestSiteOption],
        _deck: &[CardIdent<V0_111_0>],
    ) -> Vec<RestSiteOption> {
        unimplemented!("what a rest site offers on v0.111.0")
    }

    fn selected(self) -> Vec<sts2_core::HookEffect<V0_111_0>> {
        unimplemented!("what choosing {self:?} at a rest site does on v0.111.0")
    }
}
