//! Turning an encounter id into the creatures that open its fight.

use sts2_core::rng::{GameRng, MegaRng};
use sts2_core::{EncounterRoster, EncounterTable, Rules};

use super::Error;

/// Resolve a static roster without RNG, or run the build-specific generator
/// from the encounter's private stream.
pub(crate) fn opening<V: Rules>(
    raw_seed: u32,
    floor: V::Count,
    encounter: V::Encounter,
) -> Result<Vec<V::Monster>, Error<V>> {
    match encounter.roster() {
        EncounterRoster::Fixed(monsters) => Ok(monsters.collect()),
        EncounterRoster::Random => {
            let floor: u32 = num_traits::cast(floor)
                .expect("a floor number fits the seed the game mixes it into");
            let seed = raw_seed
                .wrapping_add(floor)
                .wrapping_add(encounter.seed_salt());
            let mut rng = MegaRng::new(seed);
            V::random_encounter_monsters(encounter, &mut rng).map_err(Error::from)
        }
        EncounterRoster::Configured => Err(Error::ConfiguredRoster),
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
mod error_tests {
    use super::*;
    use sts2_content::v0_107_1::{V0_107_1, encounters::Encounter};
    use sts2_core::ContentError;

    #[test]
    fn each_non_static_roster_failure_keeps_its_reason() {
        assert_eq!(
            opening::<V0_107_1>(123, 2, Encounter::BattlewornDummyEventEncounter),
            Err(Error::ConfiguredRoster)
        );
        assert_eq!(
            opening::<V0_107_1>(123, 2, Encounter::BowlbugsNormal),
            Err(Error::Content(ContentError::Unported(
                "random encounter roster"
            )))
        );
    }
}
