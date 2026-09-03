using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Random;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2DeterminismTest;

/// <summary>Adapts determinism probes to the counter-only RNG ABI.</summary>
internal static class GameApi
{
    internal static ulong ExpectedEncounterSeed(RunState state, string encounterId)
        => (uint)((int)state.Rng.Seed + state.TotalFloor
            + StringHelper.GetDeterministicHashCode(encounterId));

    internal static bool BowlbugsRngMatchesSeed(Rng actual, ulong expectedSeed)
        => actual.Seed == (uint)expectedSeed;

    internal static string Describe(Rng rng) => $"seed {rng.Seed}";
}
