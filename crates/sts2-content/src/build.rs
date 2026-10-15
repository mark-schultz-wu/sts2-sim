//! What a rule shared between builds needs of the build it runs in.
//!
//! The behaviour is written once and is generic over the build. It names
//! content through the consts in [`crate::ids`], and reads a row through
//! `sts2_core::Table`. What every build spells alike is one type, in
//! [`crate::generated`]. Each bound here is one a shared rule reads.

use sts2_core::{ContentVersion, Table};

use crate::generated::orbs::OrbData;
use crate::ids::OrbIds;

/// A build the shared behaviour runs in.
///
/// Every build holds a number as an `i32`, so a shared rule does its
/// arithmetic in one.
pub trait Build: ContentVersion<Number = i32, Orb: OrbIds + Table<Data = OrbData>> {}

impl Build for crate::v0_107_1::V0_107_1 {}

impl Build for crate::v0_111_0::V0_111_0 {}
