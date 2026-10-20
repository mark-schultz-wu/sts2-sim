//! The cards whose rules this build ports for itself, which the shared cards
//! hand over through [`crate::build::OwnRules`].

use sts2_core::{Amount, Effect, Fight, Played, Scaling};

use super::super::V0_111_0 as V;
use super::super::generated::cards::{CardVar, Cards, EventCard, IroncladCard, QuestCard};
use super::super::generated::powers::Power;
use crate::generated::runstate::MapPointType;

type Number = i32;

/// What a card of this build's own does when it is played, or nothing for a
/// shared one.
pub(crate) fn on_play<F: Fight<V>>(card: Cards, fight: &mut F) -> Option<Played<V>> {
    match card {
        // `ExpectAFight`: as much block as its `CalculatedBlock`, a move's.
        Cards::Ironclad(IroncladCard::ExpectAFight) => {
            let block = fight.resolve(Effect::GainBlock {
                recipient: fight.actor(),
                amount: Amount::Var(CardVar::CalculatedBlock),
                scaling: Scaling::Dynamic,
            });
            fight.apply(block);
            Some(Played::Done)
        }
        // `Abundance`: `GetDistinctForCombat` of three Powers from its
        // owner's own pool, off `CombatCardGeneration`, each upgraded, and
        // the one chosen into hand, free this turn.
        Cards::Event(EventCard::Abundance) => {
            use sts2_core::{CardKind, Rules as _, RunStreamKinds as _};

            let mut powers: Vec<_> = V::combat_cards(fight.character())
                .iter()
                .copied()
                .filter(|&card| V::card_kind(card) == Some(CardKind::Power))
                .collect();
            let stream = <V as sts2_core::ContentVersion>::RunRngType::COMBAT_CARD_GENERATION;
            let mut offered: [sts2_core::CardIdent<V>; sts2_core::CHOICES] = fight
                .generate_cards(&mut powers, stream)
                .unwrap_or_else(|_| unimplemented!("a pool offering fewer than three Powers"));
            for card in &mut offered {
                card.upgrade = 1;
            }
            Some(Played::Choosing {
                offered: offered.into_iter().collect(),
                skippable: false,
                at: crate::behaviour::cards::Resume::Event(
                    crate::behaviour::cards::event::Resume::AbundanceChoice,
                )
                .into(),
            })
        }
        _ => None,
    }
}

/// A number a card of this build's own works out, or nothing for a shared one.
pub(crate) fn calculated<P: sts2_core::Printed<V> + ?Sized>(
    card: Cards,
    key: CardVar,
    printed: &P,
    owner: sts2_core::CardOwner<'_, V>,
) -> Option<Number> {
    match (card, key) {
        // `ExpectAFight`: a base, and more for each Strength its owner has,
        // none for less than none.
        (Cards::Ironclad(IroncladCard::ExpectAFight), CardVar::CalculatedBlock) => {
            let strength = owner
                .powers
                .iter()
                .find(|held| held.power == Power::StrengthPower)
                .map_or(0, |held| match held.amount {
                    sts2_core::PowerAmount::Counted(amount) => amount,
                    sts2_core::PowerAmount::Endless => unimplemented!("an endless Strength"),
                });
            Some(
                printed.face(CardVar::CalculationBase)
                    + printed.face(CardVar::CalculationExtra) * strength.max(0),
            )
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
