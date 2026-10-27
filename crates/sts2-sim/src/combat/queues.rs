//! The game's `ActionQueueSet`, as far as the hook actions go.
//!
//! The game keeps one queue of actions for each player, and its executor
//! runs the ready front with the smallest id. Ids count up on every
//! enqueue, so the fronts in id order are the actions in the order they
//! were queued. These queues hold them in one list in that order. A seat's
//! queue is its own actions in it, and the next to run is the first.
//! `docs/SCHEDULER.md` has the rest of the model.

use sts2_core::Rules;

use super::live::Paused;

/// Every player's queue of hook actions.
#[derive(Debug, PartialEq)]
pub(super) struct Queues<V: Rules> {
    /// In id order.
    actions: Vec<Action<V>>,
    /// The id the next action takes.
    next_id: u32,
    /// The fronts that stopped on their owner's question, by seat. Each
    /// holds its owner's queue until the answer comes, and the other queues
    /// go on around it.
    stopped: Vec<(sts2_core::SeatIndex, Paused<V>)>,
}

/// One queued action.
#[derive(Debug, Clone, PartialEq)]
struct Action<V: Rules> {
    id: u32,
    /// Whose queue it stands in: the player who answers what it asks.
    owner: sts2_core::SeatIndex,
    body: Body<V>,
}

/// What a queued action runs.
#[derive(Debug, Clone, PartialEq)]
enum Body<V: Rules> {
    /// `GenericHookGameAction`: a listener's task a pause boundary set
    /// aside, which goes on from its question.
    Hook(Paused<V>),
}

impl<V: Rules> Queues<V> {
    pub(super) fn new() -> Self {
        Queues {
            actions: Vec::new(),
            next_id: 0,
            stopped: Vec::new(),
        }
    }

    /// Whether no action waits.
    #[inline]
    pub(super) fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// `task` at the end of `owner`'s queue, as a hook action.
    pub(super) fn enqueue_hook(&mut self, owner: sts2_core::SeatIndex, task: Paused<V>) {
        let id = self.next_id;
        self.next_id += 1;
        self.actions.push(Action {
            id,
            owner,
            body: Body::Hook(task),
        });
    }

    /// The ready front with the smallest id, taken off its queue, with its
    /// owner. A queue whose front stopped on a question has none ready.
    pub(super) fn take_next(&mut self) -> Option<(sts2_core::SeatIndex, Paused<V>)> {
        let at = self
            .actions
            .iter()
            .position(|action| self.stopped.iter().all(|(seat, _)| *seat != action.owner))?;
        let Action { owner, body, .. } = self.actions.remove(at);
        match body {
            Body::Hook(task) => Some((owner, task)),
        }
    }
}

impl<V: Rules> Queues<V> {
    /// `task`, which stopped on `seat`'s question, at the front of `seat`'s
    /// queue until the answer comes.
    pub(super) fn stop(&mut self, seat: sts2_core::SeatIndex, task: Paused<V>) {
        debug_assert!(
            self.stopped(seat).is_none(),
            "a queue stops on one question at a time"
        );
        self.stopped.push((seat, task));
    }

    /// The front of `seat`'s queue that waits on its question.
    pub(super) fn stopped(&self, seat: sts2_core::SeatIndex) -> Option<&Paused<V>> {
        self.stopped
            .iter()
            .find(|(owner, _)| *owner == seat)
            .map(|(_, task)| task)
    }

    /// Whether any queue's front waits on a question.
    pub(super) fn any_stopped(&self) -> bool {
        !self.stopped.is_empty()
    }

    /// The front of `seat`'s queue that waits on its question, taken off to
    /// go on with the answer.
    pub(super) fn go_on(&mut self, seat: sts2_core::SeatIndex) -> Option<Paused<V>> {
        let at = self.stopped.iter().position(|(owner, _)| *owner == seat)?;
        Some(self.stopped.remove(at).1)
    }
}

impl<V: Rules> Clone for Queues<V> {
    fn clone(&self) -> Self {
        Queues {
            actions: self.actions.clone(),
            next_id: self.next_id,
            stopped: self.stopped.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.actions.clone_from(&source.actions);
        self.next_id = source.next_id;
        self.stopped.clone_from(&source.stopped);
    }
}
