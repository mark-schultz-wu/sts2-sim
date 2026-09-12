use sts2_core::EventOptionKinds;

use super::{V0_111_0, relics::Relic};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TeaMasterOption {
    BoneTea,
    EmberTea,
    TeaOfDiscourtesy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ByrdonisNestOption {
    Eat,
    Leave,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WellspringOption {
    Drink,
    Fill,
    Leave,
    Pray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventOptionId {
    Relic(Relic),
    ByrdonisNest(ByrdonisNestOption),
    TeaMaster(TeaMasterOption),
    Wellspring(WellspringOption),
}

impl EventOptionKinds<V0_111_0> for EventOptionId {
    fn from_relic(relic: Relic) -> Self {
        Self::Relic(relic)
    }

    fn relic(self) -> Option<Relic> {
        match self {
            Self::Relic(relic) => Some(relic),
            Self::TeaMaster(TeaMasterOption::BoneTea) => Some(Relic::BoneTea),
            Self::TeaMaster(TeaMasterOption::EmberTea) => Some(Relic::EmberTea),
            Self::TeaMaster(TeaMasterOption::TeaOfDiscourtesy) => Some(Relic::TeaOfDiscourtesy),
            Self::ByrdonisNest(_) | Self::Wellspring(_) => None,
        }
    }
}
