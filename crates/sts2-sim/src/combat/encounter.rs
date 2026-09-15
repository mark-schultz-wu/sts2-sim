//! Turning an encounter id into the creatures that open its fight.

use crate::rng::MegaRng;
use sts2_core::rng::GameRng;
use sts2_core::{EncounterRoster, EncounterTable, Rules};

use super::Error;

/// Resolve a static roster without RNG, or run the build-specific generator
/// from the encounter's private stream.
pub(crate) fn opening<V: Rules>(
    raw_seed: u64,
    floor: V::Count,
    encounter: V::Encounter,
) -> Result<Vec<V::Monster>, Error> {
    match encounter.roster() {
        EncounterRoster::Fixed(monsters) => Ok(monsters.collect()),
        EncounterRoster::Random => {
            let floor: u64 = num_traits::cast(floor)
                .expect("a floor number fits the seed the game mixes it into");
            let seed = V::seed_from(
                raw_seed
                    .wrapping_add(floor)
                    .wrapping_add(encounter.seed_salt()),
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
                Monster::TwigSlimeS,
                Monster::TwigSlimeM,
                Monster::LeafSlimeS,
            ])
        );
    }

    #[test]
    fn a_fixed_roster_does_not_need_a_ported_generator() {
        assert_eq!(
            opening::<V0_107_1>(123, 2, Encounter::MockMonsterEncounter),
            Ok(vec![Monster::BigDummy])
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
