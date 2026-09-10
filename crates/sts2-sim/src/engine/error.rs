/// What the simulator cannot do yet.
///
/// A fault rather than a refusal: the game did not reject the move. The
/// simulator lacks the rule required to answer it.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// No rule for this yet.
    #[error("the simulator does not implement {0} yet")]
    NotImplemented(&'static str),
    #[error(transparent)]
    Combat(#[from] crate::combat::Error),
}
