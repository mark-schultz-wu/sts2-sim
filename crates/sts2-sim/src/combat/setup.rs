//! `SetupPlayerTurn` as a coroutine.
//!
//! The setup stops on a question in any of its hooks and in its draws, and
//! goes on from there once the question is answered. `diapause` turns the
//! straight-line setup below into the state it waits in, which
//! [`Live::deferred`](super::live::Live) holds and clones.

use sts2_core::{CombatId, Rules, SeatIndex};

use super::listen::{self, Walking};
use super::live::Live;
use super::turn::{Drew, Listened};
use crate::run::Belts;
use crate::stream::CombatRngs;

/// The fight, as a start or resume of the setup lends it for one step.
///
/// A paused setup waits inside [`Live`], so it keeps none of this.
pub(super) struct Lent<'a, V: Rules> {
    pub(super) live: &'a mut Live<V>,
    pub(super) rngs: CombatRngs<'a>,
    pub(super) belts: &'a mut Belts<V>,
    /// How the answer the setup waited for ended the fight, if it did.
    pub(super) ended: Option<crate::combat::Ended>,
}

impl<V: Rules> Lent<'_, V> {
    pub(super) fn parts(&mut self) -> (&mut Live<V>, CombatRngs<'_>, &mut Belts<V>) {
        (&mut *self.live, self.rngs.reborrow(), &mut *self.belts)
    }
}

/// `SetupPlayerTurn` from inside `Hook.BeforeHandDraw`, whose walk `before`
/// is, to its end, and how it ended the fight if it did.
///
/// It yields on each question it stops on, which it has put in
/// [`Live::asking`](super::live::Live). An answer that ended the fight asks
/// the rest of the walk it stood in and runs nothing more.
#[diapause::coroutine(yield = (), resume = for<'f> Lent<'f, V>)]
#[derive(Debug, Clone, PartialEq)]
pub(super) fn setup<V: Rules>(
    seat: SeatIndex,
    ascension: V::AscensionLevel,
    before: Option<Box<Walking<V>>>,
) -> Option<crate::combat::Ended> {
    let mut walk: Option<Box<Walking<V>>> = before;
    // `Hook.BeforeHandDraw`, once the energy is reset.
    loop {
        let acted: Listened = {
            let (live, rngs, belts) = resumed!().parts();
            crate::combat::turn::walk_before_hand_draw::<V>(
                seat, live, rngs, belts, ascension, &mut walk,
            )
        };
        match acted {
            Listened::Over => break,
            Listened::Done | Listened::Draw(_) => {}
            Listened::Ended(ended) => return Some(ended),
            Listened::Parked => {
                yield_!(());
                if let Some(ended) = resumed!().ended {
                    let (live, _, belts) = resumed!().parts();
                    crate::combat::turn::ask_out_walk::<V>(seat, live, belts, walk);
                    return Some(ended);
                }
            }
        }
    }
    // The turn's hand. A shuffle inside it can ask.
    let drawn: Result<usize, crate::combat::Ended> = {
        let (live, _, belts) = resumed!().parts();
        crate::combat::turn::hand_draw::<V>(seat, live, belts)
    };
    let mut left: usize = match drawn {
        Ok(drawn) => drawn,
        Err(ended) => return Some(ended),
    };
    let mut resumed: bool = false;
    loop {
        let drew: Drew = {
            let (live, rngs, belts) = resumed!().parts();
            crate::combat::turn::draw_in_setup::<V>(
                seat, live, rngs, belts, ascension, left, true, resumed,
            )
        };
        match drew {
            Drew::Done(Some(ended)) => return Some(ended),
            Drew::Done(None) => break,
            Drew::Parked { left: more } => {
                left = more;
                resumed = true;
                yield_!(());
                if let Some(ended) = resumed!().ended {
                    return Some(ended);
                }
            }
        }
    }
    // `Hook.AfterPlayerTurnStart`, the last of the setup. A listener's draw
    // stops the walk until the draw is over, and the walk then goes on.
    let player: CombatId = {
        let (live, _, belts) = resumed!().parts();
        let player = live.player(seat).id;
        let hook = sts2_core::EventHook::PlayerTurnStart {
            player,
            turn: live.turn,
        };
        walk = listen::Walking::begin(live, belts, hook).map(Box::new);
        player
    };
    loop {
        let mut ended: Option<crate::combat::Ended> = None;
        let mut drawing: Option<usize> = None;
        loop {
            let acted: Listened = {
                let (live, rngs, belts) = resumed!().parts();
                crate::combat::turn::walk_after_player_turn_start::<V>(
                    seat, live, rngs, belts, ascension, player, &mut ended, &mut walk,
                )
            };
            match acted {
                Listened::Over => break,
                Listened::Done => {}
                Listened::Draw(count) => {
                    drawing = Some(count);
                    break;
                }
                Listened::Ended(ended) => return Some(ended),
                Listened::Parked => {
                    yield_!(());
                    if let Some(ended) = resumed!().ended {
                        let (live, _, belts) = resumed!().parts();
                        crate::combat::turn::ask_out_walk::<V>(seat, live, belts, walk);
                        return Some(ended);
                    }
                }
            }
        }
        let Some(count) = drawing else {
            // `AfterPlayerTurnStartLate`, once every listener above has run.
            if ended.is_none() {
                let (live, rngs, belts) = resumed!().parts();
                crate::combat::turn::after_player_turn_start_late::<V>(
                    seat, live, rngs, belts, ascension, player,
                );
            }
            return ended;
        };
        left = count;
        resumed = false;
        loop {
            let drew: Drew = {
                let (live, rngs, belts) = resumed!().parts();
                crate::combat::turn::draw_in_setup::<V>(
                    seat, live, rngs, belts, ascension, left, false, resumed,
                )
            };
            match drew {
                Drew::Done(Some(ended)) => return Some(ended),
                Drew::Done(None) => break,
                Drew::Parked { left: more } => {
                    left = more;
                    resumed = true;
                    yield_!(());
                    if let Some(ended) = resumed!().ended {
                        let (live, _, belts) = resumed!().parts();
                        crate::combat::turn::ask_out_walk::<V>(seat, live, belts, walk);
                        return Some(ended);
                    }
                }
            }
        }
    }
}
