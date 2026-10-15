//! What content does, written once for every build.
//!
//! Generic over the build, and filed the way a build's own behaviour is. A
//! rule moves here once it names content through the build. A rule a later
//! build changed stays in each build that has it.

pub(crate) mod orbs;
