use sts2_core::EventOptionKinds;

use super::{V0_111_0, events::EventOption, modifiers::Modifier, relics::Relic};

/// An option an event offers.
///
/// An ancient offers relics, and its keys name the relic. The Architect's
/// lines and a bare `PROCEED` name no page. Every other option is one its
/// page names, which the localisation table lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventOptionId {
    Relic(Relic),
    Said(EventOption),
    /// A line of The Architect's dialogue, `<event>.dialogue.<line>`, which
    /// moves it on to the next.
    Dialogue(u8),
    /// A bare `PROCEED`, which no page names.
    Proceed,
    /// The option a run modifier gives Neow, whose key is the modifier's id.
    Modifier(Modifier),
}

impl EventOptionKinds<V0_111_0> for EventOptionId {
    fn from_relic(relic: Relic) -> Self {
        Self::Relic(relic)
    }

    fn relic(self) -> Option<Relic> {
        match self {
            Self::Relic(relic) => Some(relic),
            Self::Said(_) | Self::Modifier(_) | Self::Dialogue(_) | Self::Proceed => None,
        }
    }

    fn from_modifier(modifier: Modifier) -> Self {
        Self::Modifier(modifier)
    }
}
