//! Boundary vocabulary between generated content ids and the game's wire keys.

use sts2_core::ContentVersion;

pub trait WireContentVersion: ContentVersion {
    fn event_option(
        event: Self::Event,
        name: &str,
        relic: Option<Self::Relic>,
    ) -> Option<Self::EventOptionId>;
    fn event_option_name(option: Self::EventOptionId, locked: bool) -> Option<&'static str>;
}

macro_rules! wire_options {
    ($module:ident, $version:ident) => {
        impl WireContentVersion for sts2_content::$module::$version {
            fn event_option(
                event: Self::Event,
                name: &str,
                relic: Option<Self::Relic>,
            ) -> Option<Self::EventOptionId> {
                use sts2_content::$module::events::Event;
                use sts2_content::$module::{
                    ByrdonisNestOption as B, EventOptionId as Id, TeaMasterOption as T,
                    WellspringOption as W,
                };
                match (event, name.strip_suffix("_LOCKED").unwrap_or(name)) {
                    (Event::ByrdonisNest, "EAT") => Some(Id::ByrdonisNest(B::Eat)),
                    (Event::ByrdonisNest, "LEAVE") => Some(Id::ByrdonisNest(B::Leave)),
                    (Event::Wellspring, "DRINK") => Some(Id::Wellspring(W::Drink)),
                    (Event::Wellspring, "FILL") => Some(Id::Wellspring(W::Fill)),
                    (Event::Wellspring, "LEAVE") => Some(Id::Wellspring(W::Leave)),
                    (Event::Wellspring, "PRAY") => Some(Id::Wellspring(W::Pray)),
                    (Event::TeaMaster, "BONE_TEA") => Some(Id::TeaMaster(T::BoneTea)),
                    (Event::TeaMaster, "EMBER_TEA") => Some(Id::TeaMaster(T::EmberTea)),
                    (Event::TeaMaster, "TEA_OF_DISCOURTESY") => {
                        Some(Id::TeaMaster(T::TeaOfDiscourtesy))
                    }
                    _ => relic.map(Id::Relic),
                }
            }

            fn event_option_name(
                option: Self::EventOptionId,
                locked: bool,
            ) -> Option<&'static str> {
                use sts2_content::$module::{
                    ByrdonisNestOption as B, EventOptionId as Id, TeaMasterOption as T,
                    WellspringOption as W,
                };
                match (option, locked) {
                    (Id::ByrdonisNest(B::Eat), _) => Some("EAT"),
                    (Id::ByrdonisNest(B::Leave), _) => Some("LEAVE"),
                    (Id::Wellspring(W::Drink), _) => Some("DRINK"),
                    (Id::Wellspring(W::Fill), _) => Some("FILL"),
                    (Id::Wellspring(W::Leave), _) => Some("LEAVE"),
                    (Id::Wellspring(W::Pray), _) => Some("PRAY"),
                    (Id::TeaMaster(T::BoneTea), false) => Some("BONE_TEA"),
                    (Id::TeaMaster(T::BoneTea), true) => Some("BONE_TEA_LOCKED"),
                    (Id::TeaMaster(T::EmberTea), false) => Some("EMBER_TEA"),
                    (Id::TeaMaster(T::EmberTea), true) => Some("EMBER_TEA_LOCKED"),
                    (Id::TeaMaster(T::TeaOfDiscourtesy), false) => Some("TEA_OF_DISCOURTESY"),
                    (Id::TeaMaster(T::TeaOfDiscourtesy), true) => Some("TEA_OF_DISCOURTESY_LOCKED"),
                    (Id::Relic(_), _) => None,
                }
            }
        }
    };
}

wire_options!(v0_107_1, V0_107_1);
wire_options!(v0_111_0, V0_111_0);
