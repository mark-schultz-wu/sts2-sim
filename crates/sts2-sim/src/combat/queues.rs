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
    /// The fronts that stopped on a question: whose task each is, and the
    /// player who answers it. Each holds its answerer's queue until the
    /// answer comes, since the game's hook action for a choice stands there.
    /// The other queues go on around it.
    stopped: Vec<(sts2_core::SeatIndex, sts2_core::SeatIndex, Paused<V>)>,
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
    /// `UsePotionAction`: the potion in belt slot `slot`, drunk at `target`
    /// once the actions before it in its owner's queue are over.
    UsePotion {
        slot: usize,
        target: Option<sts2_core::CombatId>,
    },
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

    /// `owner`'s potion in belt slot `slot`, at the end of their queue.
    pub(super) fn enqueue_potion(
        &mut self,
        owner: sts2_core::SeatIndex,
        slot: usize,
        target: Option<sts2_core::CombatId>,
    ) {
        let id = self.next_id;
        self.next_id += 1;
        self.actions.push(Action {
            id,
            owner,
            body: Body::UsePotion { slot, target },
        });
    }

    /// Whether `owner`'s queue holds anything, a front that waits on a
    /// question among it, which an action they take next waits behind.
    pub(super) fn waits(&self, owner: sts2_core::SeatIndex) -> bool {
        self.stopped(owner).is_some() || self.actions.iter().any(|action| action.owner == owner)
    }

    /// `PotionModel.IsQueued` for `owner`'s belt slot `slot`.
    pub(super) fn queued_potion(&self, owner: sts2_core::SeatIndex, slot: usize) -> bool {
        self.actions.iter().any(|action| {
            action.owner == owner
                && matches!(action.body, Body::UsePotion { slot: at, .. } if at == slot)
        })
    }

    /// The first queue in id order whose front is a ready potion, taken off
    /// it: the owner, the belt slot and the target. A queue whose front
    /// stopped on a question has none ready.
    pub(super) fn take_ready_potion(
        &mut self,
    ) -> Option<(sts2_core::SeatIndex, usize, Option<sts2_core::CombatId>)> {
        let at = (0..self.actions.len()).find(|&at| {
            let owner = self.actions[at].owner;
            matches!(self.actions[at].body, Body::UsePotion { .. })
                && self.stopped(owner).is_none()
                && !self.actions[..at]
                    .iter()
                    .any(|before| before.owner == owner)
        })?;
        let Action { owner, body, .. } = self.actions.remove(at);
        match body {
            Body::UsePotion { slot, target } => Some((owner, slot, target)),
            Body::Hook(_) => unreachable!("the action found is a potion"),
        }
    }

    /// The ready hook action with the smallest id, taken off its queue, with
    /// its owner. A queue whose front stopped on a question, or is a potion,
    /// has none ready.
    pub(super) fn take_next(&mut self) -> Option<(sts2_core::SeatIndex, Paused<V>)> {
        let at = (0..self.actions.len()).find(|&at| {
            let owner = self.actions[at].owner;
            matches!(self.actions[at].body, Body::Hook(_))
                && self.stopped(owner).is_none()
                && !self.actions[..at]
                    .iter()
                    .any(|before| before.owner == owner)
        })?;
        let Action { owner, body, .. } = self.actions.remove(at);
        match body {
            Body::Hook(task) => Some((owner, task)),
            Body::UsePotion { .. } => unreachable!("the action found is a hook"),
        }
    }
}

impl<V: Rules> Queues<V> {
    /// `owner`'s `task`, which stopped on `answerer`'s question, at the
    /// front of `answerer`'s queue until the answer comes. Another player's
    /// task waits behind one there already, and is asked next. The game's
    /// hook action for its choice queues behind the first in the same way.
    ///
    /// # Panics
    ///
    /// Where `answerer`'s own action asks while their queue waits on a
    /// question. The game queues that action whole behind the one waiting,
    /// and the sim ran it up to its question already.
    pub(super) fn stop(
        &mut self,
        owner: sts2_core::SeatIndex,
        answerer: sts2_core::SeatIndex,
        task: Paused<V>,
    ) {
        if owner == answerer && self.stopped(answerer).is_some() {
            unimplemented!("an action of {owner:?}'s asking while their queue waits on a question");
        }
        self.stopped.push((owner, answerer, task));
    }

    /// The front of `seat`'s queue that waits on their question, the first
    /// of those that do.
    pub(super) fn stopped(&self, seat: sts2_core::SeatIndex) -> Option<&Paused<V>> {
        self.stopped_with_owner(seat).map(|(_, task)| task)
    }

    /// [`Queues::stopped`], beside whose task it is.
    pub(super) fn stopped_with_owner(
        &self,
        seat: sts2_core::SeatIndex,
    ) -> Option<(sts2_core::SeatIndex, &Paused<V>)> {
        self.stopped
            .iter()
            .find(|(_, answerer, _)| *answerer == seat)
            .map(|(owner, _, task)| (*owner, task))
    }

    /// Every front that waits on a question, beside whose task it is.
    pub(super) fn every_stopped(&self) -> impl Iterator<Item = (sts2_core::SeatIndex, &Paused<V>)> {
        self.stopped.iter().map(|(owner, _, task)| (*owner, task))
    }

    /// Whether any queue's front waits on a question.
    pub(super) fn any_stopped(&self) -> bool {
        !self.stopped.is_empty()
    }

    /// The front stopped last, made the first of those its answerer's queue
    /// waits on.
    pub(super) fn last_to_front(&mut self) {
        if let Some(last) = self.stopped.pop() {
            self.stopped.insert(0, last);
        }
    }

    /// The front of `seat`'s queue that waits on their question, taken off
    /// to go on with the answer, beside whose task it is.
    pub(super) fn go_on(
        &mut self,
        seat: sts2_core::SeatIndex,
    ) -> Option<(sts2_core::SeatIndex, Paused<V>)> {
        let at = (self.stopped.iter()).position(|(_, answerer, _)| *answerer == seat)?;
        let (owner, _, task) = self.stopped.remove(at);
        Some((owner, task))
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
