//! The alternative first act's monsters.

use sts2_core::{ContentError, Fight, MonsterSpawn, PowerAmount};

use super::super::super::generated::ascension::AscensionLevel;
use super::super::super::generated::encounters::Encounter;
use super::super::super::generated::monster_moves;
use super::super::super::generated::monsters::{Monster, MoveId};
use super::super::super::generated::powers::Power;
use super::{Number, V};

/// `Toadpole.SpikenAmount`, the Thorns Spike Spit takes back off itself.
const SPIKEN: Number = 2;

/// `CorpseSlug.RavenousStr`, either side of `DeadlyEnemies`.
const RAVENOUS: Number = 4;
const RAVENOUS_DEADLY: Number = 5;

/// The powers an Underdocks monster applies to itself as it spawns.
///
/// Nothing for a monster this act does not port.
pub(super) fn starting_powers(
    monster: Monster,
    ascension: AscensionLevel,
) -> Option<Vec<(Power, PowerAmount<Number>)>> {
    Some(match monster {
        Monster::CorpseSlug => vec![(
            Power::RavenousPower,
            PowerAmount::Counted(match ascension.has(AscensionLevel::DeadlyEnemies) {
                true => RAVENOUS_DEADLY,
                false => RAVENOUS,
            }),
        )],
        // Its override calls the base and nothing else.
        Monster::Seapunk => Vec::new(),
        _ => return None,
    })
}

/// Which branch an Underdocks monster's conditional state takes.
pub(super) fn conditional(
    monster: Monster,
    at: u8,
    spawn: MonsterSpawn<Encounter>,
) -> Result<u8, ContentError> {
    match monster {
        Monster::Toadpole => toadpole_opens(at, spawn),
        monster => unimplemented!("what {monster:?} asks the fight at state {at}"),
    }
}

/// `INIT_MOVE`, which reads `IsFront`. `ToadpolesWeak` builds two Toadpoles
/// and sets it on the first, so the front one opens on Spiken and the back
/// one on Whirl.
///
/// # Panics
///
/// A Toadpole an unported encounter built.
fn toadpole_opens(at: u8, spawn: MonsterSpawn<Encounter>) -> Result<u8, ContentError> {
    let state = |wanted| super::state_of(Monster::Toadpole, wanted, at);
    match (
        spawn.encounter,
        spawn.roster_size.get(),
        spawn.position.get(),
    ) {
        (Encounter::ToadpolesWeak, 2, 0) => state(MoveId::SpikenMove),
        (Encounter::ToadpolesWeak, 2, 1) => state(MoveId::WhirlMove),
        _ => unimplemented!("a Toadpole spawned {spawn:?}"),
    }
}

/// Perform one of an Underdocks monster's moves, where the generator left it
/// to a hand port.
pub(super) fn perform<F: Fight<V>>(
    monster: Monster,
    move_id: MoveId,
    step: u8,
    fight: &mut F,
) -> Option<u8> {
    match (monster, move_id) {
        // `DarkStrikeMove`: its hit. `BeforeDamage` plays its sound alone.
        (Monster::CalcifiedCultist | Monster::DampCultist, MoveId::DarkStrikeMove) => {
            fight.attack_opponents()
        }
        // `SpikeSpitMove`: the Thorns Spiken gave, taken back, then its hits.
        (Monster::Toadpole, MoveId::SpikeSpitMove) => {
            fight.apply_to_actor(Power::ThornsPower, PowerAmount::Counted(-SPIKEN));
            fight.attack_opponents();
        }
        (monster, move_id) => {
            return monster_moves::underdocks::perform(monster, move_id, step, fight);
        }
    }
    None
}
