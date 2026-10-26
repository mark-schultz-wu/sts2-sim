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

use crate::behaviour::Played;
use sts2_core::{CardVars, Fight};

use crate::ids::{CardIds as _, CardVarIds as _, EventIds as _, OrbIds as _, PowerIds as _};

/// What the run holds a card's numbers as.
type Number = i32;

mod colorless;
mod curse;
pub(crate) mod defect;
pub(crate) mod event;
pub(crate) mod ironclad;
pub(crate) mod necrobinder;
pub(crate) mod regent;
pub(crate) mod silent;
mod status;
mod token;

/// The pool a card is filed under, which is the `Cards` member holding it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardFiled {
    Ironclad,
    Silent,
    Regent,
    Necrobinder,
    Defect,
    Colorless,
    Curse,
    Event,
    Quest,
    Status,
    Token,
    Deprecated,
}

/// What a card's first upgrade writes on the copy being upgraded.
///
/// `CardModel.OnUpgrade`, which each card overrides to raise the numbers it
/// prints. Every one the generator matched is generated, and nothing on this
/// build is ported by hand.
///
/// Declared rather than applied: an upgrade writes in the same vocabulary any
/// other effect does, and the fight owns the copy it writes on.
pub(crate) fn upgrade<V: crate::Build>(card: V::Card) -> &'static [sts2_core::Change<V>] {
    V::generated_upgrade(card)
}

/// Where `card` stands now, found again by its id, or nothing where no pile
/// holds it. A card auto-played more than once moves between its plays, and
/// `CardCmd.AutoPlay` plays it wherever it is.
pub(crate) fn standing<V: crate::Build, F: Fight<V>>(
    fight: &F,
    card: sts2_core::CardAt<V>,
) -> Option<sts2_core::CardAt<V>> {
    use sts2_core::Pile;

    [Pile::Discard, Pile::Hand, Pile::Draw, Pile::Exhaust]
        .into_iter()
        .find_map(|pile| {
            fight
                .cards_in(pile)
                .into_iter()
                .find(|at| at.id() == card.id())
        })
}

/// What a card that works its own target out from the fight names.
pub(crate) fn computed_target<V: crate::Build>(
    card: V::Card,
    powers: &[sts2_core::Stack<V>],
) -> crate::generated::cards::TargetType {
    match card {
        card if V::card_filed(card) == CardFiled::Token => {
            token::computed_target::<V>(card, powers)
        }
        // `Malaise.TargetType`, which names what its constructor does.
        __c0 if __c0 == V::Card::MALAISE => crate::generated::cards::TargetType::AnyEnemy,
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
/// Out of line: inlined into the walk, it slows every decision the walk asks.
#[inline(never)]
pub(crate) fn decide_hook<V: crate::Build>(
    card: V::Card,
    pile: Option<sts2_core::Pile>,
    history: sts2_core::History<'_, V>,
    hook: sts2_core::DecisionHook<V>,
) -> sts2_core::HookResponse<bool> {
    use sts2_core::{DecisionHook, HookResponse};

    match (card, hook) {
        // `Normality.ShouldPlay`: from the hand, no play once its owner has
        // started three this turn. A fight here has one player, whose plays
        // are all its owner's.
        (__c0, DecisionHook::ShouldPlay { .. }) if __c0 == V::Card::NORMALITY => {
            HookResponse::Decision(
                pile != Some(sts2_core::Pile::Hand)
                    || history.started_this_turn().count() < NORMALITY_PLAYS,
            )
        }
        // `Enthralled.ShouldPlay`: from the hand, nothing but itself is played
        // by hand. An auto-play goes ahead.
        (__c0, DecisionHook::ShouldPlay { card, auto, .. }) if __c0 == V::Card::ENTHRALLED => {
            HookResponse::Decision(
                pile != Some(sts2_core::Pile::Hand) || card == V::Card::ENTHRALLED || auto,
            )
        }
        (card, hook) => unimplemented!("what {card:?} answers for {hook:?}"),
    }
}

/// `CardModel.IsPlayable`, which this build's `GrandFinale`, `Clash` and
/// `HighFive` override.
pub(crate) fn is_playable<V: crate::Build>(
    card: V::Card,
    fight: &impl sts2_core::CardLogic,
) -> sts2_core::HookResponse<bool> {
    match card {
        // `GrandFinale`: only once its owner's draw pile is empty.
        __c0 if __c0 == V::Card::GRAND_FINALE => {
            sts2_core::HookResponse::Decision(fight.draw_pile() == 0)
        }
        // `HighFive`: only while its owner's Osty stands.
        __c0 if __c0 == V::Card::HIGH_FIVE => sts2_core::HookResponse::Decision(fight.osty()),
        // `Clash`: only while every card in its owner's hand is an Attack.
        __c0 if __c0 == V::Card::CLASH => {
            sts2_core::HookResponse::Decision(fight.hand_all_attacks())
        }
        _ => sts2_core::HookResponse::DoesNotImplement,
    }
}

/// `GetResultPileTypeForCardPlay`, which this build's `ParticleWall` alone
/// overrides.
pub(crate) fn result_pile<V: crate::Build>(
    card: V::Card,
    base: (Option<sts2_core::Pile>, sts2_core::PilePosition),
) -> sts2_core::HookResponse<(Option<sts2_core::Pile>, sts2_core::PilePosition)> {
    use sts2_core::{HookResponse, Pile};

    if let Some(own) = V::own_card_result_pile(card, base) {
        return own;
    }
    match card {
        // `ParticleWall`: back into the hand where the base class discards.
        __c0 if __c0 == V::Card::PARTICLE_WALL => HookResponse::Decision(match base {
            (Some(Pile::Discard), position) => (Some(Pile::Hand), position),
            base => base,
        }),
        _ => HookResponse::DoesNotImplement,
    }
}

/// `PowerModel.TypeForCurrentAmount == PowerType.Debuff`, which
/// [`crate::behaviour::powers::debuff_at`] answers. An endless power's amount is the
/// game's -1.
pub(crate) fn debuff_for_amount<V: crate::Build>(
    power: V::Power,
    amount: sts2_core::PowerAmount<i32>,
) -> bool {
    let amount = match amount {
        sts2_core::PowerAmount::Counted(amount) => amount,
        sts2_core::PowerAmount::Endless => -1,
    };
    crate::behaviour::powers::debuff_at::<V>(power, amount)
}

/// What a card works out from the fight for a number it prints.
pub(crate) fn calculated<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    card: V::Card,
    key: V::CardVar,
    printed: &P,
    owner: sts2_core::CardOwner<'_, V>,
) -> Number {
    // A card whose number is the build's own.
    if let Some(number) = V::own_card_calculated(card, key, printed, owner) {
        return number;
    }
    match (card, key) {
        (card, key) if V::card_filed(card) == CardFiled::Token => {
            token::calculated::<V, _>(card, key, printed, owner.powers)
        }
        // `Rend`: a base, and more for each debuff on its target that is not
        // temporary.
        (__c0, __c1) if (__c0 == V::Card::REND && __c1 == V::CardVar::CALCULATED_DAMAGE) => {
            let debuffs = owner.target.map_or(0, |powers| {
                powers
                    .iter()
                    // A temporary power's type is worked out as it stands, and
                    // Rend leaves it out whatever it is.
                    .filter(|held| {
                        crate::behaviour::powers::lends::<V>(held.power).is_none()
                            && debuff_for_amount::<V>(held.power, held.amount)
                    })
                    .count()
            });
            let debuffs =
                Number::try_from(debuffs).expect("a creature's powers fit the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::EXTRA_DAMAGE) * debuffs
        }
        // `Unleash`: a base, and one more for each point of health a living
        // Osty has.
        (__c0, __c1) if (__c0 == V::Card::UNLEASH && __c1 == V::CardVar::CALCULATED_DAMAGE) => {
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::EXTRA_DAMAGE) * owner.osty.map_or(0, Number::from)
        }
        // `Protector`: a base, and more for each point of maximum health a
        // living Osty has.
        (__c0, __c1) if (__c0 == V::Card::PROTECTOR && __c1 == V::CardVar::CALCULATED_DAMAGE) => {
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::EXTRA_DAMAGE) * owner.osty_max_hp.map_or(0, Number::from)
        }
        // `Murder`: a base, and more for each card its owner drew this fight.
        // A fight here has one player, who draws every card drawn.
        (__c0, __c1) if (__c0 == V::Card::MURDER && __c1 == V::CardVar::CALCULATED_DAMAGE) => {
            let drawn = Number::try_from(owner.history.drawn_this_fight())
                .expect("a fight's draws fit the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::EXTRA_DAMAGE) * drawn
        }
        // `PreciseCut`: a base, and less for each other card in its owner's
        // hand.
        (__c0, __c1) if (__c0 == V::Card::PRECISE_CUT && __c1 == V::CardVar::CALCULATED_DAMAGE) => {
            let others =
                Number::try_from(owner.hand_besides).expect("a hand fits the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                - printed.face(V::CardVar::EXTRA_DAMAGE) * others
        }
        // `SoulStorm`: a base, and more for each Soul in its owner's exhaust
        // pile.
        (__c0, __c1) if (__c0 == V::Card::SOUL_STORM && __c1 == V::CardVar::CALCULATED_DAMAGE) => {
            let souls = owner
                .exhaust
                .iter()
                .filter(|&&card| card == V::Card::SOUL)
                .count();
            let souls = Number::try_from(souls).expect("an exhaust pile fits the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::EXTRA_DAMAGE) * souls
        }
        // `Rattle`: a base, and more for each of one and the attacks Osty made
        // this turn. A fight here has one player, whose Osty every Osty is.
        (__c0, __c1) if (__c0 == V::Card::RATTLE && __c1 == V::CardVar::CALCULATED_HITS) => {
            let attacks = owner
                .history
                .attacked_this_turn()
                .filter(|&monster| monster == Some(<V::Monster as crate::ids::MonsterIds>::OSTY))
                .count();
            let hits =
                Number::try_from(1 + attacks).expect("a turn's attacks fit the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * hits
        }
        // `Sacrifice`: a base, and more for each of twice the maximum health a
        // living Osty has.
        (__c0, __c1) if (__c0 == V::Card::SACRIFICE && __c1 == V::CardVar::CALCULATED_BLOCK) => {
            let twice = owner.osty_max_hp.map_or(0, |hp| Number::from(hp) * 2);
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * twice
        }
        // `Stack`: a base, and more for each card in its owner's discard pile.
        (__c0, __c1) if (__c0 == V::Card::STACK && __c1 == V::CardVar::CALCULATED_BLOCK) => {
            let discard =
                Number::try_from(owner.discard).expect("a discard pile fits the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * discard
        }
        // `AshenStrike`: a base, and more for each card in its owner's
        // exhaust pile.
        (__c0, __c1)
            if (__c0 == V::Card::ASHEN_STRIKE && __c1 == V::CardVar::CALCULATED_DAMAGE) =>
        {
            let exhausted = Number::try_from(owner.exhaust.len())
                .expect("an exhaust pile fits the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::EXTRA_DAMAGE) * exhausted
        }
        // `Mirage`: a base, and one more for each Poison on the living
        // enemies.
        (__c0, __c1) if (__c0 == V::Card::MIRAGE && __c1 == V::CardVar::CALCULATED_BLOCK) => {
            let poison: Number = owner
                .enemy_powers
                .iter()
                .filter(|stack| stack.power == V::Power::POISON_POWER)
                .map(|stack| match stack.amount {
                    sts2_core::PowerAmount::Counted(amount) => amount,
                    sts2_core::PowerAmount::Endless => unimplemented!("an endless Poison"),
                })
                .sum();
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * poison
        }
        // `Bully`: a base, and more for each Vulnerable on its target.
        (__c0, __c1) if (__c0 == V::Card::BULLY && __c1 == V::CardVar::CALCULATED_DAMAGE) => {
            let vulnerable = owner
                .target
                .and_then(|powers| {
                    powers
                        .iter()
                        .find(|stack| stack.power == V::Power::VULNERABLE_POWER)
                })
                .map_or(0, |stack| match stack.amount {
                    sts2_core::PowerAmount::Counted(amount) => amount,
                    sts2_core::PowerAmount::Endless => unimplemented!("an endless Vulnerable"),
                });
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::EXTRA_DAMAGE) * vulnerable
        }
        // `Normality`: a base, and one fewer card for each play its owner
        // started this turn, up to three.
        (__c0, __c1) if (__c0 == V::Card::NORMALITY && __c1 == V::CardVar::CALCULATED_CARDS) => {
            let started = owner
                .history
                .started_this_turn()
                .count()
                .min(NORMALITY_PLAYS);
            let started = Number::try_from(started).expect("three fits the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * started
        }
        // `BeatIntoShape`: a base, and more for each powered hit its owner
        // landed on its target this turn.
        (__c0, __c1)
            if (__c0 == V::Card::BEAT_INTO_SHAPE && __c1 == V::CardVar::CALCULATED_FORGE) =>
        {
            let hits = owner
                .history
                .damage_received_this_turn()
                .filter(|received| {
                    Some(received.receiver) == owner.targeted
                        && received.dealer == Some(owner.player)
                        && received.powered
                })
                .count();
            let hits = Number::try_from(hits).expect("a turn's hits fit the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * hits
        }
        // `KnifeTrap`: a base, and more for each Shiv in its owner's exhaust
        // pile.
        (__c0, __c1) if (__c0 == V::Card::KNIFE_TRAP && __c1 == V::CardVar::CALCULATED_SHIVS) => {
            let shivs = owner
                .exhaust
                .iter()
                .filter(|&&card| sts2_core::CardTable::tagged(card, sts2_core::CardTagKinds::SHIV))
                .count();
            let shivs = Number::try_from(shivs).expect("an exhaust pile fits the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * shivs
        }
        // `TearAsunder`: a base, and more for each of one and the hits this
        // fight that took hit points from its owner.
        (__c0, __c1) if (__c0 == V::Card::TEAR_ASUNDER && __c1 == V::CardVar::CALCULATED_HITS) => {
            let hurt = owner
                .history
                .damage_received_this_fight()
                .filter(|received| received.receiver == owner.player && received.unblocked > 0)
                .count();
            let hits = Number::try_from(1 + hurt).expect("a fight's hits fit the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * hits
        }
        // `BodySlam`: a base, and more for each point of its owner's block.
        (__c0, __c1) if (__c0 == V::Card::BODY_SLAM && __c1 == V::CardVar::CALCULATED_DAMAGE) => {
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::EXTRA_DAMAGE) * owner.block
        }
        // `Barrage`: a base, and one more hit for each orb its owner holds.
        // `HelixDrill`: a base, and one more hit for each energy its owner
        // spent this turn. In the play pile it leaves out what it costs now.
        (__c0, __c1) if (__c0 == V::Card::HELIX_DRILL && __c1 == V::CardVar::CALCULATED_HITS) => {
            let spent = owner.history.energy_spent_this_turn();
            let own = match owner.cost_in_play {
                None => 0,
                Some(sts2_core::Cost::Fixed(cost)) => cost.get(),
                Some(cost) => unimplemented!("Helix Drill costing {cost:?} in play"),
            };
            let spent = Number::try_from(spent).expect("a turn's energy fits a game number");
            let energy = spent - Number::from(own);
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * energy
        }
        // `Voltaic`: a base, and one more for each Lightning its owner
        // channelled this fight.
        (__c0, __c1) if (__c0 == V::Card::VOLTAIC && __c1 == V::CardVar::CALCULATED_CHANNELS) => {
            let lightning =
                Number::try_from(owner.history.channelled_this_fight(V::Orb::LIGHTNING_ORB))
                    .expect("a fight's orbs fit the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * lightning
        }
        // `FlakCannon`: a base, and one more hit for each status its owner
        // holds outside the exhaust pile.
        (__c0, __c1) if (__c0 == V::Card::FLAK_CANNON && __c1 == V::CardVar::CALCULATED_HITS) => {
            let statuses =
                Number::try_from(owner.statuses).expect("a fight's cards fit the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * statuses
        }
        (__c0, __c1) if (__c0 == V::Card::BARRAGE && __c1 == V::CardVar::CALCULATED_HITS) => {
            let orbs =
                Number::try_from(owner.orbs.len()).expect("an orb queue fits the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * orbs
        }
        // `CompileDriver`: a base, and one more card for each kind of orb its
        // owner holds.
        (__c0, __c1)
            if (__c0 == V::Card::COMPILE_DRIVER && __c1 == V::CardVar::CALCULATED_CARDS) =>
        {
            let mut kinds: smallvec::SmallVec<[_; 10]> = smallvec::SmallVec::new();
            for held in owner.orbs {
                if !kinds.contains(&held.orb) {
                    kinds.push(held.orb);
                }
            }
            let kinds =
                Number::try_from(kinds.len()).expect("an orb queue fits the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * kinds
        }
        // `PerfectedStrike`: a base, and more for each of its owner's cards in
        // the fight tagged Strike, itself included.
        (__c0, __c1)
            if (__c0 == V::Card::PERFECTED_STRIKE && __c1 == V::CardVar::CALCULATED_DAMAGE) =>
        {
            let strikes =
                Number::try_from(owner.strikes).expect("a fight's cards fit the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::EXTRA_DAMAGE) * strikes
        }
        // `MindBlast`: a base, and one more for each card in the draw pile.
        (__c0, __c1) if (__c0 == V::Card::MIND_BLAST && __c1 == V::CardVar::CALCULATED_DAMAGE) => {
            let draw =
                Number::try_from(owner.draw).expect("a draw pile's size fits the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::EXTRA_DAMAGE) * draw
        }
        // `LunarBlast`: a base, and one more hit for each Skill its owner's
        // plays finished this turn. A fight here has one player.
        (__c0, __c1) if (__c0 == V::Card::LUNAR_BLAST && __c1 == V::CardVar::CALCULATED_HITS) => {
            let skills = owner
                .history
                .finished_this_turn()
                .filter(|&card| {
                    <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Skill)
                })
                .count();
            let skills = Number::try_from(skills).expect("a turn's plays fit the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * skills
        }
        // `Finisher`: a base, and one more hit for each attack its owner's
        // plays finished this turn. A fight here has one player.
        (__c0, __c1) if (__c0 == V::Card::FINISHER && __c1 == V::CardVar::CALCULATED_HITS) => {
            let attacks = owner
                .history
                .finished_this_turn()
                .filter(|&card| {
                    <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Attack)
                })
                .count();
            let attacks = Number::try_from(attacks).expect("a turn's plays fit the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * attacks
        }
        // `NoEscape`: a base, and more for each whole `DoomThreshold` of Doom
        // on its target.
        (__c0, __c1) if (__c0 == V::Card::NO_ESCAPE && __c1 == V::CardVar::CALCULATED_DOOM) => {
            let doom = owner
                .target
                .and_then(|powers| {
                    powers
                        .iter()
                        .find(|stack| stack.power == V::Power::DOOM_POWER)
                })
                .map_or(0, |stack| match stack.amount {
                    sts2_core::PowerAmount::Counted(amount) => amount,
                    sts2_core::PowerAmount::Endless => unimplemented!("an endless Doom"),
                });
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA)
                    * (doom / printed.face(V::CardVar::DOOM_THRESHOLD))
        }
        // `Radiate`: a base, and one more hit for each star its owner gained
        // this turn. A fight here has one player.
        (__c0, __c1) if (__c0 == V::Card::RADIATE && __c1 == V::CardVar::CALCULATED_HITS) => {
            let gained: Number = owner
                .history
                .stars_modified_this_turn()
                .filter(|&amount| amount > 0)
                .sum();
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * gained
        }
        // `CrescentSpear`: a base, and more for each of its owner's cards in
        // the fight that prints a star cost, itself included.
        (__c0, __c1)
            if (__c0 == V::Card::CRESCENT_SPEAR && __c1 == V::CardVar::CALCULATED_DAMAGE) =>
        {
            let starred =
                Number::try_from(owner.star_cards).expect("a fight's cards fit the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::EXTRA_DAMAGE) * starred
        }
        // `GoldAxe`: a base, and more for each card play the fight has
        // finished.
        (__c0, __c1) if (__c0 == V::Card::GOLD_AXE && __c1 == V::CardVar::CALCULATED_DAMAGE) => {
            let plays = owner.history.finished_this_fight().count();
            let plays = Number::try_from(plays).expect("a fight's plays fit the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::EXTRA_DAMAGE) * plays
        }
        // `PullFromBelow`: a base, and one more hit for each of its owner's
        // plays this fight of a card that was Ethereal. A fight here has one
        // player.
        (__c0, __c1)
            if (__c0 == V::Card::PULL_FROM_BELOW && __c1 == V::CardVar::CALCULATED_HITS) =>
        {
            let ethereal = owner
                .history
                .finished_this_fight()
                .filter(|finished| finished.ethereal)
                .count();
            let ethereal =
                Number::try_from(ethereal).expect("a fight's plays fit the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * ethereal
        }
        // `Supermassive`: a base, and more for each card its owner made in
        // the fight.
        (__c0, __c1)
            if (__c0 == V::Card::SUPERMASSIVE && __c1 == V::CardVar::CALCULATED_DAMAGE) =>
        {
            let made = Number::try_from(owner.history.generated_by_player())
                .expect("a fight's cards fit the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::EXTRA_DAMAGE) * made
        }
        // `MementoMori`: a base, and more for each card its owner discarded
        // this turn. A fight here has one player.
        (__c0, __c1)
            if (__c0 == V::Card::MEMENTO_MORI && __c1 == V::CardVar::CALCULATED_DAMAGE) =>
        {
            let discarded = Number::try_from(owner.history.discarded_this_turn())
                .expect("a turn's discards fit the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::EXTRA_DAMAGE) * discarded
        }
        // `Synchronize`: a base, and more Focus for each kind of orb its
        // owner holds.
        (__c0, __c1) if (__c0 == V::Card::SYNCHRONIZE && __c1 == V::CardVar::CALCULATED_FOCUS) => {
            let mut kinds: smallvec::SmallVec<[_; 10]> = smallvec::SmallVec::new();
            for held in owner.orbs {
                if !kinds.contains(&held.orb) {
                    kinds.push(held.orb);
                }
            }
            let kinds =
                Number::try_from(kinds.len()).expect("an orb queue fits the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * kinds
        }
        // `Flechettes`: a base, and one more hit for each Skill in its owner's
        // hand.
        (__c0, __c1) if (__c0 == V::Card::FLECHETTES && __c1 == V::CardVar::CALCULATED_HITS) => {
            let skills = owner
                .hand
                .iter()
                .filter(|&&card| {
                    <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Skill)
                })
                .count();
            let skills = Number::try_from(skills).expect("a hand fits the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::CALCULATION_EXTRA) * skills
        }
        // `Squeeze`: a base, and more for each of its owner's other cards in
        // the fight tagged as Osty's attack.
        (__c0, __c1) if (__c0 == V::Card::SQUEEZE && __c1 == V::CardVar::CALCULATED_DAMAGE) => {
            let attacks = Number::try_from(owner.osty_attacks_besides)
                .expect("a fight's cards fit the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::EXTRA_DAMAGE) * attacks
        }
        // `TimesUp`: a base, and more for each Doom on its target.
        (__c0, __c1) if (__c0 == V::Card::TIMES_UP && __c1 == V::CardVar::CALCULATED_DAMAGE) => {
            let doom = owner
                .target
                .and_then(|powers| {
                    powers
                        .iter()
                        .find(|stack| stack.power == V::Power::DOOM_POWER)
                })
                .map_or(0, |stack| match stack.amount {
                    sts2_core::PowerAmount::Counted(amount) => amount,
                    sts2_core::PowerAmount::Endless => unimplemented!("an endless Doom"),
                });
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::EXTRA_DAMAGE) * doom
        }
        // `DeathMarch`: a base, and more for each card its owner drew this
        // turn outside the hand draw. A fight here has one player.
        (__c0, __c1) if (__c0 == V::Card::DEATH_MARCH && __c1 == V::CardVar::CALCULATED_DAMAGE) => {
            let drawn = owner.history.drawn_outside_hand_draw_this_turn();
            let drawn = Number::try_from(drawn).expect("a turn's draws fit the game's numbers");
            printed.face(<V::CardVar as crate::ids::CardVarIds>::CALCULATION_BASE)
                + printed.face(V::CardVar::EXTRA_DAMAGE) * drawn
        }
        (card, key) => unimplemented!("the {} {card:?} works out", key.name()),
    }
}

/// Where a card's own play stops to ask something, under its card grouping,
/// in a fight of kind `K`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Resume<V: crate::Build, K: sts2_core::FightKind<V>> {
    Colorless(colorless::Resume),
    /// A Defect card's play, paused where it asked.
    Defect(defect::paused::State<V, K>),
    Event(event::Resume<V>),
    /// An Ironclad card's play, paused where it asked.
    Ironclad(ironclad::paused::State<V, K>),
    /// A Necrobinder card's play, paused where it asked.
    Necrobinder(necrobinder::paused::State<V, K>),
    /// A Regent card's play, paused where it asked.
    Regent(regent::paused::State<V, K>),
    /// A Silent card's play, paused where it asked.
    Silent(silent::paused::State<V, K>),
    Token(token::Resume),
    /// A generated play, from the command after the one that left a
    /// question waiting.
    Generated(V::Card, u8),
    /// A play of the build's own, from the command after the one that left a
    /// question waiting. The list holds the cards it still has to go through.
    Own(V::Card, u8, Left<V>),
}

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

/// `start`, `resume` and `settle` for a pool's `paused` coroutine, whose
/// paused plays wait in `Resume::$pool`.
macro_rules! paused_play {
    ($pool:ident) => {
        /// A play of [`paused`], from its start.
        fn start<'a, V: crate::Build, K: sts2_core::FightKind<V>>(
            card: V::Card,
            fight: &mut K::Fight<'a>,
        ) -> crate::behaviour::Played<V, K> {
            let mut play = paused::<V, K>(card);
            let state = play.start_with(crate::behaviour::cards::Lent { fight, taken: None });
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
                crate::behaviour::cards::Lent {
                    fight,
                    taken: Some(taken),
                },
            );
            settle::<V, K>(play, state)
        }

        /// How far a step of `play` got.
        fn settle<V: crate::Build, K: sts2_core::FightKind<V>>(
            play: paused::State<V, K>,
            state: diapause::CoroutineState<crate::behaviour::cards::Ask<V>, ()>,
        ) -> crate::behaviour::Played<V, K> {
            match state {
                diapause::CoroutineState::Complete(()) => crate::behaviour::Played::Done,
                diapause::CoroutineState::Yielded(ask) => {
                    ask.at(crate::behaviour::cards::Resume::$pool(play).into())
                }
            }
        }
    };
}
pub(crate) use paused_play;

/// The card a choose-a-card screen took, or none where it was skipped.
pub(crate) fn chosen<V: crate::Build>(
    taken: &Option<sts2_core::Taken<V>>,
) -> Option<sts2_core::CardIdent<V>> {
    match taken {
        Some(sts2_core::Taken::Chosen(card)) => *card,
        taken => unreachable!("a choice, not {taken:?}"),
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

/// How many cards [`Left`] holds. Most lists are a hand's worth, but
/// Eidolon's is every Ethereal card in the exhaust pile.
pub(crate) const LEFT: usize = 24;

/// The cards a play still has to go through once it waited, in the order
/// the game's list took them. A card that joined the pile meanwhile is not
/// one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Left<V: crate::Build>([Option<sts2_core::CardAt<V>>; LEFT]);

impl<V: crate::Build> Left<V> {
    /// `cards`, in order.
    ///
    /// # Panics
    ///
    /// More cards than [`LEFT`].
    pub(crate) fn of(cards: &[sts2_core::CardAt<V>]) -> Self {
        let mut left = [None; LEFT];
        if cards.len() > LEFT {
            unimplemented!("a play going through {} cards after a wait", cards.len());
        }
        for (at, &card) in cards.iter().enumerate() {
            left[at] = Some(card);
        }
        Left(left)
    }

    /// The cards, in order.
    pub(crate) fn cards(self) -> impl Iterator<Item = sts2_core::CardAt<V>> {
        self.0.into_iter().flatten()
    }
}

/// The creatures a play took before a command that can wait, which it reads
/// after. A fight holds ten at most.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Held([Option<sts2_core::CombatId>; 10]);

impl Held {
    /// `creatures`, in order.
    ///
    /// # Panics
    ///
    /// More creatures than a fight holds.
    pub(crate) fn of(creatures: &[sts2_core::CombatId]) -> Self {
        let mut held = [None; 10];
        assert!(
            creatures.len() <= held.len(),
            "more creatures than a fight holds"
        );
        for (at, &creature) in creatures.iter().enumerate() {
            held[at] = Some(creature);
        }
        Held(held)
    }

    /// The creatures, in order.
    pub(crate) fn creatures(self) -> impl Iterator<Item = sts2_core::CombatId> {
        self.0.into_iter().flatten()
    }
}

/// A generated play from the command at `from` on, standing still where a
/// command leaves a question waiting.
pub(crate) fn generated_from<V: crate::Build, F: Fight<V>>(
    card: V::Card,
    fight: &mut F,
    from: u8,
) -> Played<V, F::Kind> {
    let played = V::generated_play_from(card, fight, from);
    played.unwrap_or_else(|next| Played::Waiting {
        at: Resume::Generated(card, next).into(),
    })
}

/// What a card does when it is played.
#[tracing::instrument(level = "trace", skip_all, fields(card = ?card))]
pub(crate) fn on_play<'a, V: crate::Build, K: sts2_core::FightKind<V>>(
    card: V::Card,
    fight: &mut K::Fight<'a>,
) -> Played<V, K> {
    // A card whose rule is the build's own.
    if let Some(played) = V::own_card_play(card, fight) {
        return played;
    }
    match V::card_filed(card) {
        CardFiled::Ironclad => ironclad::on_play::<V, K>(card, fight),
        CardFiled::Silent => silent::on_play::<V, K>(card, fight),
        CardFiled::Regent => regent::on_play::<V, K>(card, fight),
        CardFiled::Necrobinder => necrobinder::on_play::<V, K>(card, fight),
        CardFiled::Defect => defect::on_play::<V, K>(card, fight),
        CardFiled::Colorless => colorless::on_play::<V, _>(card, fight),
        CardFiled::Curse | CardFiled::Quest | CardFiled::Deprecated => {
            generated_from::<V, _>(card, fight, 0)
        }
        CardFiled::Event => event::on_play::<V, _>(card, fight),
        CardFiled::Status => status::on_play::<V, _>(card, fight),
        CardFiled::Token => token::on_play::<V, _>(card, fight),
    }
}

/// What a card still in the hand does when its owner's turn ends.
pub(crate) fn on_turn_end<V: crate::Build, F: Fight<V>>(card: V::Card, fight: &mut F) {
    match V::card_filed(card) {
        CardFiled::Curse => curse::on_turn_end::<V, _>(card, fight),
        CardFiled::Status => status::on_turn_end::<V, _>(card, fight),
        _ => V::generated_turn_end(card, fight),
    }
}

/// Deliver a notification to one card in the fight.
///
/// `printed` is what this copy of the card shows, which is what the model
/// reads its own numbers off.
pub(crate) fn event_hook<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    card: V::Card,
    id: sts2_core::CardId<V>,
    pile: Option<sts2_core::Pile>,
    printed: &P,
    history: sts2_core::History<'_, V>,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    // A card whose hooks are the build's own, for a hook it answers itself.
    if V::own_card_hooks(card) && V::own_card_event_hook(card, hook, asked).is_some() {
        return;
    }
    // The fight asks only a card whose class overrides the hook.
    match card {
        // `Guilty.AfterCombatEnd` counts on the copy in the deck, and a copy
        // in the fight's piles does nothing.
        __c0 if __c0 == V::Card::GUILTY
            && (matches!(hook, sts2_core::EventHook::CombatEnd { .. })) => {}
        card if V::card_filed(card) == CardFiled::Status => {
            status::event_hook::<V, _>(card, id, printed, hook, asked)
        }
        card if V::card_filed(card) == CardFiled::Curse => {
            curse::event_hook::<V>(card, pile, hook, asked)
        }
        // `HowlFromBeyond.AfterAutoPostPlayPhaseEntered`: from the exhaust
        // pile, it plays itself.
        __c0 if __c0 == V::Card::HOWL_FROM_BEYOND
            && (matches!(hook, sts2_core::EventHook::AutoPostPlayPhase { .. })) =>
        {
            if pile == Some(sts2_core::Pile::Exhaust) {
                asked.push(sts2_core::HookEffect::AutoPlay(id));
            }
        }
        card if V::card_filed(card) == CardFiled::Ironclad => {
            ironclad::event_hook::<V, _>(card, id, printed, history, hook, asked)
        }
        card if V::card_filed(card) == CardFiled::Colorless => {
            colorless::event_hook::<V>(card, id, history, hook, asked)
        }
        // `SovereignBlade.AfterCardChangedPiles` plays the forge's effects and
        // removes its node, and its `AfterTransformedFrom` removes the node
        // too. Neither touches the fight.
        __c0 if __c0 == V::Card::SOVEREIGN_BLADE
            && (matches!(hook, sts2_core::EventHook::CardChangedPiles { .. })) => {}
        card if V::card_filed(card) == CardFiled::Necrobinder => {
            necrobinder::event_hook::<V, _>(card, id, printed, history, hook, asked)
        }
        // `Bombardment.AfterAutoPrePlayPhaseEnteredEarly`: from the exhaust
        // pile, it plays itself.
        __c0 if __c0 == V::Card::BOMBARDMENT
            && (matches!(hook, sts2_core::EventHook::AutoPrePlayPhaseEarly { .. })) =>
        {
            if pile == Some(sts2_core::Pile::Exhaust) {
                asked.push(sts2_core::HookEffect::AutoPlay(id));
            }
        }
        // `IAmInvincible.AfterAutoPostPlayPhaseEntered`: on top of the draw
        // pile, it has the top card played, which is itself.
        __c0 if __c0 == V::Card::I_AM_INVINCIBLE
            && (matches!(hook, sts2_core::EventHook::AutoPostPlayPhase { .. })) =>
        {
            asked.push(sts2_core::HookEffect::AutoPlayFromDrawIfTop(id));
        }
        card if V::card_filed(card) == CardFiled::Regent => {
            regent::event_hook::<V, _>(card, id, printed, history, hook, asked)
        }
        card if V::card_filed(card) == CardFiled::Silent => {
            silent::event_hook::<V>(card, id, history, hook, asked)
        }
        // `RocketPunch.AfterCardGeneratedForCombat`: free until played once
        // its owner makes a Status.
        __c0 if __c0 == V::Card::ROCKET_PUNCH => match hook {
            sts2_core::EventHook::CardGeneratedForCombat {
                card,
                creator: Some(_),
                ..
            } if <V as sts2_core::Rules>::card_kind(card) == Some(sts2_core::CardKind::Status) => {
                asked.push(sts2_core::HookEffect::ModifySelf {
                    change: sts2_core::Change::SetCost(sts2_core::CostPool::Energy, 0),
                    lasting: sts2_core::Lasting::UntilPlayed,
                })
            }
            sts2_core::EventHook::CardGeneratedForCombat { .. } => {}
            hook => unimplemented!("what RocketPunch does for {hook:?}"),
        },
        card => unimplemented!("what {card:?} does for {hook:?}"),
    }
}

/// What a deck card's own model saves between fights: its `[SavedProperty]`
/// fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum State<V: crate::Build> {
    Stateless,
    /// A card whose rule is the build's own.
    Own(V::OwnCard),
    /// `Guilty.CombatsSeen`.
    Guilty {
        combats_seen: u8,
    },
    /// `GeneticAlgorithm.IncreasedBlock`.
    GeneticAlgorithm {
        increased_block: Number,
    },
    /// `TheScythe.IncreasedDamage`.
    TheScythe {
        increased_damage: Number,
    },
}

impl<V: crate::Build> sts2_core::CardModel<V> for State<V> {
    fn new(card: V::Card) -> Self {
        match card {
            __c0 if __c0 == V::Card::GUILTY => Self::Guilty { combats_seen: 0 },
            __c0 if __c0 == V::Card::GENETIC_ALGORITHM => {
                Self::GeneticAlgorithm { increased_block: 0 }
            }
            __c0 if __c0 == V::Card::THE_SCYTHE => Self::TheScythe {
                increased_damage: 0,
            },
            card => V::own_card_state(card).map_or(Self::Stateless, Self::Own),
        }
    }

    /// `Guilty.CombatsSeen`'s setter prints `Combats` as the fights it has
    /// left. The copy of a Genetic Algorithm or a Scythe keeps what the card
    /// gained, and prints the base and that.
    fn written(&self) -> Option<sts2_core::Change<V>> {
        match *self {
            Self::Guilty { combats_seen } if combats_seen > 0 => Some(sts2_core::Change::Raise(
                V::CardVar::COMBATS,
                -Number::from(combats_seen),
            )),
            Self::GeneticAlgorithm {
                increased_block: gained,
            }
            | Self::TheScythe {
                increased_damage: gained,
            } if gained != 0 => Some(sts2_core::Change::Own(CardWrite::BuffFromPlay(gained))),
            Self::Own(own) => V::own_card_written(own),
            _ => None,
        }
    }
}

/// A method of a card's own class, on one fight copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CardWrite {
    /// `Claw.BuffFromClawPlay`, `Maul.BuffFromMaulPlay`, and `BuffFromPlay`
    /// of `TheScythe` and `GeneticAlgorithm`. Rampage's play and Kingly
    /// Punch's draw make the same two writes inline.
    BuffFromPlay(Number),
    /// What Thrash's play writes inline: the hit it took from the card it
    /// exhausted.
    ExtraDamage(sts2_core::Fixed),
}

/// The fields a fight copy's own class adds to `CardModel`, for the classes
/// whose `AfterDowngraded` reads them.
///
/// Each number is a [`Field`], so that a copy holds them in four bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fields {
    /// A class that adds none `AfterDowngraded` reads.
    Nothing,
    /// `Claw.ExtraDamageFromClawPlays`.
    Claw { extra_damage: Field },
    /// `Maul.ExtraDamageFromMaulPlays`.
    Maul { extra_damage: Field },
    /// `Rampage.ExtraDamageFromPlays`.
    Rampage { extra_damage: Field },
    /// `KinglyPunch.ExtraDamage`.
    KinglyPunch { extra_damage: Field },
    /// `Thrash.ExtraDamage`, a decimal: its coefficient and scale.
    Thrash { coefficient: Field, scale: u8 },
    /// `TheScythe.IncreasedDamage`.
    TheScythe { increased_damage: Field },
    /// `GeneticAlgorithm.IncreasedBlock`.
    GeneticAlgorithm { increased_block: Field },
    /// `SovereignBlade.CurrentDamage`. Its `CurrentRepeats` stays at one,
    /// because nothing calls `SetRepeats`.
    SovereignBlade { current_damage: Field },
}

/// One number of [`Fields`], as two bytes hold it.
///
/// `None` is a number that outgrew them, which panics only where it is read.
/// So a fight that forges a blade past them panics only if it then
/// downgrades the blade.
pub type Field = Option<nonmax::NonMaxI16>;

/// `TheScythe`'s damage before anything it gained.
const SCYTHE_DAMAGE: Number = 13;
/// `GeneticAlgorithm`'s block before anything it gained.
const GENETIC_ALGORITHM_BLOCK: Number = 1;
/// `SovereignBlade._currentDamage` as a blade is made.
const SOVEREIGN_BLADE_DAMAGE: Number = 10;

/// `number` as a [`Field`].
fn field(number: impl TryInto<i16>) -> Field {
    nonmax::NonMaxI16::new(number.try_into().ok()?)
}

/// What `field` holds, for a downgrade to write back.
///
/// # Panics
///
/// A number that outgrew the field.
fn read(field: Field) -> Number {
    let field = field.unwrap_or_else(|| unimplemented!("a card's field past what two bytes hold"));
    Number::from(field.get())
}

/// `field + by`.
fn added(field: Field, by: Number) -> Field {
    self::field(Number::from(field?.get()).checked_add(by)?)
}

/// The raise that takes `key` from what `printed` holds to `to`.
fn set<V: crate::Build, P: sts2_core::Printed<V> + ?Sized>(
    key: V::CardVar,
    to: Number,
    printed: &P,
) -> sts2_core::Change<V> {
    sts2_core::Change::Raise(key, to - printed.face(key))
}

impl<V: crate::Build> sts2_core::CardFields<V> for Fields {
    fn new(card: V::Card) -> Self {
        let zero = field(0);
        match card {
            __c0 if __c0 == V::Card::CLAW => Self::Claw { extra_damage: zero },
            __c0 if __c0 == V::Card::MAUL => Self::Maul { extra_damage: zero },
            __c0 if __c0 == V::Card::RAMPAGE => Self::Rampage { extra_damage: zero },
            __c0 if __c0 == V::Card::KINGLY_PUNCH => Self::KinglyPunch { extra_damage: zero },
            __c0 if __c0 == V::Card::THRASH => Self::Thrash {
                coefficient: zero,
                scale: 0,
            },
            __c0 if __c0 == V::Card::THE_SCYTHE => Self::TheScythe {
                increased_damage: zero,
            },
            __c0 if __c0 == V::Card::GENETIC_ALGORITHM => Self::GeneticAlgorithm {
                increased_block: zero,
            },
            __c0 if __c0 == V::Card::SOVEREIGN_BLADE => Self::SovereignBlade {
                current_damage: field(SOVEREIGN_BLADE_DAMAGE),
            },
            _ => Self::Nothing,
        }
    }

    fn write<P: sts2_core::Printed<V> + ?Sized>(
        &mut self,
        write: CardWrite,
        printed: &P,
    ) -> sts2_core::Change<V> {
        match (self, write) {
            (
                Self::Claw { extra_damage }
                | Self::Maul { extra_damage }
                | Self::Rampage { extra_damage }
                | Self::KinglyPunch { extra_damage },
                CardWrite::BuffFromPlay(by),
            ) => {
                *extra_damage = added(*extra_damage, by);
                sts2_core::Change::Raise(V::CardVar::DAMAGE, by)
            }
            // `CurrentDamage = 13 + IncreasedDamage`, whose setter prints it.
            (Self::TheScythe { increased_damage }, CardWrite::BuffFromPlay(by)) => {
                *increased_damage = added(*increased_damage, by);
                set::<V, _>(
                    V::CardVar::DAMAGE,
                    SCYTHE_DAMAGE + read(*increased_damage),
                    printed,
                )
            }
            // `CurrentBlock = 1 + IncreasedBlock`, whose setter prints it.
            (Self::GeneticAlgorithm { increased_block }, CardWrite::BuffFromPlay(by)) => {
                *increased_block = added(*increased_block, by);
                set::<V, _>(
                    V::CardVar::BLOCK,
                    GENETIC_ALGORITHM_BLOCK + read(*increased_block),
                    printed,
                )
            }
            (Self::Thrash { coefficient, scale }, CardWrite::ExtraDamage(by)) => {
                let sum = coefficient
                    .map(|held| {
                        sts2_core::Fixed::new(held.get().into(), *scale).exact() + by.exact()
                    })
                    .and_then(sts2_core::Fixed::from_exact);
                *coefficient = sum.and_then(|sum| field(sum.coefficient()));
                *scale = sum.map_or(0, sts2_core::Fixed::scale);
                sts2_core::Change::RaiseExactly(V::CardVar::DAMAGE, by)
            }
            (fields, write) => unimplemented!("{write:?} on a copy keeping {fields:?}"),
        }
    }

    fn add_damage<P: sts2_core::Printed<V> + ?Sized>(
        &mut self,
        amount: Number,
        printed: &P,
    ) -> sts2_core::Change<V> {
        let Self::SovereignBlade { current_damage } = self else {
            unreachable!("AddDamage on a copy keeping {self:?}")
        };
        // `Damage.BaseValue += amount`, then `CurrentDamage` is what it
        // prints.
        *current_damage = printed
            .face(V::CardVar::DAMAGE)
            .checked_add(amount)
            .and_then(field);
        sts2_core::Change::Raise(V::CardVar::DAMAGE, amount)
    }

    fn after_downgraded<P: sts2_core::Printed<V> + ?Sized>(
        &self,
        printed: &P,
    ) -> Option<sts2_core::Change<V>> {
        match *self {
            Self::Nothing => None,
            Self::Claw { extra_damage }
            | Self::Maul { extra_damage }
            | Self::Rampage { extra_damage }
            | Self::KinglyPunch { extra_damage } => Some(sts2_core::Change::Raise(
                V::CardVar::DAMAGE,
                read(extra_damage),
            )),
            Self::Thrash { coefficient, scale } => Some(sts2_core::Change::RaiseExactly(
                V::CardVar::DAMAGE,
                sts2_core::Fixed::new(read(coefficient), scale),
            )),
            Self::TheScythe { increased_damage } => Some(set::<V, _>(
                V::CardVar::DAMAGE,
                SCYTHE_DAMAGE + read(increased_damage),
                printed,
            )),
            Self::GeneticAlgorithm { increased_block } => Some(set::<V, _>(
                V::CardVar::BLOCK,
                GENETIC_ALGORITHM_BLOCK + read(increased_block),
                printed,
            )),
            Self::SovereignBlade { current_damage } => Some(set::<V, _>(
                V::CardVar::DAMAGE,
                read(current_damage),
                printed,
            )),
        }
    }
}

pub use crate::behaviour::DeckWrite;

/// `write` on the deck card `card`.
///
/// # Panics
///
/// A write the card's model has no field for.
pub(crate) fn write_deck_card<V: crate::Build>(
    card: &mut sts2_core::DeckCard<V>,
    write: DeckWrite,
) {
    match (&mut card.state, write) {
        (
            State::GeneticAlgorithm {
                increased_block: gained,
            },
            DeckWrite::BuffFromPlay(n),
        )
        | (
            State::TheScythe {
                increased_damage: gained,
            },
            DeckWrite::BuffFromPlay(n),
        ) => {
            *gained += n;
        }
        (_, DeckWrite::RaiseEnchantment(n)) => {
            let enchanted = (card.ident.enchantment.as_mut())
                .unwrap_or_else(|| unreachable!("an enchantment raised on a card without one"));
            enchanted.amount = (enchanted.amount.checked_add(n))
                .unwrap_or_else(|| unimplemented!("an enchantment past what a byte holds"));
        }
        (state, write) => unimplemented!("{write:?} on a deck card saving {state:?}"),
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
pub(crate) fn deck_event_hook<V: crate::Build>(
    card: V::Card,
    state: &mut State<V>,
    hook: sts2_core::EventHook<V>,
    asked: &mut sts2_core::HookEffects<V>,
) {
    match (card, state, hook) {
        (card, State::Own(own), hook) => V::own_deck_card_event_hook(card, own, hook, asked),
        // `Guilty.AfterCombatEnd`: one more fight seen, and out of the deck at
        // five.
        (__c0, State::Guilty { combats_seen }, sts2_core::EventHook::CombatEnd { .. })
            if __c0 == V::Card::GUILTY =>
        {
            *combats_seen += 1;
            if *combats_seen >= GUILTY_COMBATS {
                asked.push(sts2_core::HookEffect::RemoveFromDeck);
            }
        }
        // `SpoilsMap.ModifyGeneratedMap`: a `SpoilsActMap` in place of its
        // act's own.
        (__c0, _, sts2_core::EventHook::GeneratedMap { act }) if __c0 == V::Card::SPOILS_MAP => {
            if act == SPOILS_ACT {
                asked.push(sts2_core::HookEffect::SpoilsMap);
            }
        }
        // `SpoilsMap.ModifyGeneratedMapLate`: `SpoilsCoord` on the act's first
        // Treasure point, where `AfterMapGenerated` puts its quest and the
        // chest pays its `GoldVar(600)`.
        (__c0, _, sts2_core::EventHook::GeneratedMapLate { act })
            if __c0 == V::Card::SPOILS_MAP =>
        {
            if act == SPOILS_ACT {
                asked.push(sts2_core::HookEffect::SpoilsQuest {
                    card,
                    gold: SPOILS_GOLD,
                });
            }
        }
        // `SpoilsMap.AfterMapGenerated` puts its quest on `SpoilsCoord`, which
        // the run keeps.
        (__c0, _, sts2_core::EventHook::MapGenerated { .. }) if __c0 == V::Card::SPOILS_MAP => {}
        // `LanternKey.ModifyUnknownMapPointRoomTypes`: nothing but an event in
        // its act.
        (__c0, _, sts2_core::EventHook::UnknownRoomTypes { act })
            if __c0 == V::Card::LANTERN_KEY =>
        {
            if act == LANTERN_KEY_ACT {
                asked.push(sts2_core::HookEffect::EventRoomsOnly);
            }
        }
        // `ByrdonisEgg.TryModifyRestSiteOptions`: the Hatch option.
        (__c0, _, sts2_core::EventHook::RestSiteOptions) if __c0 == V::Card::BYRDONIS_EGG => {
            asked.push(sts2_core::HookEffect::OfferRestOption(
                <V::RestSiteOption as crate::ids::RestSiteOptionIds>::HATCH,
            ));
        }
        // `SovereignBlade.AfterCardChangedPiles` only plays the forge's
        // effects.
        (__c0, _, sts2_core::EventHook::AddedToDeck { .. }) if __c0 == V::Card::SOVEREIGN_BLADE => {
        }
        // `Melancholy.AfterDeath` changes its cost only in a combat pile.
        (__c0, _, sts2_core::EventHook::AfterDeath { .. }) if __c0 == V::Card::MELANCHOLY => {}
        (card, _, hook) => unimplemented!("what {card:?} in the deck does for {hook:?}"),
    }
}

/// The rest of a play that stopped, once the fight has its answer.
pub(crate) fn resume_play<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V, F::Kind>,
    chosen: sts2_core::SelectedCards<V>,
) -> Played<V, F::Kind> {
    match at {
        Resume::Colorless(at) => colorless::resume_play::<V, _>(fight, at, chosen),
        Resume::Event(at) => event::resume_play::<V, _>(fight, at, chosen),
        Resume::Token(at) => unreachable!("{at:?} waits on a question, not a pick"),
        Resume::Defect(_)
        | Resume::Ironclad(_)
        | Resume::Necrobinder(_)
        | Resume::Regent(_)
        | Resume::Silent(_) => {
            unreachable!("a coroutine's play resumes through it")
        }
        Resume::Generated(card, _) => {
            unreachable!("generated {card:?} waits on a question, not a pick")
        }
        Resume::Own(card, ..) => unreachable!("{card:?} waits on a question, not a pick"),
    }
}

/// The rest of a play that waited on its own draw, with every copy the draw
/// drew. A play that waited on anything else carries on as after a wait.
pub(crate) fn resume_drew<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V, F::Kind>,
    drew: &[sts2_core::CardId<V>],
) -> Played<V, F::Kind> {
    match at {
        Resume::Own(card, from, _) => V::own_card_drew_from(card, fight, from, drew),
        at => resume_waited::<V, _>(fight, at),
    }
}

/// The rest of a play that waited on a question one of its commands left,
/// once the question is answered.
pub(crate) fn resume_waited<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V, F::Kind>,
) -> Played<V, F::Kind> {
    match at {
        Resume::Colorless(at) => colorless::resume_waited::<V, _>(fight, at),
        Resume::Event(at) => event::resume_waited::<V, _>(fight, at),
        Resume::Defect(_)
        | Resume::Ironclad(_)
        | Resume::Necrobinder(_)
        | Resume::Regent(_)
        | Resume::Silent(_) => {
            unreachable!("a coroutine's play resumes through it")
        }
        Resume::Token(at) => token::resume_waited::<V, _>(fight, at),
        Resume::Generated(card, from) => generated_from::<V, _>(card, fight, from),
        Resume::Own(card, from, left) => V::own_card_play_from(card, fight, from, left),
    }
}

/// The rest of a play that stopped on a choose-a-card screen, once the
/// player took one or skipped it.
pub(crate) fn resume_chosen<V: crate::Build, F: Fight<V>>(
    fight: &mut F,
    at: Resume<V, F::Kind>,
    chosen: Option<sts2_core::CardIdent<V>>,
) -> Played<V, F::Kind> {
    match at {
        Resume::Colorless(at) => colorless::resume_chosen::<V, _>(fight, at, chosen),
        Resume::Event(at) => event::resume_chosen::<V, _>(fight, at, chosen),
        at => unimplemented!("{at:?} resumed with a chosen card"),
    }
}

/// The act, counted from zero, in which the Lantern Key answers.
const LANTERN_KEY_ACT: usize = 2;

/// `SpoilsMap.SpoilsActIndex`, which `AfterCreated` sets.
const SPOILS_ACT: usize = 1;

/// The Spoils Map's `GoldVar(600)`.
const SPOILS_GOLD: Number = 600;

/// `ModifyNextEvent` for a card in the deck. The Lantern Key sends its act's
/// next event to War Historian Repy.
///
/// # Panics
///
/// Any other card, which the run asks only when its class overrides the hook.
pub(crate) fn next_event<V: crate::Build>(card: V::Card, act: usize, event: V::Event) -> V::Event {
    match card {
        __c0 if __c0 == V::Card::LANTERN_KEY && (act == LANTERN_KEY_ACT) => {
            V::Event::WAR_HISTORIAN_REPY
        }
        __c0 if __c0 == V::Card::LANTERN_KEY => event,
        card => unimplemented!("what {card:?} does to the next event"),
    }
}
