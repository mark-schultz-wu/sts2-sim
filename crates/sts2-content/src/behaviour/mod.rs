//! What content does, written once for every build.
//!
//! Generic over the build, and filed the way a build's own behaviour is. A
//! rule moves here once it names content through the build. A rule a later
//! build changed stays in each build that has it.

pub(crate) mod acts;
pub(crate) mod enchantments;
pub(crate) mod encounters;
pub(crate) mod modifiers;
pub(crate) mod orbs;

/// What a fight's card writes on its deck version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeckWrite {
    /// `BuffFromPlay(n)` of `GeneticAlgorithm` or `TheScythe`.
    BuffFromPlay(i32),
    /// `DeckVersion.Enchantment.Amount += n`, Goopy's.
    RaiseEnchantment(u8),
}
