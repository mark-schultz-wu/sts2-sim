//! What runs on one player's machine alone in a run of several players.
//!
//! `MerchantEntry.OnTryPurchase` runs where the player clicked. Once it
//! returns, with every pick its `RelicCmd.Obtain` asked answered, the
//! buyer's machine sends `RewardSynchronizer`'s `GoldLostMessage` and
//! `RewardObtainedMessage`. Each other machine then loses the gold and
//! obtains the thing for that player itself. A question that obtain asks
//! takes the answer the buyer's `SyncLocalChoice` sent. So each machine
//! keeps what it waits on, and the table hands each machine's messages to
//! the others.
//!
//! A reward screen is not one of these: it syncs each choice with
//! `PlayerChoiceSynchronizer`, and every machine runs it.

use std::collections::VecDeque;

use smallvec::SmallVec;
use sts2_core::{CardIdent, Rules, SeatIndex};

use super::State;

/// What a machine knows of what runs on its own machine and on the others'.
#[derive(Debug, Clone)]
pub(super) struct Relay<V: Rules> {
    /// The purchase this machine's own player is in the middle of, with the
    /// answers its questions took so far.
    open: Option<Open<V>>,
    /// What this machine sent, which the table hands to the other machines.
    outbox: Vec<Message<V>>,
    /// The players whose purchase runs on their own machine and has not come
    /// back.
    elsewhere: SmallVec<[SeatIndex; 4]>,
    /// The answers a relayed obtain's questions take, in the order the
    /// player gave them.
    synced: VecDeque<sts2_core::Answer>,
}

impl<V: Rules> Default for Relay<V> {
    fn default() -> Self {
        Self {
            open: None,
            outbox: Vec::new(),
            elsewhere: SmallVec::new(),
            synced: VecDeque::new(),
        }
    }
}

/// A purchase under way on this machine.
#[derive(Debug, Clone)]
struct Open<V: Rules> {
    seat: SeatIndex,
    spent: V::Number,
    bought: Bought<V>,
    picks: Vec<sts2_core::Answer>,
}

/// One of `RewardSynchronizer`'s messages, from the machine of the player it
/// is about.
#[derive(Debug, Clone)]
pub(crate) enum Message<V: Rules> {
    /// `GoldLostMessage`.
    GoldLost { seat: SeatIndex, gold: V::Number },
    /// `RewardObtainedMessage`, with the answer to each question its obtain
    /// asked, which `SyncLocalChoice` sent.
    Obtained {
        seat: SeatIndex,
        bought: Bought<V>,
        picks: Vec<sts2_core::Answer>,
    },
}

/// What a purchase obtained.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Bought<V: Rules> {
    Relic(V::Relic),
    Potion(V::Potion),
    Card(CardIdent<V>),
}

impl<V: Rules> State<V> {
    /// Whether what `seat` does on their own screen runs on another machine
    /// than this one. A run of one player has no other.
    pub(super) fn bought_elsewhere(&self, seat: SeatIndex) -> bool {
        self.players.len() > 1 && seat != self.local
    }

    /// Another player's purchase, which this machine waits on.
    pub(super) fn wait_on_purchase(&mut self, seat: SeatIndex) {
        self.relay.get_or_insert_default().elsewhere.push(seat);
    }

    /// Whether `decision` is `seat`'s answer inside a purchase that runs on
    /// their own machine, which this one never asked.
    pub(crate) fn answers_elsewhere(
        &self,
        seat: SeatIndex,
        decision: &sts2_core::Decision<V>,
    ) -> bool {
        matches!(decision, sts2_core::Decision::Answer(_))
            && self.bought_elsewhere(seat)
            && (self.relay.as_ref()).is_some_and(|relay| relay.elsewhere.contains(&seat))
    }

    /// This machine's own player begins a purchase, in a run of several.
    pub(super) fn begin_purchase(&mut self, seat: SeatIndex, spent: V::Number, bought: Bought<V>) {
        if self.players.len() == 1 || seat != self.local {
            return;
        }
        let relay = self.relay.get_or_insert_default();
        if let Some(open) = &relay.open {
            unimplemented!("{open:?} still open as {bought:?} is bought");
        }
        relay.open = Some(Open {
            seat,
            spent,
            bought,
            picks: Vec::new(),
        });
    }

    /// An answer this machine's own player gave inside the open purchase,
    /// which it sends with its results.
    pub(super) fn picked_for_purchase(&mut self, seat: SeatIndex, answer: &sts2_core::Answer) {
        if let Some(Open {
            seat: buying,
            picks,
            ..
        }) = self.relay.as_mut().and_then(|relay| relay.open.as_mut())
            && *buying == seat
        {
            picks.push(*answer);
        }
    }

    /// `TryToProcure` refused the potion, and the purchase sends nothing.
    pub(super) fn purchase_refused(&mut self, seat: SeatIndex) {
        if let Some(relay) = self.relay.as_mut()
            && relay.open.as_ref().is_some_and(|open| open.seat == seat)
        {
            relay.open = None;
        }
    }

    /// `OnTryPurchase` returned on this machine: its results go to the
    /// others.
    pub(super) fn purchase_over(&mut self, seat: SeatIndex) {
        let Some(relay) = self.relay.as_mut() else {
            return;
        };
        if relay.open.as_ref().is_some_and(|open| open.seat == seat) {
            let Open {
                seat,
                spent,
                bought,
                picks,
            } = relay.open.take().expect("an open purchase");
            relay.outbox.push(Message::GoldLost { seat, gold: spent });
            relay.outbox.push(Message::Obtained {
                seat,
                bought,
                picks,
            });
        }
    }

    /// What this machine sent since it was last asked.
    pub(crate) fn take_sent(&mut self) -> Vec<Message<V>> {
        self.relay
            .as_mut()
            .map(|relay| std::mem::take(&mut relay.outbox))
            .unwrap_or_default()
    }

    /// `RewardSynchronizer`'s handler for one of another machine's messages.
    ///
    /// # Panics
    ///
    /// An obtain that asks something the buyer never answered.
    pub(crate) fn receive(&mut self, message: Message<V>) {
        match message {
            // `PlayerCmd.LoseGold`, which a purchase sends once it is over.
            Message::GoldLost { seat, gold } => {
                if let Some(relay) = self.relay.as_mut() {
                    relay.elsewhere.retain(|waiting| *waiting != seat);
                }
                self.players[seat].gold = self.players[seat].gold - gold;
            }
            Message::Obtained {
                seat,
                bought,
                picks,
            } => {
                self.relay.get_or_insert_default().synced = picks.into();
                match bought {
                    // Each question its obtain asks takes the answer the
                    // buyer's machine sent.
                    Bought::Relic(relic) => {
                        let steps = self.obtained_held(seat, relic, false, None);
                        let mut asking = self.carry_out(seat, steps);
                        while let Some(super::state::Asked { pickup, mut rest }) = asking {
                            let answer =
                                (self.relay.as_mut()).and_then(|relay| relay.synced.pop_front());
                            let Some(answer) = answer else {
                                unimplemented!(
                                    "another player's relic asking {pickup:?}, which they never answered"
                                );
                            };
                            let lent = super::state::pickup::Lent {
                                run: &mut *self,
                                answer: Some(answer),
                            };
                            asking = match diapause::Coroutine::resume(&mut *rest, lent) {
                                diapause::CoroutineState::Yielded(pickup) => {
                                    Some(super::state::Asked { pickup, rest })
                                }
                                diapause::CoroutineState::Complete(()) => None,
                            };
                        }
                    }
                    // `PotionCmd.TryToProcure`, which a full belt refuses here
                    // too.
                    Bought::Potion(potion) => {
                        self.procure(seat, potion);
                    }
                    // `RunState.AddCard`, then `CardPileCmd.Add` to the deck.
                    Bought::Card(card) => self.add_to_deck(seat, card),
                }
                if let Some(relay) = self.relay.as_mut()
                    && !relay.synced.is_empty()
                {
                    unimplemented!("another player's obtain leaving picks it never asked");
                }
            }
        }
    }
}
