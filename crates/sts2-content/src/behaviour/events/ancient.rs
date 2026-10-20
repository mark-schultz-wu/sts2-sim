//! What the ancients share: `AncientEventModel`'s relic options, one taken
//! with `RelicOption`, and then `Done`.

use nonmax::NonMaxU8;
use sts2_core::rng::{GameRng, Span};
use sts2_core::{EventOption, EventOptionKey, HookEffect, HookEffects, Room};

use crate::behaviour::EventOptionId;

/// What one copy of an ancient offers.
#[derive(Debug, Clone)]
pub struct Offered<V: crate::Build> {
    offered: Vec<V::Relic>,
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

impl<V: crate::Build> Offered<V> {
    /// `GenerateInitialOptions`, as the relics it drew.
    pub(super) fn new(offered: Vec<V::Relic>) -> Self {
        Offered {
            offered,
            chosen: None,
        }
    }

    /// Each relic as its option, in the order drawn.
    pub(super) fn observe(&self, event: V::Event) -> Room<V> {
        let options = self
            .offered
            .iter()
            .enumerate()
            .map(|(index, &relic)| EventOption {
                index: index as u8,
                key: EventOptionKey {
                    event,
                    option: EventOptionId::Relic(relic),
                },
                title: None,
                text: None,
                locked: false,
                proceed: false,
                chosen: self.chosen == NonMaxU8::new(index as u8),
                relic: Some(relic),
            })
            .collect();
        Room::Event {
            event,
            finished: false,
            title: None,
            text: None,
            options,
        }
    }

    /// The relic an option offers.
    pub(super) fn relic(&self, option: u8) -> V::Relic {
        self.offered[usize::from(option)]
    }

    /// `RelicOption`'s `OnChosen`: `RelicCmd.Obtain`, then `Done`.
    pub(super) fn choose(&mut self, option: u8) -> HookEffects<V> {
        let relic = self.offered[usize::from(option)];
        self.chosen = NonMaxU8::new(option);
        smallvec::smallvec![HookEffect::ObtainRelic(relic), HookEffect::FinishEvent]
    }
}

/// One item of a list, which is `Rng.NextItem`.
///
/// A two-argument `NextInt`, which is a different draw from the one-argument
/// one even over the same range. An empty list costs no draw at all.
pub(super) fn next_item<T: Copy, R: GameRng>(rng: &mut R, items: &[T]) -> Option<T> {
    let count: R::Int = num_traits::cast(items.len())?;
    let span = Span::new(num_traits::zero(), count).ok()?;
    let at: usize = num_traits::cast(rng.next_int_range(span))?;
    items.get(at).copied()
}
