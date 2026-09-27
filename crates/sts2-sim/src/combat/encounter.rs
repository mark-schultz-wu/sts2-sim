//! Turning an encounter id into the creatures that open its fight.

use crate::rng::MegaRng;
use sts2_core::rng::GameRng;
use sts2_core::{EncounterRoster, EncounterTable, Rules, Spawned};

use super::Error;

/// Resolve a static roster without RNG, or run the build-specific generator
/// from the encounter's private stream.
///
/// A fixed roster opens each monster on the state its own table entry
/// carries, or on the one the encounter writes onto it.
pub(crate) fn opening<V: Rules>(
    raw_seed: u64,
    floor: V::Count,
    encounter: V::Encounter,
) -> Result<Vec<Spawned<V>>, Error> {
    match encounter.roster() {
        EncounterRoster::Fixed(monsters) => Ok(monsters
            .map(|(monster, opens)| match opens {
                Some(opens) => Spawned::opening_on(monster, opens),
                None => Spawned::new(monster),
            })
            .collect()),
        EncounterRoster::Random => {
            let floor: u64 = num_traits::cast(floor)
                .expect("a floor number fits the seed the game mixes it into");
            let seed = V::seed_from(
                raw_seed
                    .wrapping_add(floor)
                    .wrapping_add(V::name_hash(encounter.seed_name())),
            );
            let mut rng = MegaRng::new(seed);
            V::random_encounter_monsters(encounter, &mut rng).map_err(Error::from)
        }
        EncounterRoster::Configured => {
            unimplemented!("an encounter whose caller supplies its roster")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sts2_content::v0_107_1::{V0_107_1, encounters::Encounter, monsters::Monster};

    #[test]
    fn weak_slimes_are_generated_from_the_encounter_private_stream() {
        assert_eq!(
            opening::<V0_107_1>(123, 2, Encounter::SlimesWeak),
            Ok(vec![
                Spawned::new(Monster::TwigSlimeS),
                Spawned::new(Monster::TwigSlimeM),
                Spawned::new(Monster::LeafSlimeS),
            ])
        );
    }

    #[test]
    fn a_fixed_roster_does_not_need_a_ported_generator() {
        assert_eq!(
            opening::<V0_107_1>(123, 2, Encounter::MockMonsterEncounter),
            Ok(vec![Spawned::new(Monster::BigDummy)])
        );
    }

    /// `DecimillipedeElite` rolls one number and hands the three segments the
    /// three moves that cycle, in the order the row stands in.
    ///
    /// The roster is the same three monsters however the draw falls, so what
    /// the draw decides is only which of them opens on which move.
    #[test]
    fn the_decimillipede_segments_open_on_three_different_moves() {
        use sts2_content::v0_107_1::monsters::MoveId;

        let roster = opening::<V0_107_1>(123, 2, Encounter::DecimillipedeElite)
            .expect("the segments are generated");

        assert_eq!(
            roster.iter().map(|s| s.monster).collect::<Vec<_>>(),
            [
                Monster::DecimillipedeSegmentFront,
                Monster::DecimillipedeSegmentMiddle,
                Monster::DecimillipedeSegmentBack,
            ]
        );
        let opens: Vec<_> = roster.iter().map(|s| s.opens_on).collect();
        let cycle = [MoveId::WritheMove, MoveId::BulkMove, MoveId::ConstrictMove];
        let Some(first) = opens[0].and_then(|id| cycle.iter().position(|&m| m == id)) else {
            panic!("the front segment opens on one of the three: {opens:?}");
        };
        assert_eq!(
            opens,
            (0..3)
                .map(|at| Some(cycle[(first + at) % 3]))
                .collect::<Vec<_>>()
        );
    }
}

#[cfg(test)]
mod unported_tests {
    use super::*;
    use sts2_content::v0_107_1::{V0_107_1, encounters::Encounter};

    /// A roster the caller supplies is a rule nobody has written.
    #[test]
    #[should_panic(expected = "an encounter whose caller supplies its roster")]
    fn a_configured_roster_stops_the_run() {
        let _ = opening::<V0_107_1>(123, 2, Encounter::BattlewornDummyEventEncounter);
    }

    /// So is a roster this build rolls that the port has not reached.
    #[test]
    #[should_panic(expected = "the roster")]
    fn an_unported_random_roster_stops_the_run() {
        let _ = opening::<V0_107_1>(123, 2, Encounter::BowlbugsNormal);
    }
}
