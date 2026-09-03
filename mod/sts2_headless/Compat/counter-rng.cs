using System.Collections.Generic;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Entities.Rngs;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

/// <summary>Adapts game builds that serialize each RNG as a draw counter.</summary>
internal static class GameApi
{
    internal static bool CanUseOrRemovePotions(Player player) => player.CanRemovePotions;

    internal static IReadOnlyDictionary<RunRngType, int> RunRngCounters(RunState state)
        => state.Rng.ToSerializable().Counters;

    internal static IReadOnlyDictionary<PlayerRngType, int> PlayerRngCounters(Player player)
        => player.PlayerRng.ToSerializable().Counters;
}
