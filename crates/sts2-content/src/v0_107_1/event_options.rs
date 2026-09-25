use sts2_core::EventOptionKinds;

use super::{V0_107_1, events::EventOption, relics::Relic};

/// An option an event offers.
///
/// An ancient offers relics, and its keys name the relic. Every other option
/// is one its page names, which the localisation table lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventOptionId {
    Relic(Relic),
    Said(EventOption),
}

impl EventOptionKinds<V0_107_1> for EventOptionId {
    fn from_relic(relic: Relic) -> Self {
        Self::Relic(relic)
    }

    fn relic(self) -> Option<Relic> {
        match self {
            Self::Relic(relic) => Some(relic),
            Self::Said(_) => None,
        }
    }
}
