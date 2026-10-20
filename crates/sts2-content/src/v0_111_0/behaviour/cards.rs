//! The cards whose rules this build ports for itself, which the shared cards
//! hand over through [`crate::build::OwnRules`].

use sts2_core::{Fight, Played};

use super::super::V0_111_0 as V;
use super::super::generated::cards::{CardVar, Cards, EventCard, IroncladCard, QuestCard};
use crate::generated::runstate::MapPointType;

type Number = i32;

/// What a card of this build's own does when it is played, or nothing for a
/// shared one.
pub(crate) fn on_play<F: Fight<V>>(card: Cards, _fight: &mut F) -> Option<Played<V>> {
    match card {
        // `ExpectAFight` gains its calculated block here.
        Cards::Ironclad(IroncladCard::ExpectAFight) => {
            unimplemented!("Expect a Fight's play in v0.111.0")
        }
        _ => None,
    }
}

/// A number a card of this build's own works out, or nothing for a shared one.
pub(crate) fn calculated<P: sts2_core::Printed<V> + ?Sized>(
    card: Cards,
    _key: CardVar,
    _printed: &P,
    _owner: sts2_core::CardOwner<'_, V>,
) -> Option<Number> {
    match card {
        // `ExpectAFight`: a base, and more for each Strength its owner has.
        Cards::Ironclad(IroncladCard::ExpectAFight) => {
            unimplemented!("Expect a Fight's calculated block in v0.111.0")
        }
        _ => None,
    }
}

/// A deck card of this build's own, by what it saves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OwnCard {
    /// `Dowsing.RoomsEntered`.
    Dowsing { rooms_entered: u8 },
}

/// `Dowsing.maxRooms`: the `?` rooms it waits for.
const DOWSING_ROOMS: u8 = 5;

/// What a deck card of this build's own saves as it is made.
pub(crate) fn own_card_state(card: Cards) -> Option<OwnCard> {
    match card {
        Cards::Quest(QuestCard::Dowsing) => Some(OwnCard::Dowsing { rooms_entered: 0 }),
        _ => None,
    }
}

/// `Dowsing.RoomsEntered`'s setter prints `Rooms` as the rooms it has left.
pub(crate) fn own_card_written(own: OwnCard) -> Option<sts2_core::Change<V>> {
    match own {
        OwnCard::Dowsing { rooms_entered } if rooms_entered > 0 => Some(sts2_core::Change::Raise(
            CardVar::Rooms,
            -Number::from(rooms_entered),
        )),
        OwnCard::Dowsing { .. } => None,
    }
}

/// `EventHook` for a deck card of this build's own.
pub(crate) fn own_deck_card_event_hook(
    card: Cards,
    own: &mut OwnCard,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (own, hook) {
        // `Dowsing.BeforeRoomEntered`: one more `?` room, if it is the map
        // point's first, and an Abundance in its place at five.
        // `PlayerCmd.CompleteQuest` writes the map point's history, which
        // the run here does not keep.
        (
            OwnCard::Dowsing { rooms_entered },
            sts2_core::EventHook::RoomEntering { point, base },
        ) => {
            if base && point == Some(MapPointType::Unknown) {
                *rooms_entered += 1;
                if *rooms_entered >= DOWSING_ROOMS {
                    asked.push(sts2_core::HookEffect::TransformTo(Cards::Event(
                        EventCard::Abundance,
                    )));
                }
            }
        }
        (_, hook) => unimplemented!("what {card:?} in the deck does for {hook:?}"),
    }
}
