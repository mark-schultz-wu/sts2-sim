using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Random;

namespace Sts2Dump;

/// <summary>Adapts dumps to the full-state RNG ABI.</summary>
internal static class GameApi
{
    internal static Rng NewRngAtDraw(uint seed, int counter)
    {
        var rng = new Rng(seed);
        for (int i = 0; i < counter; i++)
        {
            rng.NextInt();
        }
        return rng;
    }

    internal static int Counter(Rng rng) => rng.ToSerializable().counter;

    internal static ulong NamedSeed(uint seed, string name)
        => seed + StringHelper.GetDeterministicHashCode(name);

    internal static bool IsDebugEncounter(EncounterModel encounter) => false;
}
