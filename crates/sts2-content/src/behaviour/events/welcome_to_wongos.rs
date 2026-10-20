//! Welcome to Wongos: a relic bought at one of three prices, or a card
//! downgraded on the way out.

use nonmax::NonMaxU8;
use sts2_core::{EventBehavior, EventView, HookEffect, HookEffects, RelicPull, Room, Stream};

use crate::generated::relics::RelicRarity;
use crate::ids::{EventIds as _, EventOptionIds as _, RelicIds as _};

type Number = i32;

/// `BargainBinCost`.
const BARGAIN_BIN: Number = 100;

/// `FeaturedItemCost`.
const FEATURED_ITEM: Number = 200;

/// `MysteryBoxCost`.
const MYSTERY_BOX: Number = 300;

#[derive(Debug, Clone)]
pub struct State<V: crate::Build> {
    /// `FeaturedItem`, pulled as the options generate.
    featured: Option<V::Relic>,
    /// Whether the purse held each price as the options generated, in the
    /// order they stand.
    affordable: [bool; 3],
    /// The option taken.
    chosen: Option<NonMaxU8>,
}

pub struct WelcomeToWongos;

impl<V: crate::Build> EventBehavior<V> for WelcomeToWongos {
    type State = State<V>;

    /// The second act, with a hundred gold or more, which the table of
    /// allowed events checks.
    fn allowed(&self, _run: &impl EventView<V>) -> bool {
        true
    }

    /// `GenerateInitialOptions`, which locks each purchase the purse cannot
    /// cover.
    fn enter<R: sts2_core::rng::GameRng>(&self, run: &impl EventView<V>, _rng: &mut R) -> State<V> {
        let gold = run.gold();
        State {
            featured: None,
            affordable: [BARGAIN_BIN, FEATURED_ITEM, MYSTERY_BOX].map(|cost| gold >= cost),
            chosen: None,
        }
    }

    /// `FeaturedItem`: the next rare among those a merchant stocks.
    fn relics_pulled(&self, _state: &State<V>) -> &'static [RelicPull<V>] {
        &[RelicPull::Stocked(RelicRarity::Rare)]
    }

    fn hand_pulled_relics(&self, state: &mut State<V>, relics: &[V::Relic]) {
        state.featured = relics.first().copied();
    }

    fn observe(&self, state: &State<V>, _run: &impl EventView<V>) -> Room<V> {
        let [bargain, featured, mystery] = state.affordable;
        crate::behaviour::events::page::<V>(
            V::Event::WELCOME_TO_WONGOS,
            &[
                (V::EventOption::WELCOME_TO_WONGOS_BARGAIN_BIN, !bargain),
                (V::EventOption::WELCOME_TO_WONGOS_FEATURED_ITEM, !featured),
                (V::EventOption::WELCOME_TO_WONGOS_MYSTERY_BOX, !mystery),
                (V::EventOption::WELCOME_TO_WONGOS_LEAVE, false),
            ],
            state.chosen,
        )
    }

    /// Each purchase spends its price and obtains its relic: the next
    /// common among those a merchant stocks, the featured rare, or the
    /// Mystery Ticket. `Leave` downgrades one upgraded card off the event's stream.
    ///
    /// `CheckObtainWongoBadge` hands over a badge once the profile's Wongo
    /// points pass a multiple of two thousand. A run starts from a profile
    /// with none, and one visit earns thirty-two at most.
    fn choose<R: sts2_core::rng::GameRng>(
        &self,
        state: &mut State<V>,
        option: u8,
        _run: &impl EventView<V>,
        _rng: &mut R,
    ) -> HookEffects<V> {
        state.chosen = NonMaxU8::new(option);
        let bought = match option {
            0 => (
                BARGAIN_BIN,
                HookEffect::ObtainNextRelic(RelicPull::Stocked(RelicRarity::Common)),
            ),
            1 => (
                FEATURED_ITEM,
                HookEffect::ObtainRelic(
                    state
                        .featured
                        .unwrap_or_else(|| unreachable!("the featured relic is pulled on entry")),
                ),
            ),
            2 => (
                MYSTERY_BOX,
                HookEffect::ObtainRelic(V::Relic::WONGOS_MYSTERY_TICKET),
            ),
            _ => {
                return smallvec::smallvec![
                    HookEffect::DowngradeRandomDeckCard {
                        stream: Stream::Event,
                    },
                    HookEffect::FinishEvent,
                ];
            }
        };
        let (cost, relic) = bought;
        smallvec::smallvec![HookEffect::LoseGold(cost), relic, HookEffect::FinishEvent]
    }
}
