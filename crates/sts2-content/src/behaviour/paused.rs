//! What every paused play shares: what it asks, what each step is lent,
//! and how a step's end turns into a [`Played`].
//!
//! A play that stops on a question is a `diapause` coroutine. The fight is
//! lent to it again at each step, so the paused play holds no borrow and
//! clones with the fight.

use crate::behaviour::Played;

/// What a paused play asks, before it is told where it waits.
pub(crate) enum Ask<V: crate::Build> {
    /// A `CardSelectCmd` over a pile.
    Asking {
        pile: sts2_core::Pile,
        selection: sts2_core::Selection,
    },
    /// A `CardSelectCmd` over the cards a filter lets through.
    AskingAmong {
        cards: smallvec::SmallVec<[sts2_core::CardAt<V>; sts2_core::MAX_CARDS_IN_HAND]>,
        selection: sts2_core::Selection,
    },
    /// A choose-a-card screen over `offered`.
    Choosing {
        offered: sts2_core::Offered<V>,
        skippable: bool,
    },
    /// A question one of its commands left, which the fight holds.
    Waiting,
}

impl<V: crate::Build> Ask<V> {
    /// The question, with the play waiting at `at`.
    pub(crate) fn at<K: sts2_core::FightKind<V>>(
        self,
        at: crate::behaviour::Resume<V, K>,
    ) -> Played<V, K> {
        match self {
            Ask::Asking { pile, selection } => Played::Asking {
                pile,
                selection,
                at,
            },
            Ask::AskingAmong { cards, selection } => Played::AskingAmong {
                cards,
                selection,
                at,
            },
            Ask::Choosing { offered, skippable } => Played::Choosing {
                offered,
                skippable,
                at,
            },
            Ask::Waiting => Played::Waiting { at },
        }
    }
}

/// What a paused play is lent at each step: the fight, for that step alone,
/// and the answer it waited for. Nothing on the first step.
pub(crate) struct Lent<'f, 'a, V: crate::Build, K: sts2_core::FightKind<V>> {
    pub(crate) fight: &'f mut K::Fight<'a>,
    pub(crate) taken: Option<sts2_core::Taken<V>>,
}

/// The cards a selection took, which is what `taken` answered.
///
/// # Panics
///
/// An answer to anything but a selection.
pub(crate) fn selected<V: crate::Build>(
    taken: &Option<sts2_core::Taken<V>>,
) -> &sts2_core::SelectedCards<V> {
    match taken {
        Some(sts2_core::Taken::Selected(cards)) => cards,
        taken => unreachable!("a pick, not {taken:?}"),
    }
}

/// The card a choose-a-card screen took, or none where it was skipped.
///
/// # Panics
///
/// An answer to anything but a choice.
pub(crate) fn chosen<V: crate::Build>(
    taken: &Option<sts2_core::Taken<V>>,
) -> Option<sts2_core::CardIdent<V>> {
    match taken {
        Some(sts2_core::Taken::Chosen(card)) => *card,
        taken => unreachable!("a choice, not {taken:?}"),
    }
}

/// `start`, `resume` and `settle` for a module's `paused` coroutine over
/// `$item`, whose paused plays wait in the variant `$wrap`.
macro_rules! paused_play {
    ($item:ty, $wrap:path) => {
        /// A play of [`paused`], from its start.
        fn start<'a, V: crate::Build, K: sts2_core::FightKind<V>>(
            item: $item,
            fight: &mut K::Fight<'a>,
        ) -> crate::behaviour::Played<V, K> {
            let mut play = paused::<V, K>(item);
            let state = play.start_with(crate::behaviour::paused::Lent { fight, taken: None });
            settle::<V, K>(play, state)
        }

        /// A paused play of [`paused`], with the answer it waited for.
        pub(crate) fn resume<'a, V: crate::Build, K: sts2_core::FightKind<V>>(
            fight: &mut K::Fight<'a>,
            mut play: paused::State<V, K>,
            taken: sts2_core::Taken<V>,
        ) -> crate::behaviour::Played<V, K> {
            let state = diapause::Coroutine::resume(
                &mut play,
                crate::behaviour::paused::Lent {
                    fight,
                    taken: Some(taken),
                },
            );
            settle::<V, K>(play, state)
        }

        /// How far a step of `play` got.
        fn settle<V: crate::Build, K: sts2_core::FightKind<V>>(
            play: paused::State<V, K>,
            state: diapause::CoroutineState<crate::behaviour::paused::Ask<V>, ()>,
        ) -> crate::behaviour::Played<V, K> {
            match state {
                diapause::CoroutineState::Complete(()) => crate::behaviour::Played::Done,
                diapause::CoroutineState::Yielded(ask) => ask.at($wrap(play).into()),
            }
        }
    };
}
pub(crate) use paused_play;
