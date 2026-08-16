//! Game content, one module per build.
//!
//! Content is the part that moves between game versions: which cards exist,
//! what they cost, what they target. The trait layer above does not, so it
//! lives elsewhere and this crate is what gets regenerated.
//!
//! A version is identified by [`Version::content_hash`], not by its name.
//! Names repeat across builds and say nothing about whether ids still line up;
//! the hash is XxHash32 over the game's own model-id namespace, and it moves
//! when and only when content moves. The game uses the same value to refuse a
//! replay recorded against different content, and `sts2-headless` reports it in
//! the greeting — so a table can be checked against the build it is talking to
//! before anything trusts it.

#[cfg(feature = "v0_107_1")]
pub mod v0_107_1;

/// A card field that is usually a constant, but which some cards work out from
/// combat state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Field<T> {
    /// Fixed for this card, whatever the run is doing.
    Constant(T),
    /// The card computes it — `Shiv` widens to all enemies under Fan of Knives,
    /// `Mad Science` becomes whatever it stored. Ask; do not look up.
    Computed,
}

/// Which build a content module came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    /// The build's own version string. A label, not an identity.
    pub name: &'static str,
    /// XxHash32 over the model-id namespace. This is the identity.
    pub content_hash: u32,
}
