//! Content for game build `v0.107.1`.
//!
//! `dumps/` holds what the running game printed, under
//! `STS2_HEADLESS_MODE=carddump` and friends. See `mod/sts2_dump/`. Each dump
//! carries the content hash in its header, and the `tests` module requires that
//! to match [`VERSION`]. Regenerating against a different build then fails,
//! rather than quietly replacing the tables. `generated/` mirrors it, one
//! module per kind of content.
//!
//! Static means true with no run in progress. That is what a card costs, how
//! much HP a monster rolls, whether a power stacks, and which monsters an
//! encounter fields. What any of it *does* is code, and gets ported by hand.
//!
//! Where a value is code, the dump says so rather than guessing, and it
//! generates as [`Field::Computed`](sts2_core::Field). That is the simulator's
//! cue to ask rather than look up. Each domain decides it differently, because
//! each had to. A card is marked from its overrides, a power from its IL, and
//! an encounter by running it and reading the `Rng` counter afterwards. All of
//! them err the same way, towards computed. Marking a constant as computed
//! costs one method, and the reverse costs a divergence nobody can see.

mod behaviour;
mod generated;

pub use generated::*;

use sts2_core::Version;

/// This build, as a type.
///
/// Zero-sized: it exists to be a type parameter, so that a client or a
/// simulator names the build once and every id follows. See
/// [`ContentVersion`](sts2_core::ContentVersion).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct V0_107_1;

impl sts2_core::ContentVersion for V0_107_1 {
    const VERSION: Version = VERSION;

    type Number = i32;
    type Count = u32;

    type AscensionLevel = generated::ascension::AscensionLevel;
    type Act = generated::acts::Act;
    type Affliction = generated::afflictions::Affliction;
    type Card = generated::cards::Cards;
    type CardVar = generated::cards::CardVar;
    type Character = generated::characters::Character;
    type Enchantment = generated::enchantments::Enchantment;
    type Encounter = generated::encounters::Encounter;
    type Event = generated::events::Event;
    type Modifier = generated::modifiers::Modifier;
    type Monster = generated::monsters::Monster;
    type Orb = generated::orbs::Orb;
    type Potion = generated::potions::Potion;
    type Power = generated::powers::Power;
    type Relic = generated::relics::Relic;
    type RoomType = generated::encounters::RoomType;

    // The vocabulary a run's position is described in. `runstate` is one dump
    // of its own because none of these is a property of a content item; the two
    // below it belong to the domains whose content has them.
    type MapPointType = generated::runstate::MapPointType;
    type CombatSide = generated::runstate::CombatSide;
    type PlayerTurnPhase = generated::runstate::PlayerTurnPhase;
    type UnplayableReason = generated::runstate::UnplayableReason;
    type UnplayableReasons = generated::runstate::UnplayableReasons;
    type RunRngType = generated::runstate::RunRngType;
    type PlayerRngType = generated::runstate::PlayerRngType;
    type IntentKind = generated::monsters::IntentKind;
    type MoveId = generated::monsters::MoveId;
    type RestSiteOption = generated::restsite::RestSiteOption;
    type TargetType = generated::cards::TargetType;
}

/// Nothing yet. The rules land here as they are ported, one pool at a time.
/// Until then this build is its tables and no more.
impl sts2_core::Rules for V0_107_1 {
    fn relic_pool(character: Self::Character) -> generated::relics::RelicPool {
        sts2_core::Table::data(character).relic_pool
    }

    fn starting_deck(character: Self::Character) -> Vec<Self::Card> {
        sts2_core::Table::data(character).deck.iter().collect()
    }

    fn starting_relics(character: Self::Character) -> Vec<Self::Relic> {
        sts2_core::Table::data(character)
            .starting_relics
            .iter()
            .collect()
    }

    fn starting_hp(character: Self::Character) -> u16 {
        sts2_core::Table::data(character).starting_hp
    }

    fn starting_gold(character: Self::Character) -> u16 {
        sts2_core::Table::data(character).starting_gold
    }

    fn max_energy(character: Self::Character) -> u8 {
        sts2_core::Table::data(character).max_energy
    }

    fn starting_potion_slots() -> usize {
        3
    }

    fn neow_event() -> Self::Event {
        generated::events::Event::Neow
    }

    fn colorless_cards() -> sts2_core::Drafted<'static, Self::Card> {
        generated::cards::colorless_cards()
    }

    fn reward_rarity(card: Self::Card) -> sts2_core::RewardRarity {
        use generated::cards::CardRarity;
        match sts2_core::CardTable::rarity(card) {
            CardRarity::Common => sts2_core::RewardRarity::Common,
            CardRarity::Uncommon => sts2_core::RewardRarity::Uncommon,
            CardRarity::Rare => sts2_core::RewardRarity::Rare,
            _ => sts2_core::RewardRarity::Other,
        }
    }

    fn lead_paperweight() -> Self::Relic {
        generated::relics::Relic::LeadPaperweight
    }

    fn divine_right() -> Self::Relic {
        generated::relics::Relic::DivineRight
    }

    fn on_play<F: sts2_core::Fight<Self>>(card: Self::Card, fight: &mut F) {
        behaviour::cards::on_play(card, fight)
    }

    fn reward_cards(character: Self::Character) -> sts2_core::Drafted<'static, Self::Card> {
        generated::cards::reward_cards(character)
    }

    fn reward_potions(character: Self::Character) -> sts2_core::Drafted<'static, Self::Potion> {
        generated::potions::reward_potions(character)
    }

    fn encounter_gold(
        encounter: Self::Encounter,
        ascension: Self::AscensionLevel,
    ) -> sts2_core::rng::Interval<u16> {
        let gold = sts2_core::Table::data(encounter).gold;
        sts2_core::rng::Interval::new(gold.min.at(ascension as u8), gold.max.at(ascension as u8))
    }

    fn monster_hp_bounds(
        monster: Self::Monster,
        ascension: Self::AscensionLevel,
    ) -> sts2_core::rng::Interval<u16> {
        let data = sts2_core::Table::data(monster);
        sts2_core::rng::Interval::new(
            data.min_hp.at(ascension as u8),
            data.max_hp.at(ascension as u8),
        )
    }

    fn roll_monster_move<R: sts2_core::rng::GameRng>(
        monster: Self::Monster,
        ascension: Self::AscensionLevel,
        spawn: sts2_core::MonsterSpawn<Self::Encounter>,
        machine: &mut sts2_core::MoveMachine,
        rng: &mut R,
    ) -> Result<sts2_core::NextMove<Self>, sts2_core::ContentError> {
        roll_monster_move(monster, ascension as u8, spawn, machine, rng)
    }

    fn initial_monster_state(monster: Self::Monster) -> u8 {
        sts2_core::Table::data(monster).initial
    }

    fn damage_multiplier(power: Self::Power, holding: sts2_core::Holding) -> f64 {
        behaviour::powers::damage_multiplier(power, holding)
    }

    fn ticks_down(power: Self::Power, side: Self::CombatSide) -> bool {
        behaviour::powers::ticks_down(power, side)
    }

    fn perform_move<F: sts2_core::Fight<Self>>(
        monster: Self::Monster,
        move_id: Self::MoveId,
        fight: &mut F,
    ) {
        behaviour::monsters::perform(monster, move_id, fight);
    }

    fn random_encounter_monsters<R: sts2_core::rng::GameRng>(
        encounter: Self::Encounter,
        rng: &mut R,
    ) -> Result<Vec<Self::Monster>, sts2_core::ContentError> {
        use generated::encounters::Encounter;
        use generated::monsters::Monster;
        use sts2_core::rng::Bound;

        let Encounter::SlimesWeak = encounter else {
            unimplemented!("the roster {encounter:?} rolls");
        };

        let small = [Monster::LeafSlimeS, Monster::TwigSlimeS];
        let first = rng.next_int(Bound::new(2).expect("two small slimes")) as usize;
        let second = rng.next_int(Bound::new(1).expect("one small slime remains"));
        debug_assert_eq!(second, 0);
        let medium = [Monster::LeafSlimeM, Monster::TwigSlimeM]
            [rng.next_int(Bound::new(2).expect("two medium slimes")) as usize];
        Ok(vec![small[first], medium, small[1 - first]])
    }

    fn encounters(act: Self::Act) -> &'static [Self::Encounter] {
        generated::encounters::encounters_of(act)
    }

    fn events(act: Self::Act) -> &'static [Self::Event] {
        generated::events::events_of(act)
    }

    fn ancients(act: Self::Act) -> &'static [Self::Event] {
        generated::events::ancients_of(act)
    }

    fn shared_events() -> &'static [Self::Event] {
        &generated::events::SHARED_EVENTS
    }

    fn shared_ancients() -> &'static [Self::Event] {
        &generated::events::SHARED_ANCIENTS
    }

    fn neow_curses() -> &'static [Self::Relic] {
        &generated::events::NEOW_CURSES
    }

    fn neow_positives() -> &'static [Self::Relic] {
        &generated::events::NEOW_POSITIVES
    }

    fn neow_flips() -> &'static [(Self::Relic, Self::Relic)] {
        &generated::events::NEOW_FLIPS
    }

    fn neow_pairs() -> &'static [(Self::Relic, Self::Relic)] {
        &generated::events::NEOW_PAIRS
    }

    fn neow_suppresses_flip() -> Self::Relic {
        generated::events::NEOW_SUPPRESSES_FLIP
    }

    /// The four relics whose Neow gate is code rather than a flag.
    ///
    /// `relics.txt` marks them `gatedAtNeow` or `gatedInRun`, which is the cue
    /// to ask rather than look up. The conditions themselves are one line each
    /// and are ported here, beside the build they belong to.
    ///
    /// Both unlock questions are true for a run, which is set up with
    /// everything unlocked so that no scenario depends on a save file.
    fn allowed_at_neow(relic: Self::Relic, players: usize) -> bool {
        use generated::relics::Relic;
        match relic {
            // `runState.Players.Count == 1`.
            Relic::SilverCrucible => players == 1,
            // `runState.Players.Count > 1`.
            Relic::MassiveScroll => players > 1,
            // Every character's card pool unlocked, and every card pool has
            // enough commons to build a bundle from.
            Relic::Kaleidoscope | Relic::ScrollBoxes => true,
            _ => true,
        }
    }

    fn run_rng_types() -> &'static [Self::RunRngType] {
        &generated::runstate::RunRngType::ALL
    }

    fn niche_rng() -> Self::RunRngType {
        generated::runstate::RunRngType::Niche
    }

    fn monster_ai_rng() -> Self::RunRngType {
        generated::runstate::RunRngType::MonsterAi
    }

    fn shuffle_rng() -> Self::RunRngType {
        generated::runstate::RunRngType::Shuffle
    }

    fn up_front_rng() -> Self::RunRngType {
        generated::runstate::RunRngType::UpFront
    }

    fn player_rng_types() -> &'static [Self::PlayerRngType] {
        &generated::runstate::PlayerRngType::ALL
    }

    fn rewards_rng() -> Self::PlayerRngType {
        generated::runstate::PlayerRngType::Rewards
    }
    fn enemy_side() -> Self::CombatSide {
        generated::runstate::CombatSide::Enemy
    }
}

/// Walk to the move a monster makes next, which is `RollMove`.
///
/// `FindNextMoveState` returns early while the machine is pinned. Two things
/// pin it. One is the move it started on, until any move has been performed.
/// The other is a move the graph holds it on until that move has been.
///
/// Otherwise it steps until it lands on a move, and logs the move it lands on.
fn roll_monster_move<R: sts2_core::rng::GameRng>(
    monster: generated::monsters::Monster,
    ascension: u8,
    spawn: sts2_core::MonsterSpawn<generated::encounters::Encounter>,
    machine: &mut sts2_core::MoveMachine,
    rng: &mut R,
) -> Result<sts2_core::NextMove<V0_107_1>, sts2_core::ContentError> {
    use generated::monsters::{FollowUp, State};

    let data = sts2_core::Table::data(monster);
    let state = |at: u8| {
        data.states
            .get(usize::from(at))
            .copied()
            .ok_or(sts2_core::ContentError::InvalidMonsterState { index: at })
    };

    if !pinned(state(machine.at)?, machine) {
        loop {
            let next = match state(machine.at)? {
                State::Move(held) => match held.follow_up {
                    FollowUp::Next(at) | FollowUp::OncePerformed(at) => at,
                    // `MoveState.GetNextState` throws on a missing follow-up.
                    FollowUp::Never => unimplemented!("a move with no follow-up"),
                },
                State::Random(branches) => branch(branches, machine, rng)?,
                State::Conditional(_) => {
                    behaviour::monsters::conditional(monster, machine.at, spawn)?
                }
            };
            machine.enter(next);
            if let State::Move(_) = state(machine.at)? {
                machine.log.push(machine.at);
                break;
            }
        }
    }

    let State::Move(held) = state(machine.at)? else {
        return Err(sts2_core::ContentError::InvalidMonsterState { index: machine.at });
    };
    intents(held, ascension)
}

/// Whether the machine cannot leave the state it stands on.
fn pinned(state: generated::monsters::State, machine: &sts2_core::MoveMachine) -> bool {
    use generated::monsters::{FollowUp, State};

    let State::Move(held) = state else {
        return false;
    };
    // `CanTransitionAway`, then the machine-wide "no move performed yet".
    matches!(held.follow_up, FollowUp::OncePerformed(_)) && !machine.performed
        || !machine.ever_performed
}

/// One branch of a random node, which is `RandomBranchState.GetNextState`.
///
/// One draw, scaled by the total weight, walking the branches in order. A
/// branch its own history bars weighs nothing, which is `GetStateWeight`.
fn branch<R: sts2_core::rng::GameRng>(
    branches: &'static [generated::monsters::Branch],
    machine: &sts2_core::MoveMachine,
    rng: &mut R,
) -> Result<u8, sts2_core::ContentError> {
    use crate::Field;
    use sts2_core::rng::Interval;

    let weight = |branch: &generated::monsters::Branch| match branch.weight {
        Field::Constant(weight) if allowed(branch, machine) => weight,
        Field::Constant(_) => 0.0,
        Field::Computed => unimplemented!("a branch whose weight the fight decides"),
    };

    let total: f32 = branches.iter().map(weight).sum();
    let range = Interval::try_new(0.0, total)
        .map_err(|_| sts2_core::ContentError::InvalidMonsterWeights)?;
    let mut draw = rng.next_f32_range(range);
    for branch in branches {
        draw -= weight(branch);
        if draw <= 0.0 {
            return Ok(branch.target);
        }
    }
    Err(sts2_core::ContentError::InvalidMonsterWeights)
}

/// Whether a branch's own history leaves it takeable.
fn allowed(branch: &generated::monsters::Branch, machine: &sts2_core::MoveMachine) -> bool {
    use generated::monsters::Repeat;

    let recent = |n: usize| machine.log.iter().rev().take(n);
    match branch.repeat {
        Repeat::Forever => true,
        // Barred while the last `n` log entries are all this branch.
        Repeat::InARow(n) => {
            let n = usize::from(n.get());
            machine.log.len() < n || recent(n).any(|&at| at != branch.target)
        }
        // Barred while this branch is anywhere in the last `n`.
        Repeat::Cooldown(n) => recent(usize::from(n.get())).all(|&at| at != branch.target),
        Repeat::Once => !machine.log.contains(&branch.target),
    }
}

/// The intents a move state publishes, at this ascension.
fn intents(
    held: generated::monsters::Move,
    ascension: u8,
) -> Result<sts2_core::NextMove<V0_107_1>, sts2_core::ContentError> {
    use crate::Field;
    use generated::monsters::IntentKind;

    let attack = match held.intents.attack {
        None => None,
        Some(attack) => {
            let Field::Constant(damage) = attack.damage else {
                unimplemented!("a move whose damage the fight decides");
            };
            let Field::Constant(repeats) = attack.repeats else {
                unimplemented!("a move whose repeats the fight decides");
            };
            Some(sts2_core::Attack {
                damage: i32::from(damage.at(ascension)),
                repeats: repeats.at(ascension),
            })
        }
    };
    let intents = held
        .intents
        .kinds
        .iter()
        .map(|kind| sts2_core::Intent {
            kind,
            attack: matches!(kind, IntentKind::Attack | IntentKind::DeathBlow)
                .then_some(attack)
                .flatten(),
        })
        .collect();
    Ok(sts2_core::NextMove {
        id: held.id,
        intents,
    })
}

pub const VERSION: Version = Version {
    name: "v0.107.1",
    commit: "59260271",
    assembly_hash: 1_692_500_715,
    content_hash: 3_954_186_980,
};

/// The act dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const ACTS: &str = include_str!("dumps/acts.txt");

/// The card dump, verbatim. Test data, not runtime data.
///
/// `cargo xtask codegen` reads it from disk to build [`cards`], and the `tests`
/// module replays it to check that table. Nothing at runtime parses it. That is
/// what the generated table is for. A `pub const` here both invites it and puts
/// the dump's text into every binary that links this crate.
#[cfg(test)]
const CARDS: &str = include_str!("dumps/cards.txt");

/// The monster dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const MONSTERS: &str = include_str!("dumps/monsters.txt");

/// The power dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const POWERS: &str = include_str!("dumps/powers.txt");

/// The encounter dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const ENCOUNTERS: &str = include_str!("dumps/encounters.txt");

/// The relic dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const RELICS: &str = include_str!("dumps/relics.txt");

/// The character dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const CHARACTERS: &str = include_str!("dumps/characters.txt");

/// The event dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const EVENTS: &str = include_str!("dumps/events.txt");

/// The potion dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const POTIONS: &str = include_str!("dumps/potions.txt");

/// The enchantment dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const ENCHANTMENTS: &str = include_str!("dumps/enchantments.txt");

/// The affliction dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const AFFLICTIONS: &str = include_str!("dumps/afflictions.txt");

/// The orb dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const ORBS: &str = include_str!("dumps/orbs.txt");

/// The modifier dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const MODIFIERS: &str = include_str!("dumps/modifiers.txt");

/// The ascension dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const ASCENSION: &str = include_str!("dumps/ascension.txt");

/// The run-state dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const RUNSTATE: &str = include_str!("dumps/runstate.txt");

/// The rest-site dump, verbatim. Test data, on the same terms as [`CARDS`].
#[cfg(test)]
const RESTSITE: &str = include_str!("dumps/restsite.txt");

/// The hook dump, verbatim. Test data, on the same terms as [`CARDS`].
///
/// The one dump with no generated table beside it. It describes the interface
/// between an engine and the things that react to it. A simulator implements
/// that rather than looking it up, so there is nothing to generate. It is still
/// a measurement of this build, and the tests are where it gets pinned.
#[cfg(test)]
const HOOKS: &str = include_str!("dumps/hooks.txt");

#[cfg(test)]
mod tests;
