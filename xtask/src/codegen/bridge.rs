//! The consts a rule holding only a `ContentVersion` names an enum's members by.
//!
//! Three domains generate an enum the simulator has to name a member of, and
//! each generates it per build. This is the shared half: check the members
//! against what the simulator names, then write the `impl`. Which members those
//! are stays with the domain that owns the enum.

use std::fmt::Write as _;

use super::sanitize;

/// One generated enum, and the consts the simulator names its members by.
///
/// A generated enum is per build, so the simulator cannot write
/// `MapPointType::Elite`. These are the bridges, spelled out member by member
/// rather than derived from the order.
pub(super) struct Bridge {
    /// The generated enum.
    pub(super) enum_name: &'static str,
    /// The trait in `sts2_core` whose consts it implements.
    pub(super) trait_name: &'static str,
    /// `(const, member)`, in the trait's own order. The member is the name
    /// the game reflects, which is not always the variant it generates as.
    pub(super) named: &'static [(&'static str, &'static str)],
    /// The members deliberately left off the trait. Each is a C# zero,
    /// standing for "no answer" rather than for a value.
    ///
    /// Every member appears in one list or the other. A build that adds or
    /// renames one stops here, where the answer is to look at what changed.
    pub(super) unnamed: &'static [&'static str],
}

pub(super) fn emit_bridge(
    out: &mut String,
    bridge: &Bridge,
    members: &[(u8, String)],
) -> Result<(), String> {
    let Bridge {
        enum_name,
        trait_name,
        named,
        unnamed,
    } = bridge;

    for (_, member) in *named {
        if !members.iter().any(|(_, have)| have == member) {
            return Err(format!(
                "the dump reflects no {enum_name}::{member}, which the simulator \
                 names — see what this build calls it"
            ));
        }
    }
    if members.len() != named.len() + unnamed.len() {
        return Err(format!(
            "{enum_name} has {} members and the simulator names {} — the new one \
             needs a const on sts2_core::{trait_name}",
            members.len(),
            named.len() + unnamed.len()
        ));
    }

    writeln!(out, "impl sts2_core::{trait_name} for {enum_name} {{").unwrap();
    for (name, member) in *named {
        writeln!(
            out,
            "    const {name}: {enum_name} = {enum_name}::{};",
            sanitize(member)
        )
        .unwrap();
    }
    writeln!(out, "}}\n").unwrap();
    Ok(())
}
