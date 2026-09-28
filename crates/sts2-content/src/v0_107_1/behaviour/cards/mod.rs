//! `CardModel.OnPlay`, and the rest of what a card does, ported.
//!
//! `generated/card_plays/` holds every play and upgrade whose statements the
//! generator matched. What is here is the rest: the plays it left to a hand
//! port, and every hook a card overrides.
//!
//! One module per pool, filed the way `generated/cards/` is, so a card's
//! behaviour sits under the same name as its row. A pool gets a module when
//! its first card is ported by hand.
//!
//! [`on_play`] panics for a card neither the generator nor a hand port
//! covers. A card outside both stops the run where it stands rather than
//! playing as if it did nothing.

use sts2_core::{CardVars, Fight, Played};

use super::super::V0_107_1;
use super::super::generated::card_plays;
use super::super::generated::cards::{
    CardVar, Cards, ColorlessCard, CurseCard, DefectCard, IroncladCard, NecrobinderCard, QuestCard,
    RegentCard, SilentCard, TokenCard,
};
use super::super::generated::events::Event;
use super::super::generated::powers::Power;

/// What the run holds a card's numbers as.
type Number = <V0_107_1 as sts2_core::ContentVersion>::Number;

mod colorless;
mod curse;
mod defect;
mod event;
mod ironclad;
mod necrobinder;
mod regent;
mod silent;
mod status;
mod token;

type V = V0_107_1;

/// What a card's first upgrade writes on the copy being upgraded.
///
/// `CardModel.OnUpgrade`, which each card overrides to raise the numbers it
/// prints. Every one the generator matched is generated, and nothing on this
/// build is ported by hand.
///
/// Declared rather than applied: an upgrade writes in the same vocabulary any
/// other effect does, and the fight owns the copy it writes on.
pub(crate) fn upgrade(card: Cards) -> &'static [Change] {
    match card {
        Cards::Ironclad(card) => card_plays::ironclad::upgrade(card),
        Cards::Silent(card) => card_plays::silent::upgrade(card),
        Cards::Regent(card) => card_plays::regent::upgrade(card),
        Cards::Necrobinder(card) => card_plays::necrobinder::upgrade(card),
        Cards::Defect(card) => card_plays::defect::upgrade(card),
        Cards::Colorless(card) => card_plays::colorless::upgrade(card),
        Cards::Curse(card) => card_plays::curse::upgrade(card),
        Cards::Event(card) => card_plays::event::upgrade(card),
        Cards::Quest(card) => card_plays::quest::upgrade(card),
        Cards::Status(card) => card_plays::status::upgrade(card),
        Cards::Token(card) => card_plays::token::upgrade(card),
        Cards::Deprecated(card) => card_plays::deprecated::upgrade(card),
    }
}

/// One thing an effect writes on a copy of a card, as this build spells it.
pub(crate) type Change = sts2_core::Change<V>;

/// What a card that works its own target out from the fight names.
pub(crate) fn computed_target(
    card: Cards,
    powers: &[sts2_core::Stack<V0_107_1>],
) -> super::super::generated::cards::TargetType {
    match card {
        Cards::Token(card) => token::computed_target(card, powers),
        // `Malaise.TargetType`, which names what its constructor does.
        Cards::Silent(SilentCard::Malaise) => super::super::generated::cards::TargetType::AnyEnemy,
        card => unimplemented!("what {card:?} points at"),
    }
}

/// `Normality`'s number of plays a turn.
const NORMALITY_PLAYS: usize = 3;

/// Answer a yes-or-no hook for one card in the fight.
///
/// # Panics
///
/// A card nobody has ported answering one.
pub(crate) fn decide_hook(
    card: Cards,
    pile: Option<sts2_core::Pile>,
    history: sts2_core::History<'_, V0_107_1>,
    hook: sts2_core::DecisionHook<V>,
) -> sts2_core::HookResponse<bool> {
    use sts2_core::{DecisionHook, HookResponse};

    match (card, hook) {
        // `Normality.ShouldPlay`: from the hand, no play once its owner has
        // started three this turn. A fight here has one player, whose plays
        // are all its owner's.
        (Cards::Curse(CurseCard::Normality), DecisionHook::ShouldPlay { .. }) => {
            HookResponse::Decision(
                pile != Some(sts2_core::Pile::Hand)
                    || history.started_this_turn().count() < NORMALITY_PLAYS,
            )
        }
        (card, hook) => unimplemented!("what {card:?} answers for {hook:?}"),
    }
}

/// What a card works out from the fight for a number it prints.
pub(crate) fn calculated<P: sts2_core::Printed<V> + ?Sized>(
    card: Cards,
    key: CardVar,
    printed: &P,
    owner: sts2_core::CardOwner<'_, V0_107_1>,
) -> Number {
    match (card, key) {
        (Cards::Token(card), key) => token::calculated(card, key, printed, owner.powers),
        // `Unleash`: a base, and one more for each point of health a living
        // Osty has.
        (Cards::Necrobinder(NecrobinderCard::Unleash), CardVar::CalculatedDamage) => {
            printed.face(CardVar::CalculationBase)
                + printed.face(CardVar::ExtraDamage) * owner.osty.map_or(0, Number::from)
        }
        // `Protector`: a base, and more for each point of maximum health a
        // living Osty has.
        (Cards::Necrobinder(NecrobinderCard::Protector), CardVar::CalculatedDamage) => {
            printed.face(CardVar::CalculationBase)
                + printed.face(CardVar::ExtraDamage) * owner.osty_max_hp.map_or(0, Number::from)
        }
        // `PreciseCut`: a base, and less for each other card in its owner's
        // hand.
        (Cards::Silent(SilentCard::PreciseCut), CardVar::CalculatedDamage) => {
            let others =
                Number::try_from(owner.hand_besides).expect("a hand fits the game's numbers");
            printed.face(CardVar::CalculationBase) - printed.face(CardVar::ExtraDamage) * others
        }
        // `SoulStorm`: a base, and more for each Soul in its owner's exhaust
        // pile.
        (Cards::Necrobinder(NecrobinderCard::SoulStorm), CardVar::CalculatedDamage) => {
            let souls = owner
                .exhaust
                .iter()
                .filter(|&&card| card == Cards::Token(TokenCard::Soul))
                .count();
            let souls = Number::try_from(souls).expect("an exhaust pile fits the game's numbers");
            printed.face(CardVar::CalculationBase) + printed.face(CardVar::ExtraDamage) * souls
        }
        // `Sacrifice`: a base, and more for each of twice the maximum health a
        // living Osty has.
        (Cards::Necrobinder(NecrobinderCard::Sacrifice), CardVar::CalculatedBlock) => {
            let twice = owner.osty_max_hp.map_or(0, |hp| Number::from(hp) * 2);
            printed.face(CardVar::CalculationBase) + printed.face(CardVar::CalculationExtra) * twice
        }
        // `AshenStrike`: a base, and more for each card in its owner's
        // exhaust pile.
        (Cards::Ironclad(IroncladCard::AshenStrike), CardVar::CalculatedDamage) => {
            let exhausted = Number::try_from(owner.exhaust.len())
                .expect("an exhaust pile fits the game's numbers");
            printed.face(CardVar::CalculationBase) + printed.face(CardVar::ExtraDamage) * exhausted
        }
        // `Mirage`: a base, and one more for each Poison on the living
        // enemies.
        (Cards::Silent(SilentCard::Mirage), CardVar::CalculatedBlock) => {
            let poison: Number = owner
                .enemy_powers
                .iter()
                .filter(|stack| stack.power == Power::PoisonPower)
                .map(|stack| match stack.amount {
                    sts2_core::PowerAmount::Counted(amount) => amount,
                    sts2_core::PowerAmount::Endless => unimplemented!("an endless Poison"),
                })
                .sum();
            printed.face(CardVar::CalculationBase)
                + printed.face(CardVar::CalculationExtra) * poison
        }
        // `Bully`: a base, and more for each Vulnerable on its target.
        (Cards::Ironclad(IroncladCard::Bully), CardVar::CalculatedDamage) => {
            let vulnerable = owner
                .target
                .and_then(|powers| {
                    powers
                        .iter()
                        .find(|stack| stack.power == Power::VulnerablePower)
                })
                .map_or(0, |stack| match stack.amount {
                    sts2_core::PowerAmount::Counted(amount) => amount,
                    sts2_core::PowerAmount::Endless => unimplemented!("an endless Vulnerable"),
                });
            printed.face(CardVar::CalculationBase) + printed.face(CardVar::ExtraDamage) * vulnerable
        }
        // `Normality`: a base, and one fewer card for each play its owner
        // started this turn, up to three.
        (Cards::Curse(CurseCard::Normality), CardVar::CalculatedCards) => {
            let started = owner
                .history
                .started_this_turn()
                .count()
                .min(NORMALITY_PLAYS);
            let started = Number::try_from(started).expect("three fits the game's numbers");
            printed.face(CardVar::CalculationBase)
                + printed.face(CardVar::CalculationExtra) * started
        }
        // `BodySlam`: a base, and more for each point of its owner's block.
        (Cards::Ironclad(IroncladCard::BodySlam), CardVar::CalculatedDamage) => {
            printed.face(CardVar::CalculationBase)
                + printed.face(CardVar::ExtraDamage) * owner.block
        }
        // `Barrage`: a base, and one more hit for each orb its owner holds.
        (Cards::Defect(DefectCard::Barrage), CardVar::CalculatedHits) => {
            let orbs =
                Number::try_from(owner.orbs.len()).expect("an orb queue fits the game's numbers");
            printed.face(CardVar::CalculationBase) + printed.face(CardVar::CalculationExtra) * orbs
        }
        // `CompileDriver`: a base, and one more card for each kind of orb its
        // owner holds.
        (Cards::Defect(DefectCard::CompileDriver), CardVar::CalculatedCards) => {
            let mut kinds: smallvec::SmallVec<[_; 10]> = smallvec::SmallVec::new();
            for held in owner.orbs {
                if !kinds.contains(&held.orb) {
                    kinds.push(held.orb);
                }
            }
            let kinds =
                Number::try_from(kinds.len()).expect("an orb queue fits the game's numbers");
            printed.face(CardVar::CalculationBase) + printed.face(CardVar::CalculationExtra) * kinds
        }
        // `PerfectedStrike`: a base, and more for each of its owner's cards in
        // the fight tagged Strike, itself included.
        (Cards::Ironclad(IroncladCard::PerfectedStrike), CardVar::CalculatedDamage) => {
            let strikes =
                Number::try_from(owner.strikes).expect("a fight's cards fit the game's numbers");
            printed.face(CardVar::CalculationBase) + printed.face(CardVar::ExtraDamage) * strikes
        }
        // `MindBlast`: a base, and one more for each card in the draw pile.
        (Cards::Colorless(ColorlessCard::MindBlast), CardVar::CalculatedDamage) => {
            let draw =
                Number::try_from(owner.draw).expect("a draw pile's size fits the game's numbers");
            printed.face(CardVar::CalculationBase) + printed.face(CardVar::ExtraDamage) * draw
        }
        // `LunarBlast`: a base, and one more hit for each Skill its owner's
        // plays finished this turn. A fight here has one player.
        (Cards::Regent(RegentCard::LunarBlast), CardVar::CalculatedHits) => {
            let skills = owner
                .history
                .finished_this_turn()
                .filter(|&card| {
                    <V0_107_1 as sts2_core::Rules>::card_kind(card)
                        == Some(sts2_core::CardKind::Skill)
                })
                .count();
            let skills = Number::try_from(skills).expect("a turn's plays fit the game's numbers");
            printed.face(CardVar::CalculationBase)
                + printed.face(CardVar::CalculationExtra) * skills
        }
        // `Finisher`: a base, and one more hit for each attack its owner's
        // plays finished this turn. A fight here has one player.
        (Cards::Silent(SilentCard::Finisher), CardVar::CalculatedHits) => {
            let attacks = owner
                .history
                .finished_this_turn()
                .filter(|&card| {
                    <V0_107_1 as sts2_core::Rules>::card_kind(card)
                        == Some(sts2_core::CardKind::Attack)
                })
                .count();
            let attacks = Number::try_from(attacks).expect("a turn's plays fit the game's numbers");
            printed.face(CardVar::CalculationBase)
                + printed.face(CardVar::CalculationExtra) * attacks
        }
        // `NoEscape`: a base, and more for each whole `DoomThreshold` of Doom
        // on its target.
        (Cards::Necrobinder(NecrobinderCard::NoEscape), CardVar::CalculatedDoom) => {
            let doom = owner
                .target
                .and_then(|powers| powers.iter().find(|stack| stack.power == Power::DoomPower))
                .map_or(0, |stack| match stack.amount {
                    sts2_core::PowerAmount::Counted(amount) => amount,
                    sts2_core::PowerAmount::Endless => unimplemented!("an endless Doom"),
                });
            printed.face(CardVar::CalculationBase)
                + printed.face(CardVar::CalculationExtra)
                    * (doom / printed.face(CardVar::DoomThreshold))
        }
        // `Radiate`: a base, and one more hit for each star its owner gained
        // this turn. A fight here has one player.
        (Cards::Regent(RegentCard::Radiate), CardVar::CalculatedHits) => {
            let gained: Number = owner
                .history
                .stars_modified_this_turn()
                .filter(|&amount| amount > 0)
                .sum();
            printed.face(CardVar::CalculationBase)
                + printed.face(CardVar::CalculationExtra) * gained
        }
        (card, key) => unimplemented!("the {} {card:?} works out", key.name()),
    }
}

/// Where a card's own play stops to ask something, under its card grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resume {
    Colorless(colorless::Resume),
    Defect(defect::Resume),
    Event(event::Resume),
    Ironclad(ironclad::Resume),
    Necrobinder(necrobinder::Resume),
    Regent(regent::Resume),
    Silent(silent::Resume),
}

/// What a card does when it is played.
#[tracing::instrument(level = "trace", skip_all, fields(card = ?card))]
pub(crate) fn on_play<F: Fight<V>>(card: Cards, fight: &mut F) -> Played<V> {
    match card {
        Cards::Ironclad(card) => ironclad::on_play(card, fight),
        Cards::Silent(card) => silent::on_play(card, fight),
        Cards::Regent(card) => regent::on_play(card, fight),
        Cards::Necrobinder(card) => necrobinder::on_play(card, fight),
        Cards::Defect(card) => defect::on_play(card, fight),
        Cards::Colorless(card) => colorless::on_play(card, fight),
        Cards::Curse(card) => card_plays::curse::on_play(card, fight),
        Cards::Event(card) => event::on_play(card, fight),
        Cards::Quest(card) => card_plays::quest::on_play(card, fight),
        Cards::Status(card) => status::on_play(card, fight),
        Cards::Token(card) => token::on_play(card, fight),
        Cards::Deprecated(card) => card_plays::deprecated::on_play(card, fight),
    }
}

/// What a card still in the hand does when its owner's turn ends.
pub(crate) fn on_turn_end<F: Fight<V>>(card: Cards, fight: &mut F) {
    match card {
        Cards::Ironclad(card) => card_plays::ironclad::on_turn_end(card, fight),
        Cards::Silent(card) => card_plays::silent::on_turn_end(card, fight),
        Cards::Regent(card) => card_plays::regent::on_turn_end(card, fight),
        Cards::Necrobinder(card) => card_plays::necrobinder::on_turn_end(card, fight),
        Cards::Defect(card) => card_plays::defect::on_turn_end(card, fight),
        Cards::Colorless(card) => card_plays::colorless::on_turn_end(card, fight),
        Cards::Curse(card) => curse::on_turn_end(card, fight),
        Cards::Event(card) => card_plays::event::on_turn_end(card, fight),
        Cards::Quest(card) => card_plays::quest::on_turn_end(card, fight),
        Cards::Status(card) => status::on_turn_end(card, fight),
        Cards::Token(card) => card_plays::token::on_turn_end(card, fight),
        Cards::Deprecated(card) => card_plays::deprecated::on_turn_end(card, fight),
    }
}

/// Deliver a notification to one card in the fight.
///
/// `printed` is what this copy of the card shows, which is what the model
/// reads its own numbers off.
pub(crate) fn event_hook<P: sts2_core::Printed<V> + ?Sized>(
    card: Cards,
    id: sts2_core::CardId<V>,
    pile: Option<sts2_core::Pile>,
    printed: &P,
    history: sts2_core::History<'_, V>,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // The fight asks only a card whose class overrides the hook.
    match card {
        // `Guilty.AfterCombatEnd` counts on the copy in the deck, and a copy
        // in the fight's piles does nothing.
        Cards::Curse(CurseCard::Guilty)
            if matches!(hook, sts2_core::EventHook::CombatEnd { .. }) => {}
        Cards::Status(card) => status::event_hook(card, id, printed, hook, asked),
        Cards::Curse(card) => curse::event_hook(card, pile, hook, asked),
        Cards::Ironclad(card) => ironclad::event_hook(card, id, history, hook, asked),
        Cards::Colorless(card) => colorless::event_hook(card, id, history, hook, asked),
        // `SovereignBlade.AfterCardChangedPiles` plays the forge's effects and
        // removes its node, and its `AfterTransformedFrom` removes the node
        // too. Neither touches the fight.
        Cards::Token(TokenCard::SovereignBlade)
            if matches!(hook, sts2_core::EventHook::CardChangedPiles { .. }) => {}
        Cards::Necrobinder(card) => {
            necrobinder::event_hook(card, id, printed, history, hook, asked)
        }
        Cards::Regent(card) => regent::event_hook(card, id, printed, history, hook, asked),
        Cards::Silent(card) => silent::event_hook(card, id, history, hook, asked),
        card => unimplemented!("what {card:?} does for {hook:?}"),
    }
}

/// What a deck card's own model saves between fights: its `[SavedProperty]`
/// fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State {
    Stateless,
    /// `Guilty.CombatsSeen`.
    Guilty {
        combats_seen: u8,
    },
}

impl sts2_core::CardModel<V> for State {
    fn new(card: Cards) -> Self {
        match card {
            Cards::Curse(CurseCard::Guilty) => Self::Guilty { combats_seen: 0 },
            _ => Self::Stateless,
        }
    }

    /// `Guilty.CombatsSeen`'s setter prints `Combats` as the fights it has
    /// left.
    fn written(&self) -> Option<Change> {
        match *self {
            Self::Guilty { combats_seen } if combats_seen > 0 => {
                Some(Change::Raise(CardVar::Combats, -Number::from(combats_seen)))
            }
            _ => None,
        }
    }
}

/// `Guilty.maxCombats`: the fights it stays in the deck for.
const GUILTY_COMBATS: u8 = 5;

/// What a card in the run's deck asks for when `hook` reaches it.
///
/// The run asks only a card whose class overrides the hook.
///
/// # Panics
///
/// An override nobody has ported.
pub(crate) fn deck_event_hook(
    card: Cards,
    state: &mut State,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (card, state, hook) {
        // `Guilty.AfterCombatEnd`: one more fight seen, and out of the deck at
        // five.
        (
            Cards::Curse(CurseCard::Guilty),
            State::Guilty { combats_seen },
            sts2_core::EventHook::CombatEnd { .. },
        ) => {
            *combats_seen += 1;
            if *combats_seen >= GUILTY_COMBATS {
                asked.push(sts2_core::HookEffect::RemoveFromDeck);
            }
        }
        (card, _, hook) => unimplemented!("what {card:?} in the deck does for {hook:?}"),
    }
}

/// The rest of a play that stopped, once the fight has its answer.
pub(crate) fn resume_play<F: Fight<V>>(
    fight: &mut F,
    at: Resume,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V> {
    match at {
        Resume::Colorless(at) => colorless::resume_play(fight, at, chosen),
        Resume::Defect(at) => defect::resume_play(fight, at, chosen),
        Resume::Event(at) => event::resume_play(fight, at, chosen),
        Resume::Ironclad(at) => ironclad::resume_play(fight, at, chosen),
        Resume::Necrobinder(at) => necrobinder::resume_play(fight, at, chosen),
        Resume::Regent(at) => regent::resume_play(fight, at, chosen),
        Resume::Silent(at) => silent::resume_play(fight, at, chosen),
    }
}

/// The act, counted from zero, in which the Lantern Key answers.
const LANTERN_KEY_ACT: usize = 2;

/// `ModifyUnknownMapPointRoomTypes` for a card in the deck. The Lantern Key
/// leaves nothing but an event in its act.
///
/// # Panics
///
/// Any other card, which the run asks only when its class overrides the hook.
pub(crate) fn narrows_unknown_rooms(card: Cards, act: usize) -> bool {
    match card {
        Cards::Quest(QuestCard::LanternKey) => act == LANTERN_KEY_ACT,
        card => unimplemented!("what {card:?} does to an unknown room"),
    }
}

/// `ModifyNextEvent` for a card in the deck. The Lantern Key sends its act's
/// next event to War Historian Repy.
///
/// # Panics
///
/// Any other card, which the run asks only when its class overrides the hook.
pub(crate) fn next_event(card: Cards, act: usize, event: Event) -> Event {
    match card {
        Cards::Quest(QuestCard::LanternKey) if act == LANTERN_KEY_ACT => Event::WarHistorianRepy,
        Cards::Quest(QuestCard::LanternKey) => event,
        card => unimplemented!("what {card:?} does to the next event"),
    }
}
