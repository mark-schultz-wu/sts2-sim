//! A shop purchase in a run of several players, which runs on the buyer's
//! machine alone.
//!
//! `MerchantEntry.OnTryPurchase` runs where the player clicked. Once it
//! returns, with every pick its `RelicCmd.Obtain` asked answered, the
//! buyer's machine sends `RewardSynchronizer`'s `GoldLostMessage` and
//! `RewardObtainedMessage`. Each other machine then loses the gold and
//! obtains the thing for that player itself. A pick that obtain asks takes
//! the cards the buyer's `SyncLocalChoice` sent.
//!
//! So each machine keeps what it is waiting on, and the table hands each
//! purchase's results from the buyer's machine to the others.

use std::collections::VecDeque;

use smallvec::SmallVec;
use sts2_core::{CardIdent, Rules, SeatIndex};

use super::State;

/// What a machine knows of the purchases running on its own machine and on
/// the others'.
#[derive(Debug, Clone)]
pub(super) struct Relay<V: Rules> {
    /// The purchase this machine's own player is making, with the deck picks
    /// its obtain took so far.
    buying: Option<Purchase<V>>,
    /// The purchases this machine finished, which the table hands to the
    /// other machines.
    outbox: Vec<Purchase<V>>,
    /// The players whose purchase runs on their own machine and has not come
    /// back.
    elsewhere: SmallVec<[SeatIndex; 4]>,
    /// The deck picks a relayed purchase's obtain takes, in the order the
    /// buyer made them.
    synced: VecDeque<Vec<usize>>,
}

impl<V: Rules> Default for Relay<V> {
    fn default() -> Self {
        Self {
            buying: None,
            outbox: Vec::new(),
            elsewhere: SmallVec::new(),
            synced: VecDeque::new(),
        }
    }
}

/// One purchase's results, as the buyer's machine sends them.
#[derive(Debug, Clone)]
pub(crate) struct Purchase<V: Rules> {
    seat: SeatIndex,
    /// `GoldLostMessage`'s gold.
    spent: V::Number,
    /// `RewardObtainedMessage`'s model.
    bought: Bought<V>,
    /// Each deck pick the obtain took, as the deck's places of the cards.
    picks: Vec<Vec<usize>>,
}

/// What a purchase obtained.
#[derive(Debug, Clone, Copy)]
pub(super) enum Bought<V: Rules> {
    Relic(V::Relic),
    Potion(V::Potion),
    Card(CardIdent<V>),
}

impl<V: Rules> State<V> {
    /// Whether `seat`'s purchase runs on another machine than this one. A
    /// run of one player has no other.
    pub(super) fn bought_elsewhere(&self, seat: SeatIndex) -> bool {
        self.players.len() > 1 && seat != self.local
    }

    /// Another player's purchase, which this machine waits on.
    pub(super) fn wait_on_purchase(&mut self, seat: SeatIndex) {
        self.relay.get_or_insert_default().elsewhere.push(seat);
    }

    /// Whether `seat` is buying on their own machine, whose answers this one
    /// never sees asked.
    pub(crate) fn buying_elsewhere(&self, seat: SeatIndex) -> bool {
        self.relay
            .as_ref()
            .is_some_and(|relay| relay.elsewhere.contains(&seat))
    }

    /// This machine's own player begins a purchase, in a run of several.
    pub(super) fn begin_purchase(&mut self, seat: SeatIndex, spent: V::Number, bought: Bought<V>) {
        if self.players.len() == 1 {
            return;
        }
        let relay = self.relay.get_or_insert_default();
        if let Some(open) = &relay.buying {
            unimplemented!("{open:?} still open as {bought:?} is bought");
        }
        relay.buying = Some(Purchase {
            seat,
            spent,
            bought,
            picks: Vec::new(),
        });
    }

    /// A deck pick this machine's own player made, which the purchase sends
    /// with its results.
    pub(super) fn picked_for_purchase(&mut self, seat: SeatIndex, taken: &[usize]) {
        if let Some(Purchase {
            seat: buyer, picks, ..
        }) = self.relay.as_mut().and_then(|relay| relay.buying.as_mut())
            && *buyer == seat
        {
            picks.push(taken.to_vec());
        }
    }

    /// `TryToProcure` refused the potion, and the purchase sends nothing.
    pub(super) fn purchase_refused(&mut self, seat: SeatIndex) {
        if let Some(relay) = self.relay.as_mut()
            && relay.buying.as_ref().is_some_and(|open| open.seat == seat)
        {
            relay.buying = None;
        }
    }

    /// `OnTryPurchase` returned on this machine: its results go to the
    /// others.
    pub(super) fn purchase_over(&mut self, seat: SeatIndex) {
        let Some(relay) = self.relay.as_mut() else {
            return;
        };
        if relay.buying.as_ref().is_some_and(|open| open.seat == seat) {
            let done = relay.buying.take().expect("an open purchase");
            relay.outbox.push(done);
        }
    }

    /// The purchases this machine finished since it was last asked.
    pub(crate) fn take_sent(&mut self) -> Vec<Purchase<V>> {
        self.relay
            .as_mut()
            .map(|relay| std::mem::take(&mut relay.outbox))
            .unwrap_or_default()
    }

    /// The deck pick a relayed purchase's obtain takes next, as the buyer
    /// made it.
    pub(super) fn synced_pick(&mut self, seat: SeatIndex) -> Option<Vec<usize>> {
        if !self.bought_elsewhere(seat) {
            return None;
        }
        self.relay.as_mut()?.synced.pop_front()
    }

    /// `HandleGoldLostMessage`, then `HandleRewardObtainedMessage`, for
    /// another machine's purchase.
    ///
    /// # Panics
    ///
    /// An obtain that asks something the buyer's picks do not answer.
    pub(crate) fn receive(&mut self, purchase: Purchase<V>) {
        let Purchase {
            seat,
            spent,
            bought,
            picks,
        } = purchase;
        let relay = self.relay.get_or_insert_default();
        relay.elsewhere.retain(|waiting| *waiting != seat);
        relay.synced = picks.into();
        // `PlayerCmd.LoseGold`.
        self.players[seat].gold = self.players[seat].gold - spent;
        match bought {
            Bought::Relic(relic) => {
                let steps = self.obtained_held(seat, relic, false, None);
                if let Some(asked) = self.carry_out(seat, steps) {
                    unimplemented!(
                        "another player's relic asking {:?} on this machine",
                        asked.pickup
                    );
                }
            }
            // `PotionCmd.TryToProcure`, which a full belt refuses here too.
            Bought::Potion(potion) => {
                self.procure(seat, potion);
            }
            // `RunState.AddCard`, then `CardPileCmd.Add` to the deck.
            Bought::Card(card) => self.add_to_deck(seat, card),
        }
        if let Some(relay) = self.relay.as_mut()
            && !relay.synced.is_empty()
        {
            unimplemented!("another player's purchase leaving picks it never asked");
        }
    }
}
