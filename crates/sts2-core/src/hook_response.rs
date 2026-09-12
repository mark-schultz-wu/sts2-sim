/// A content model's response to a hook that it does not always implement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookResponse<T> {
    Decision(T),
    DoesNotImplement,
}

impl<T> HookResponse<T> {
    /// Returns whether this response contains a decision matching `predicate`.
    pub fn is_decision_and(self, predicate: impl FnOnce(T) -> bool) -> bool {
        match self {
            Self::Decision(value) => predicate(value),
            Self::DoesNotImplement => false,
        }
    }
}
