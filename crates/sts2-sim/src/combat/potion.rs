//! Drinking one potion into the fight.
//!
//! `PotionModel.OnUseWrapper`: the potion leaves the belt, then its own effect
//! runs through the board a card acts through. Whether it can be drunk at all
//! is the belt's question, and the run asks it before this.

use sts2_core::{CombatId, Rules};

use super::board::Board;
use super::{Error, Fighting};

/// Drink one potion into the fight where it stands.
///
/// Whatever the drink ended, if it ended the fight.
pub(crate) fn drink<V: Rules>(
    seat: sts2_core::SeatIndex,
    fighting: Fighting<'_, V>,
    potion: V::Potion,
    target: Option<CombatId>,
) -> Result<Option<super::Ended>, Error> {
    let Fighting {
        live,
        streams,
        belts,
    } = fighting;
    let ascension = live.ascension;
    // A potion that names `Self`, or `AnyPlayer` with one player to aim at,
    // is used on its drinker. Its own effect acts for the player it is aimed
    // at, and its drinker applies what it applies.
    let target = target.or_else(|| on_drinker::<V>(potion).then(|| seat.creature()));
    let reaching = super::live::potion_reaching(live, seat, target);
    // `Hook.BeforePotionUsed`, which nothing that listens answers with more
    // than what it writes on itself.
    let hook = sts2_core::EventHook::PotionUsing { target };
    if let Some((listener, effect)) = super::listen::first_answer::<V>(seat, live, belts, hook) {
        unimplemented!("{listener:?} asking for {effect:?} before a potion")
    }
    // `BeginCardOrPotionEffect`.
    live.players[seat].effect_depth += 1;
    let mut board = Board::for_card(seat, live, streams.combat(), belts, ascension, target, None)
        .blocking()
        .reaching(reaching);
    let used = V::on_use::<super::board::BoardKind<V>>(potion, &mut board);
    let ended = board.ended();
    let skipped = std::mem::take(&mut board.skipped);

    super::play::merge::<V>(&mut live.skip_tick, skipped);
    super::play::carry_on(
        seat,
        Fighting {
            live,
            streams,
            belts,
        },
        ascension,
        super::live::Resuming {
            // A drink runs once. `GeneratePlayCount` belongs to a card.
            plays: super::live::Plays {
                done: num_traits::zero(),
                count: num_traits::one(),
            },
            asker: super::live::Asker::Potion(potion),
            target,
            ended,
        },
        used,
        true,
    )
}

/// Whether `potion` is used on its drinker where it is aimed at nobody. Its
/// target type is `Self`, or `AnyPlayer`, which a run of one player aims at
/// that player.
fn on_drinker<V: Rules>(potion: V::Potion) -> bool {
    use sts2_core::{PotionTable as _, TargetKinds as _};

    let target = potion
        .target()
        .unwrap_or_else(|| V::computed_potion_target(potion, true));
    target == V::TargetType::SELF || target == V::TargetType::ANY_PLAYER
}
