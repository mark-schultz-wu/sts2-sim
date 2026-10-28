//! What every random run carries, for the census's runner and the recorder.
//!
//! 25 Gorgets and a Mercury Hourglass, so random play reaches content instead
//! of dying early. Both play the same runs, so both carry the same.

use sts2_content::ids::RelicIds as _;

/// The relics a run is granted before it starts, and how many of each.
pub fn carried<V: sts2_content::Build>() -> [(V::Relic, usize); 2] {
    [(V::Relic::GORGET, 25), (V::Relic::MERCURY_HOURGLASS, 1)]
}

/// What a co-op run of `seats` players carries: every player is granted
/// [`carried`], in seat order. Each game makes every grant, for every
/// player, before the first move, and so does each seat's simulator.
#[allow(dead_code, reason = "only the co-op runner and the replay read it")]
pub fn carried_by_every_seat<V: sts2_content::Build>(seats: usize) -> Vec<sts2_core::Setup<V>> {
    (0..seats)
        .map(|at| sts2_core::SeatIndex(u8::try_from(at).expect("at most 256 seats")))
        .flat_map(|seat| {
            carried::<V>().into_iter().flat_map(move |(relic, count)| {
                std::iter::repeat_n(
                    sts2_core::Setup::GrantTo {
                        seat,
                        gift: sts2_core::Gift::Relic(relic),
                    },
                    count,
                )
            })
        })
        .collect()
}
