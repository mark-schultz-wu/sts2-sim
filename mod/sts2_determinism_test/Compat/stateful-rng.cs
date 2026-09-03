using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Random;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Saves;

namespace Sts2DeterminismTest;

/// <summary>Adapts determinism probes to the full-state RNG ABI.</summary>
internal static class GameApi
{
    internal static ulong ExpectedEncounterSeed(RunState state, string encounterId)
        => (ulong)((long)state.Rng.Seed + state.TotalFloor)
            + StringHelper.GetDeterministicHashCode(encounterId);

    internal static bool BowlbugsRngMatchesSeed(Rng actual, ulong expectedSeed)
    {
        var expected = new Rng(expectedSeed);
        expected.NextInt(2);
        SerializableRng left = actual.ToSerializable();
        SerializableRng right = expected.ToSerializable();
        return left.counter == right.counter
            && left.state0 == right.state0
            && left.state1 == right.state1
            && left.state2 == right.state2
            && left.state3 == right.state3;
    }

    internal static string Describe(Rng rng)
    {
        SerializableRng state = rng.ToSerializable();
        return $"counter {state.counter}, state {state.state0:x16}";
    }
}
