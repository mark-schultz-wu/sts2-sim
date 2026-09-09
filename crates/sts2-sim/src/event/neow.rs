//! The blessing the run opens on.
//!
//! A port of `Neow.GenerateInitialOptions`. One curse is drawn, and the
//! blessing that pairs with it comes off the table. Three coin flips add one of
//! a pair each, and two of what is left are shuffled out. The curse goes last,
//! so a run sees two blessings and one price.
//!
//! Off the event's own stream and not the run's. `EventModel.BeginEvent` seeds
//! it from the run's seed, the player's slot and the event's own id, so no
//! draw here moves any counter an observation reports.

use sts2_core::rng::{GameRng, Span, name_hash};
use sts2_core::{ContentVersion, Rules};

/// The relics Neow offers, in the order the room lists them.
///
/// Two blessings then the curse, which is the order `GenerateInitialOptions`
/// builds and not a sort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blessing<V: ContentVersion> {
    pub options: Vec<V::Relic>,
}

impl<V: Rules> Blessing<V> {
    /// The event's own stream, which is `EventModel.BeginEvent`.
    ///
    /// The player's slot index is added in for an event that is not shared, and
    /// Neow is not. A single player sits in slot zero, so this is the run's
    /// seed mixed with the event's id and nothing else.
    pub fn stream<R: GameRng>(seed: u32, slot: u32) -> R {
        R::new(
            seed.wrapping_add(slot)
                .wrapping_add(name_hash("NEOW") as u32),
        )
    }

    /// Roll the three options.
    ///
    /// `players` is how many are in the run, which two of the relic gates ask
    /// about. `rng` is the event's own stream at its start.
    pub fn roll<R: GameRng>(rng: &mut R, players: usize) -> Self {
        let allowed = |relic: V::Relic| V::allowed_at_neow(relic, players);

        let curses: Vec<V::Relic> = V::neow_curses()
            .iter()
            .copied()
            .filter(|&r| allowed(r))
            .collect();
        let Some(curse) = next_item(rng, &curses) else {
            return Blessing {
                options: Vec::new(),
            };
        };

        let mut positives: Vec<V::Relic> = V::neow_positives().to_vec();
        if let Some((_, paired)) = V::neow_pairs().iter().find(|(with, _)| *with == curse) {
            positives.retain(|relic| relic != paired);
        }

        // One of each pair, on a coin flip. The first pair is skipped when the
        // curse drawn is the one that suppresses it, which is why the flip
        // itself is skipped and not just its result.
        for (at, &(heads, tails)) in V::neow_flips().iter().enumerate() {
            if at == 0 && curse == V::neow_suppresses_flip() {
                continue;
            }
            positives.push(if rng.next_bool() { heads } else { tails });
        }

        positives.retain(|&relic| allowed(relic));
        rng.unstable_shuffle(&mut positives);

        let mut options: Vec<V::Relic> = positives.into_iter().take(2).collect();
        options.push(curse);
        Blessing { options }
    }
}

/// One item of a list, which is `Rng.NextItem`.
fn next_item<T: Copy, R: GameRng>(rng: &mut R, items: &[T]) -> Option<T> {
    let span = Span::new(0, i32::try_from(items.len()).ok()?).ok()?;
    items.get(rng.next_int_range(span) as usize).copied()
}
