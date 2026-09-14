//! What the front end promises, as claims any implementation can be held to.
//!
//! Screens are checked by rendering fixtures in `shots`. This module checks the
//! decisions produced from a position and key. The implementation is a parameter
//! so the same promises apply to the oracle and simulators.
//!
//! # Running it against something
//!
//! Provide a [`Stage`] that can construct each required position, then call
//! [`run`]. Missing content produces [`Outcome::Unreachable`] rather than a
//! failed behavioral claim.

use std::cell::RefCell;
use std::rc::Rc;

use sts2_core::{
    Answer, Answered, Decision, Play, Question, RewardKind, Room, Snapshot, Start, TakeReward,
};

use super::app::App;
use super::keys::Action;
use super::state::{Options, Tone};

/// A position a check needs the game to be standing in.
///
/// Named for what a player calls what they are looking at. Deliberately coarse.
/// A check that needs a *particular* fight is a check about content, and this
/// suite is about the front end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Position {
    /// A fight, with at least one playable card that takes a target and at
    /// least two creatures to point it at.
    AFight,
    /// A fight with a card in hand that cannot be played.
    AFightWithADeadCard,
    /// A room offering at least two options: an event, a rest, or a chest.
    ARoom,
    /// A room that has been answered, with somewhere to travel to.
    ASpentRoom,
    /// A reward screen with at least one thing still to take.
    RewardsOnOffer,
    /// A belt holding at least one potion that can be drunk and takes a target,
    /// and at least one that cannot be drunk.
    AFightWithABelt,
}

impl Position {
    pub const ALL: [Position; 6] = [
        Position::AFight,
        Position::AFightWithADeadCard,
        Position::ARoom,
        Position::ASpentRoom,
        Position::RewardsOnOffer,
        Position::AFightWithABelt,
    ];
}

/// Something that can put an implementation into a position.
///
/// The oracle reaches one by constructing it. A scripted double is handed one,
/// and a simulator adopts one. What they have in common is all this trait asks
/// for.
pub trait Stage<V: sts2_content_wire::WireContentVersion> {
    type Game: Play<V>;

    /// A game standing in that position, or why it cannot be.
    ///
    /// The reason is reported. A run against a young implementation then reads
    /// as a list of what it cannot do yet, rather than as a wall of failures.
    fn reach(&mut self, position: Position) -> Result<Self::Game, String>;
}

/// Every decision an implementation was told, in order.
///
/// Shared with the [`Spy`] rather than read back off it. The front end owns the
/// game it is driving, and a test cannot reach through it.
#[derive(Debug)]
pub struct Sent<V: sts2_content_wire::WireContentVersion>(Rc<RefCell<Watched<V>>>);

/// What the spy saw: the run it was started with, and every decision since.
///
/// The start is beside the decisions rather than the first of them, because it
/// is not one. It reaches the game through
/// [`Play::begin`](sts2_core::Play::begin), so the spy notes it there.
#[derive(Debug)]
struct Watched<V: sts2_content_wire::WireContentVersion> {
    started: Option<Start<V>>,
    decisions: Vec<Decision<V>>,
}

impl<V: sts2_content_wire::WireContentVersion> Default for Watched<V> {
    fn default() -> Watched<V> {
        Watched {
            started: None,
            decisions: Vec::new(),
        }
    }
}

impl<V: sts2_content_wire::WireContentVersion> Clone for Sent<V> {
    fn clone(&self) -> Sent<V> {
        Sent(Rc::clone(&self.0))
    }
}

impl<V: sts2_content_wire::WireContentVersion> Default for Sent<V> {
    fn default() -> Sent<V> {
        Sent(Rc::new(RefCell::new(Watched::default())))
    }
}

impl<V: sts2_content_wire::WireContentVersion> Sent<V> {
    pub fn all(&self) -> Vec<Decision<V>> {
        self.0.borrow().decisions.clone()
    }

    pub fn count(&self) -> usize {
        self.0.borrow().decisions.len()
    }

    /// The run the game was started with, if it was started through the spy.
    pub fn started(&self) -> Option<Start<V>> {
        self.0.borrow().started.clone()
    }

    /// Writes one decision down, for whatever is watching the game.
    pub fn note(&self, decision: Decision<V>) {
        self.0.borrow_mut().decisions.push(decision);
    }

    /// Writes down the run the game was started with.
    pub(crate) fn note_start(&self, start: &Start<V>) {
        self.0.borrow_mut().started = Some(start.clone());
    }

    /// What was sent after the first `from` decisions, which is what a check
    /// cares about. A stage usually had to send some to reach its position.
    pub fn since(&self, from: usize) -> Vec<Decision<V>> {
        self.0.borrow().decisions[from.min(self.count())..].to_vec()
    }
}

/// An implementation, and a note of everything it was told.
///
/// Wraps rather than replaces. The game underneath is the real one, so a check
/// asserts about a decision that actually happened, rather than one that was
/// intercepted.
pub struct Spy<V: sts2_content_wire::WireContentVersion, G: Play<V>> {
    game: G,
    sent: Sent<V>,
}

impl<V: sts2_content_wire::WireContentVersion, G: Play<V>> Spy<V, G> {
    pub fn new(game: G) -> (Spy<V, G>, Sent<V>) {
        let sent = Sent::default();
        (
            Spy {
                game,
                sent: sent.clone(),
            },
            sent,
        )
    }
}

impl<V: sts2_content_wire::WireContentVersion, G: Play<V>> Play<V> for Spy<V, G> {
    type Error = G::Error;

    /// What the game underneath needs, and where to note what is sent.
    ///
    /// The caller supplies the sink, the way it does when it builds a `Spy`
    /// around a game it already has.
    type Host = (G::Host, Sent<V>);

    fn begin(host: Self::Host, start: &Start<V>) -> Result<Self, Self::Error> {
        let (inner, sent) = host;
        sent.note_start(start);
        Ok(Spy {
            game: G::begin(inner, start)?,
            sent,
        })
    }

    fn play(&mut self, decision: &Decision<V>) -> Result<Answered<V>, Self::Error> {
        self.sent.note(decision.clone());
        self.game.play(decision)
    }

    fn observe(&mut self) -> Result<Snapshot<V>, Self::Error> {
        self.game.observe()
    }
}

/// How one check came out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Passed,
    /// The promise was broken, and here is how.
    Failed(String),
    /// The implementation cannot stand where the check needed it.
    Unreachable(String),
}

#[derive(Debug, Clone)]
pub struct Check {
    pub name: &'static str,
    pub outcome: Outcome,
}

/// Everything this suite claims, run against one implementation.
pub fn run<V, S>(stage: &mut S) -> Vec<Check>
where
    V: sts2_content_wire::WireContentVersion,
    S: Stage<V>,
    <S::Game as Play<V>>::Error: std::error::Error + Send + Sync + 'static,
{
    let mut out = Vec::new();
    for (name, position, check) in claims::<V, S>() {
        let outcome = match stage.reach(position) {
            Err(why) => Outcome::Unreachable(why),
            Ok(game) => {
                let (spy, sent) = Spy::new(game);
                let mut app = App::playing(spy, Options::default());
                app.look_now();
                let before = sent.count();
                check(&mut app, &sent, before)
            }
        };
        out.push(Check { name, outcome });
    }
    out
}

/// The type every check has: drive the front end, then say what happened.
type Claim<V, G> = fn(&mut App<V, G>, &Sent<V>, usize) -> Outcome;

/// One claim, with the name it is reported under and where it has to stand.
type Claimed<V, S> = (
    &'static str,
    Position,
    Claim<V, Spy<V, <S as Stage<V>>::Game>>,
);

fn claims<V, S>() -> Vec<Claimed<V, S>>
where
    V: sts2_content_wire::WireContentVersion,
    S: Stage<V>,
    <S::Game as Play<V>>::Error: std::error::Error + Send + Sync + 'static,
{
    vec![
        (
            "a targeted card is aimed rather than played",
            Position::AFight,
            aiming_sends_nothing,
        ),
        (
            "confirming an aim plays the card at what it is aimed at",
            Position::AFight,
            confirming_an_aim_plays_it,
        ),
        (
            "putting a card down sends nothing",
            Position::AFight,
            cancelling_sends_nothing,
        ),
        (
            "ending the turn ends the turn",
            Position::AFight,
            ending_the_turn,
        ),
        (
            "a key with no meaning does nothing",
            Position::AFight,
            a_meaningless_key_does_nothing,
        ),
        (
            "an unplayable card is refused without asking the game",
            Position::AFightWithADeadCard,
            an_unplayable_card_is_not_sent,
        ),
        (
            "the nth option is what the nth digit takes",
            Position::ARoom,
            a_digit_takes_that_option,
        ),
        (
            "a spent room offers a way out",
            Position::ASpentRoom,
            a_spent_room_can_be_left,
        ),
        (
            "the nth reward is what the nth digit takes",
            Position::RewardsOnOffer,
            a_digit_takes_that_reward,
        ),
        (
            "a targeted potion is aimed rather than drunk",
            Position::AFightWithABelt,
            aiming_a_potion_sends_nothing,
        ),
        (
            "confirming an aimed potion drinks it at what it is aimed at",
            Position::AFightWithABelt,
            confirming_a_potion_drinks_it,
        ),
        (
            "a potion that cannot be drunk is refused without asking the game",
            Position::AFightWithABelt,
            an_undrinkable_potion_is_not_sent,
        ),
    ]
}

// ---------------------------------------------------------------------------
// The claims.
// ---------------------------------------------------------------------------

fn failed(what: impl Into<String>) -> Outcome {
    Outcome::Failed(what.into())
}

/// Confirming a targeted card starts aiming and sends nothing.
///
/// The point of aiming is not choosing. A card with one legal target has
/// nothing to choose. It is that the card shows what it does to *that*
/// creature. A front end that plays on confirm never shows it.
fn aiming_sends_nothing<V, G>(app: &mut App<V, G>, sent: &Sent<V>, before: usize) -> Outcome
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let Some(at) = first_targeted(app) else {
        return Outcome::Unreachable("no card in hand takes a target".into());
    };
    app.press(Action::Nth(at));
    app.press(Action::Confirm);

    if !sent.since(before).is_empty() {
        return failed(format!("it played the card: {:?}", sent.since(before)));
    }
    match app.aimed_at() {
        Some(_) => Outcome::Passed,
        None => failed("it is not aiming at anything"),
    }
}

fn confirming_an_aim_plays_it<V, G>(app: &mut App<V, G>, sent: &Sent<V>, before: usize) -> Outcome
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let Some(at) = first_targeted(app) else {
        return Outcome::Unreachable("no card in hand takes a target".into());
    };
    app.press(Action::Nth(at));
    app.press(Action::Confirm);
    let Some(aimed) = app.aimed_at() else {
        return failed("it did not start aiming");
    };
    app.press(Action::Confirm);

    match sent.since(before).as_slice() {
        [Decision::PlayCard { index, target }] => {
            if usize::from(index.get()) != at {
                return failed(format!("it played card {index}, not {at}"));
            }
            match *target {
                Some(id) if id == aimed => Outcome::Passed,
                other => failed(format!("it aimed at {aimed} and played at {other:?}")),
            }
        }
        other => failed(format!("expected one play_card, got {other:?}")),
    }
}

fn cancelling_sends_nothing<V, G>(app: &mut App<V, G>, sent: &Sent<V>, before: usize) -> Outcome
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let Some(at) = first_targeted(app) else {
        return Outcome::Unreachable("no card in hand takes a target".into());
    };
    app.press(Action::Nth(at));
    app.press(Action::Confirm);
    app.press(Action::Cancel);

    if app.aimed_at().is_some() {
        return failed("it is still aiming");
    }
    match sent.since(before).is_empty() {
        true => Outcome::Passed,
        false => failed(format!("it sent {:?}", sent.since(before))),
    }
}

fn ending_the_turn<V, G>(app: &mut App<V, G>, sent: &Sent<V>, before: usize) -> Outcome
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    app.press(Action::EndTurn);
    match sent.since(before).as_slice() {
        [Decision::EndTurn] => Outcome::Passed,
        other => failed(format!("expected end_turn, got {other:?}")),
    }
}

/// A stray key is silence, not a beep and not a move.
fn a_meaningless_key_does_nothing<V, G>(
    app: &mut App<V, G>,
    sent: &Sent<V>,
    before: usize,
) -> Outcome
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let logged = app.ui.log.len();
    app.press(Action::Mark);
    app.press(Action::Skip);

    if !sent.since(before).is_empty() {
        return failed(format!("it sent {:?}", sent.since(before)));
    }
    match app.ui.log.len() == logged {
        true => Outcome::Passed,
        false => failed("it said something about a key that means nothing here"),
    }
}

/// The game already said why on the card's face. Asking again is a round trip
/// for a line of log nobody needs.
fn an_unplayable_card_is_not_sent<V, G>(
    app: &mut App<V, G>,
    sent: &Sent<V>,
    before: usize,
) -> Outcome
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let Some(at) = first_unplayable(app) else {
        return Outcome::Unreachable("every card in hand can be played".into());
    };
    app.press(Action::Nth(at));
    app.press(Action::Confirm);

    if !sent.since(before).is_empty() {
        return failed(format!(
            "it asked the game anyway: {:?}",
            sent.since(before)
        ));
    }
    match app.ui.log.back().map(|note| note.tone) {
        Some(Tone::Cannot) => Outcome::Passed,
        other => failed(format!("it said nothing about why: {other:?}")),
    }
}

/// The digit beside a row is the digit that takes it, and it takes *that* row.
/// That is the part that breaks when a screen and its keys are numbered by two
/// different pieces of code.
fn a_digit_takes_that_option<V, G>(app: &mut App<V, G>, sent: &Sent<V>, _before: usize) -> Outcome
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    // Worked out from the room, not from `App::moves` — a numbering that is
    // both the expectation and the thing under test is never wrong.
    let Some(room) = app.observation().and_then(|o| o.room.as_ref()) else {
        return Outcome::Unreachable("there is no room here".into());
    };
    let open = still_open(room);
    if open.len() < 2 {
        return Outcome::Unreachable("the room offers fewer than two options".into());
    }

    // Every digit, not one of them. A list drawn backwards agrees with a list
    // drawn forwards about its middle, and about the whole of a list of one.
    for (digit, &index) in open.iter().enumerate().take(9) {
        let at = sent.count();
        app.press(Action::Nth(digit));

        let wanted = Decision::ChooseOption { index };
        match sent.since(at).as_slice() {
            [one] if *one == wanted => {}
            other => {
                return failed(format!(
                    "digit {} sent {other:?}, not {wanted:?}",
                    digit + 1
                ));
            }
        }
    }
    Outcome::Passed
}

/// A room that has been answered still reports itself. If leaving is not a move
/// the run is stuck in it.
fn a_spent_room_can_be_left<V, G>(app: &mut App<V, G>, sent: &Sent<V>, before: usize) -> Outcome
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let moves = app.moves();
    if moves.is_empty() {
        return failed("a spent room offers nothing at all — the run cannot go on");
    }
    app.press(Action::Confirm);

    match sent.since(before).as_slice() {
        [Decision::Travel { .. } | Decision::Proceed] => Outcome::Passed,
        other => failed(format!("expected a way out, got {other:?}")),
    }
}

/// The first digit takes the first thing still on offer.
///
/// The expectation is worked out from the *question*, not from
/// [`App::takeable`]. Asking the front end what it takes, and then checking
/// that it took that, is a claim about nothing. The numbering bug this is here
/// to catch moves both answers together.
fn a_digit_takes_that_reward<V, G>(app: &mut App<V, G>, sent: &Sent<V>, before: usize) -> Outcome
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let Some(Question::Rewards { rewards, .. }) = app.seen().and_then(|a| a.asking.as_ref()) else {
        return Outcome::Unreachable("the game is not offering rewards".into());
    };
    let Some(first) = rewards
        .iter()
        .find(|detail| !detail.taken && detail.populated)
        .map(|detail| match &detail.reward {
            RewardKind::Card { .. } => TakeReward::Card {
                index: detail.index,
                card: 0,
            },
            _ => TakeReward::NonCard {
                index: detail.index,
            },
        })
    else {
        return Outcome::Unreachable("nothing is still on offer".into());
    };
    app.press(Action::Nth(0));

    match sent.since(before).as_slice() {
        [Decision::Answer(Answer::Take(taken))] if taken.as_slice() == [first.clone()] => {
            Outcome::Passed
        }
        other => failed(format!("expected to take {first:?}, got {other:?}")),
    }
}

/// Opening the belt and confirming a targeted potion aims it, and sends
/// nothing.
///
/// The same promise a card gets, and for the same reason. Aiming is when the
/// screen shows which creature the potion is about to land on.
fn aiming_a_potion_sends_nothing<V, G>(
    app: &mut App<V, G>,
    sent: &Sent<V>,
    before: usize,
) -> Outcome
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let Some(at) = first_targeted_potion(app) else {
        return Outcome::Unreachable("no potion in the belt takes a target".into());
    };
    app.press(Action::ShowPotions);
    app.press(Action::Nth(at));

    if !sent.since(before).is_empty() {
        return failed(format!("it drank the potion: {:?}", sent.since(before)));
    }
    match app.drinking() {
        Some(_) => Outcome::Passed,
        None => failed("it is not aiming a potion at anything"),
    }
}

fn confirming_a_potion_drinks_it<V, G>(
    app: &mut App<V, G>,
    sent: &Sent<V>,
    before: usize,
) -> Outcome
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let Some(at) = first_targeted_potion(app) else {
        return Outcome::Unreachable("no potion in the belt takes a target".into());
    };
    app.press(Action::ShowPotions);
    app.press(Action::Nth(at));
    let Some(aimed) = app.aimed_at() else {
        return failed("it did not start aiming");
    };
    app.press(Action::Confirm);

    match sent.since(before).as_slice() {
        [Decision::UsePotion { index, target }] => {
            if usize::from(index.get()) != at {
                return failed(format!("it drank potion {index}, not {at}"));
            }
            match *target {
                Some(id) if id == aimed => Outcome::Passed,
                other => failed(format!("it aimed at {aimed} and drank at {other:?}")),
            }
        }
        other => failed(format!("expected one use_potion, got {other:?}")),
    }
}

/// The belt already said why. Asking the game again is a round trip for a line
/// of log nobody needs.
fn an_undrinkable_potion_is_not_sent<V, G>(
    app: &mut App<V, G>,
    sent: &Sent<V>,
    before: usize,
) -> Outcome
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    let Some(at) = first_undrinkable(app) else {
        return Outcome::Unreachable("every potion in the belt can be drunk".into());
    };
    app.press(Action::ShowPotions);
    app.press(Action::Nth(at));

    if !sent.since(before).is_empty() {
        return failed(format!(
            "it asked the game anyway: {:?}",
            sent.since(before)
        ));
    }
    match app.ui.log.back().map(|note| note.tone) {
        Some(Tone::Cannot) => Outcome::Passed,
        other => failed(format!("it said nothing about why: {other:?}")),
    }
}

// ---------------------------------------------------------------------------
// Reading the position.
// ---------------------------------------------------------------------------

/// The playable card in hand with the fewest targets to point at, and at least
/// one.
///
/// Fewest rather than first, because a card with exactly *one* legal target is
/// where the promise hides. Aiming it chooses nothing, so a front end that
/// plays on confirm looks correct everywhere else. Ties go to the first card.
fn first_targeted<V, G>(app: &App<V, G>) -> Option<usize>
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    app.observation()?
        .fight()?
        .legal
        .iter()
        .enumerate()
        .filter(|(_, card)| card.reason.is_none() && !card.targets.is_empty())
        .min_by_key(|(_, card)| card.targets.len())
        .map(|(at, _)| at)
}

/// Every option in this room that has not been answered, in the room's order.
///
/// Deliberately a second reading of the same rule, rather than a call into the
/// front end's own. The whole claim is that two pieces of code number a list
/// the same way.
fn still_open<V: sts2_content_wire::WireContentVersion>(room: &Room<V>) -> Vec<u8> {
    match room {
        Room::RestSite { options } => options
            .iter()
            .filter(|o| o.enabled)
            .map(|o| o.index)
            .collect(),
        Room::Treasure { options, .. } => options
            .iter()
            .filter(|o| !o.chosen)
            .map(|o| o.index)
            .collect(),
        Room::Event { options, .. } => options
            .iter()
            .filter(|o| !o.locked && !o.chosen)
            .map(|o| o.index)
            .collect(),
        // A shop is bought from rather than chosen from, and `buy` is a
        // different decision. Out of this claim's scope.
        Room::Shop { .. } => Vec::new(),
    }
}

/// The drinkable potion with the fewest targets, and at least one.
///
/// Fewest for the reason [`first_targeted`] is: a potion with exactly one legal
/// target is where a front end that drinks on confirm looks correct.
fn first_targeted_potion<V, G>(app: &App<V, G>) -> Option<usize>
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    app.observation()?
        .potions
        .iter()
        .enumerate()
        .filter(|(_, p)| p.reason.is_none() && !p.targets.is_empty())
        .min_by_key(|(_, p)| p.targets.len())
        .map(|(at, _)| at)
}

fn first_undrinkable<V, G>(app: &App<V, G>) -> Option<usize>
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    app.observation()?
        .potions
        .iter()
        .position(|p| p.reason.is_some())
}

fn first_unplayable<V, G>(app: &App<V, G>) -> Option<usize>
where
    V: sts2_content_wire::WireContentVersion,
    G: Play<V>,
    G::Error: std::error::Error + Send + Sync + 'static,
{
    app.observation()?
        .fight()?
        .legal
        .iter()
        .position(|card| card.reason.is_some())
}

// ---------------------------------------------------------------------------
// The suite, run against fixtures.
// ---------------------------------------------------------------------------
//
// This is the suite proving itself. A scripted stage is not an implementation
// of the game — it says one thing forever — so what it can show is that a claim
// reads the front end rather than the fixture, and that the front end keeps its
// promises against a state the mod could have sent.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixture;
    use crate::scripted::Scripted;
    use serde_json::json;
    use sts2_core::Applied;

    type Version = sts2_content::v0_107_1::V0_107_1;

    /// A stage that stands anywhere, by saying it is standing there.
    ///
    /// Every claim here is about what the front end *sends*, and nothing it
    /// sends depends on what comes back. So a fixed reply is the whole input.
    struct Snapshot;

    impl Stage<Version> for Snapshot {
        type Game = Scripted<Version>;

        fn reach(&mut self, position: Position) -> Result<Scripted<Version>, String> {
            Ok(Scripted::saying(match position {
                Position::AFight | Position::AFightWithADeadCard => a_fight(),
                Position::ARoom => an_event(),
                Position::ASpentRoom => a_spent_room(),
                Position::RewardsOnOffer => rewards(),
                Position::AFightWithABelt => a_fight_with_a_belt(),
            }))
        }
    }

    fn a_fight() -> Applied<Version> {
        fixture::applied(json!({
            "floor": 6, "hp": 61, "inCombat": true,
            "combat": fixture::combat(json!({
                "turn": 3, "energy": 2,
                "hand": ["STRIKE_IRONCLAD", "ARMAMENTS", "DEFEND_IRONCLAD"],
                "creatures": [
                    fixture::creature(0, "Player", Some("CHARACTER/IRONCLAD"), &[]),
                    fixture::creature(1, "Enemy", Some("MONSTER/BOWLBUG_ROCK"), &[]),
                    fixture::creature(2, "Enemy", Some("MONSTER/BOWLBUG_ROCK"), &[]),
                ],
                "legal": [
                    fixture::playable("STRIKE_IRONCLAD", &[1, 2]),
                    fixture::unplayable("ARMAMENTS", "EnergyCostTooHigh"),
                    fixture::playable("DEFEND_IRONCLAD", &[]),
                    // One legal target, which is the case the promise is
                    // about: aiming it chooses nothing and shows everything.
                    fixture::playable("BASH", &[2]),
                ],
            })),
        }))
    }

    fn a_fight_with_a_belt() -> Applied<Version> {
        fixture::applied(json!({
            "floor": 6, "hp": 61, "inCombat": true,
            "potions": [
                {"id": "BLOOD_POTION", "usable": false, "reason": "ItsOwnCheck",
                 "targets": []},
                // One legal target, which is the case the promise is about.
                {"id": "FIRE_POTION", "usable": true, "reason": null,
                 "targets": [2]},
            ],
            "combat": fixture::combat(json!({
                "turn": 3, "energy": 2,
                "hand": ["STRIKE_IRONCLAD"],
                "creatures": [
                    fixture::creature(0, "Player", Some("CHARACTER/IRONCLAD"), &[]),
                    fixture::creature(1, "Enemy", Some("MONSTER/BOWLBUG_ROCK"), &[]),
                    fixture::creature(2, "Enemy", Some("MONSTER/BOWLBUG_ROCK"), &[]),
                ],
                "legal": [fixture::playable("STRIKE_IRONCLAD", &[1, 2])],
            })),
        }))
    }

    fn an_event() -> Applied<Version> {
        fixture::applied(json!({
            "floor": 4,
            "room": {
                "type": "Event", "id": "WELLSPRING", "title": "The Wellspring",
                "options": [
                    {"index": 0, "key": "WELLSPRING.pages.INITIAL.options.DRINK", "title": "Drink"},
                    {"index": 1, "key": "WELLSPRING.pages.INITIAL.options.FILL",
                     "title": "Fill a flask"},
                    {"index": 2, "key": "WELLSPRING.pages.INITIAL.options.PRAY", "title": "Pray"},
                    // A fourth, so that a list drawn backwards is a different
                    // list at every position rather than only at the ends.
                    {"index": 3, "key": "WELLSPRING.pages.INITIAL.options.LEAVE",
                     "title": "Leave", "proceed": true},
                ]
            }
        }))
    }

    /// A room with nothing left to answer, and two ways out of it.
    fn a_spent_room() -> Applied<Version> {
        fixture::applied(json!({
            "floor": 4,
            "map": {
                "at": "1,2",
                "visited": ["1,2"],
                "travelable": [
                    {"at": "0,3", "type": "Monster"},
                    {"at": "2,3", "type": "RestSite"},
                ],
                "points": [
                    {"at": "1,2", "type": "Monster", "to": ["0,3", "2,3"]},
                    {"at": "0,3", "type": "Monster", "to": []},
                    {"at": "2,3", "type": "RestSite", "to": []},
                ],
                "proceed": false
            }
        }))
    }

    fn rewards() -> Applied<Version> {
        fixture::answered(json!({
            "type": "pending", "seq": 9, "cmd": "end_turn", "digest": "AA",
            "kind": "rewards", "skippable": true,
            "rewards": [
                {"index": 0, "type": "GoldReward", "populated": true, "taken": true,
                 "amount": 13},
                {"index": 1, "type": "CardReward", "populated": true, "taken": false,
                 "cards": [
                     {"card": "PERFECTED_STRIKE", "text": "Deal 6 damage."},
                     {"card": "BODY_SLAM", "text": "Deal damage equal to your Block."}
                 ]},
            ],
            "state": {}
        }))
    }

    /// Every promise, against a state the mod can send.
    #[test]
    fn the_front_end_keeps_its_promises() {
        let checks = run::<Version, _>(&mut Snapshot);
        assert_eq!(checks.len(), claims_count(), "every claim ran");

        let broken: Vec<String> = checks
            .iter()
            .filter_map(|check| match &check.outcome {
                Outcome::Failed(why) => Some(format!("{}: {why}", check.name)),
                _ => None,
            })
            .collect();
        assert!(broken.is_empty(), "{}", broken.join("\n"));
    }

    /// A scripted stage stands everywhere, so nothing here is unreachable. An
    /// unreachable claim against *this* stage is a claim that is quietly not
    /// being run.
    #[test]
    fn every_claim_actually_runs() {
        for check in run::<Version, _>(&mut Snapshot) {
            if let Outcome::Unreachable(why) = check.outcome {
                panic!("{} never ran: {why}", check.name);
            }
        }
    }

    fn claims_count() -> usize {
        claims::<Version, Snapshot>().len()
    }
}
