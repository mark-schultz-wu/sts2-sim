using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Random;

namespace Sts2Dump;

/// <summary>Adapts dumps to the counter-only RNG ABI.</summary>
internal static class GameApi
{
    internal static Rng NewRngAtDraw(uint seed, int counter)
        => new(seed, counter);

    internal static int Counter(Rng rng) => rng.Counter;

    internal static ulong NamedSeed(uint seed, string name) => new Rng(seed, name).Seed;

    internal static bool IsDebugEncounter(EncounterModel encounter) => encounter.IsDebugEncounter;
}
