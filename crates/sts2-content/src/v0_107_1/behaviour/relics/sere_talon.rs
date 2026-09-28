//! `SereTalon`: two random curses, and three Wishes.

use sts2_core::{EventHook, HookEffect};

use super::super::super::V0_107_1 as V;
use super::super::super::generated::cards::{Cards, CurseCard, EventCard};
use super::super::super::generated::runstate::RunRngType;

/// `Curses`.
const CURSES: usize = 2;

/// `Wishes`.
const WISHES: usize = 3;

/// `CurseCardPool`'s cards whose `CanBeGeneratedByModifiers` holds, ordered
/// by id. Ascender's Bane, Bad Luck, Curse of the Bell, Enthralled, Folly,
/// Greed, Poor Sleep and Spore Mind override it to refuse.
const CURSE_POOL: [Cards; 10] = [
    Cards::Curse(CurseCard::Clumsy),
    Cards::Curse(CurseCard::Debt),
    Cards::Curse(CurseCard::Decay),
    Cards::Curse(CurseCard::Doubt),
    Cards::Curse(CurseCard::Guilty),
    Cards::Curse(CurseCard::Injury),
    Cards::Curse(CurseCard::Normality),
    Cards::Curse(CurseCard::Regret),
    Cards::Curse(CurseCard::Shame),
    Cards::Curse(CurseCard::Writhe),
];

/// `AfterObtained`: two curses, each `NextItem` off `RunState.Rng.Niche`
/// over those not drawn already, into the deck. Then three Wishes.
pub(super) fn event_hook(hook: EventHook<V>, asked: &mut sts2_core::HookEffects<V>) {
    if let EventHook::Obtained = hook {
        asked.push(HookEffect::AddDistinctToDeck {
            cards: &CURSE_POOL,
            count: CURSES,
            stream: RunRngType::Niche,
        });
        for _ in 0..WISHES {
            asked.push(HookEffect::AddToDeck(Cards::Event(EventCard::Wish)));
        }
    }
}
