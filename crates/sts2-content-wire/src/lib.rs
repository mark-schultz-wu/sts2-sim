//! Boundary vocabulary between generated content ids and the game's wire keys.

use sts2_core::{ContentVersion, EventOptionKinds as _};

pub trait WireContentVersion: ContentVersion {
    /// The option a key names: `<event>.pages.<page>.options.<name>`.
    ///
    /// An ancient's keys name the relic it offers, which the room reports
    /// beside the key.
    fn event_option(
        event: Self::Event,
        page: &str,
        name: &str,
        relic: Option<Self::Relic>,
    ) -> Option<Self::EventOptionId>;
    /// The option a key that names no page names: The Architect's
    /// `<event>.dialogue.<line>`, or a bare `PROCEED`.
    fn event_option_unpaged(event: Self::Event, key: &str) -> Option<Self::EventOptionId>;
    /// The whole key for an option that names no page, and `None` for any
    /// other.
    fn event_option_unpaged_spelled(
        event: Self::Event,
        option: Self::EventOptionId,
    ) -> Option<String>;
    /// The page and the name a key spells for an option its page names.
    /// `None` for a relic, and for an option that names no page.
    fn event_option_spelled(
        option: Self::EventOptionId,
        locked: bool,
    ) -> Option<(&'static str, &'static str)>;
}

/// An option's key as the game spells it, put back together from its parts.
///
/// # Panics
///
/// An id no key spells, which the generated ids and the relics cover.
pub fn option_key<V: WireContentVersion>(option: &sts2_core::EventOption<V>) -> String {
    if let Some(key) = V::event_option_unpaged_spelled(option.key.event, option.key.option) {
        return key;
    }
    let (page, name) = V::event_option_spelled(option.key.option, option.locked)
        .map(|(page, name)| (page, name.to_owned()))
        .or_else(|| {
            let relic = option.key.option.relic()?;
            Some(("INITIAL", spelled(&relic)))
        })
        .expect("every option has a key the game spells");
    format!("{}.pages.{page}.options.{name}", spelled(&option.key.event))
}

/// The key of an option that leaves an event, where no page names it.
const PROCEED: &str = "PROCEED";

/// A generated id, by the serde rename the game spells it with.
fn spelled<T: serde::Serialize>(id: &T) -> String {
    match serde_json::to_value(id) {
        Ok(serde_json::Value::String(name)) => name,
        _ => unreachable!("a generated id serializes as its name"),
    }
}

macro_rules! wire_options {
    ($module:ident, $version:ident) => {
        impl WireContentVersion for sts2_content::$module::$version {
            fn event_option(
                event: Self::Event,
                page: &str,
                name: &str,
                relic: Option<Self::Relic>,
            ) -> Option<Self::EventOptionId> {
                use sts2_content::$module::EventOptionId as Id;
                use sts2_content::$module::events::EventOption;
                EventOption::from_key(event, page, name)
                    .map(Id::Said)
                    .or(relic.map(Id::Relic))
            }

            fn event_option_unpaged(event: Self::Event, key: &str) -> Option<Self::EventOptionId> {
                use sts2_content::$module::EventOptionId as Id;
                if key == PROCEED {
                    return Some(Id::Proceed);
                }
                key.strip_prefix(&spelled(&event))?
                    .strip_prefix(".dialogue.")?
                    .parse()
                    .ok()
                    .map(Id::Dialogue)
            }

            fn event_option_unpaged_spelled(
                event: Self::Event,
                option: Self::EventOptionId,
            ) -> Option<String> {
                use sts2_content::$module::EventOptionId as Id;
                match option {
                    Id::Dialogue(line) => Some(format!("{}.dialogue.{line}", spelled(&event))),
                    Id::Proceed => Some(PROCEED.to_owned()),
                    Id::Said(_) | Id::Relic(_) => None,
                }
            }

            fn event_option_spelled(
                option: Self::EventOptionId,
                locked: bool,
            ) -> Option<(&'static str, &'static str)> {
                use sts2_content::$module::EventOptionId as Id;
                match option {
                    Id::Said(option) => Some(option.spelled(locked)),
                    Id::Relic(_) | Id::Dialogue(_) | Id::Proceed => None,
                }
            }
        }
    };
}

wire_options!(v0_107_1, V0_107_1);
wire_options!(v0_111_0, V0_111_0);
