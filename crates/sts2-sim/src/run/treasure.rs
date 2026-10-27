//! `TreasureRoomRelicSynchronizer.AwardRelics`: who gets which of a chest's
//! relics once every player has voted.

use smallvec::SmallVec;
use sts2_core::Rules;
use sts2_core::rng::GameRng as _;

use crate::rng::MegaRng;

/// `RelicPickingFightMove`, in the game's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Move {
    Rock,
    Paper,
    Scissors,
}

/// `RelicPickingFightMove`'s values, which a fight draws one of.
const MOVES: [Move; 3] = [Move::Rock, Move::Paper, Move::Scissors];

/// `RelicPickingResultType`, in the game's order, which the screen sorts
/// the results by before it hands them out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    OnlyOnePlayerVoted,
    FoughtOver,
    ConsolationPrize,
    Skipped,
}

/// The chest's `relics` handed out by `votes`, one for each seat: a
/// relic's index, or nothing for a skip. Each relic beside who gets it, in
/// the order the screen obtains them, and nobody for one skipped.
///
/// A relic one player voted for is theirs. Several fight over it off
/// `rng`. `StableShuffle` deals the rest to the players who neither won one
/// nor skipped, in seat order.
pub(super) fn awarded<V: Rules>(
    relics: &[V::Relic],
    votes: &[Option<u8>],
    rng: &mut MegaRng,
) -> Vec<(Option<sts2_core::SeatIndex>, V::Relic)> {
    use sts2_core::RelicTable as _;

    let seat = |at: usize| sts2_core::SeatIndex(u8::try_from(at).expect("a seat fits a u8"));
    let mut results: Vec<(Kind, Option<sts2_core::SeatIndex>, V::Relic)> = Vec::new();
    let mut unpicked: Vec<V::Relic> = Vec::new();
    for (at, &relic) in relics.iter().enumerate() {
        let voters: SmallVec<[sts2_core::SeatIndex; 4]> = (votes.iter().enumerate())
            .filter(|(_, vote)| vote.map(usize::from) == Some(at))
            .map(|(voter, _)| seat(voter))
            .collect();
        match voters.len() {
            0 => unpicked.push(relic),
            1 => results.push((Kind::OnlyOnePlayerVoted, Some(voters[0]), relic)),
            _ => results.push((Kind::FoughtOver, Some(fight(&voters, rng)), relic)),
        }
    }
    let consoled: SmallVec<[sts2_core::SeatIndex; 4]> = (0..votes.len())
        .map(seat)
        .filter(|&player| {
            let won = results.iter().any(|&(_, winner, _)| winner == Some(player));
            !won && votes[usize::from(player.0)].is_some()
        })
        .collect();
    // `StableShuffle`: sorted by `CompareTo`, then shuffled.
    unpicked.sort_by_key(|relic| relic.order());
    rng.unstable_shuffle(&mut unpicked);
    for (at, relic) in unpicked.into_iter().enumerate() {
        results.push(match consoled.get(at) {
            Some(&player) => (Kind::ConsolationPrize, Some(player), relic),
            None => (Kind::Skipped, None, relic),
        });
    }
    // `List.Sort` by type, which is an insertion sort, and stable, at a
    // chest's few results.
    results.sort_by_key(|&(kind, _, _)| kind);
    results
        .into_iter()
        .map(|(_, winner, relic)| (winner, relic))
        .collect()
}

/// `RelicPickingResult.GenerateRelicFight`: rounds of a move drawn for each
/// player still in, in seat order. A round of exactly two moves knocks out
/// whoever threw the losing one. The last player in wins.
fn fight(players: &[sts2_core::SeatIndex], rng: &mut MegaRng) -> sts2_core::SeatIndex {
    let mut standing: SmallVec<[sts2_core::SeatIndex; 4]> = players.iter().copied().collect();
    while standing.len() > 1 {
        let thrown: SmallVec<[(sts2_core::SeatIndex, Move); 4]> = players
            .iter()
            .filter(|player| standing.contains(player))
            .map(|&player| (player, *rng.choose(&MOVES).expect("there are three moves")))
            .collect();
        let mut distinct: SmallVec<[Move; 3]> = SmallVec::new();
        for &(_, thrown) in &thrown {
            if !distinct.contains(&thrown) {
                distinct.push(thrown);
            }
        }
        let [first, second] = distinct[..] else {
            continue;
        };
        let losing = losing(first, second);
        standing.retain(|player| {
            thrown
                .iter()
                .any(|&(thrower, thrown)| thrower == *player && thrown != losing)
        });
    }
    standing[0]
}

/// `GetLosingMove`: the first loses where the second is the move after it.
fn losing(first: Move, second: Move) -> Move {
    let index = |thrown: Move| {
        MOVES
            .iter()
            .position(|&each| each == thrown)
            .expect("a move")
    };
    match (index(first) + 1) % 3 == index(second) {
        true => first,
        false => second,
    }
}
