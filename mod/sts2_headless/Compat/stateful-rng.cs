using System.Collections.Generic;
using System.Linq;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Entities.Rngs;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

/// <summary>Adapts game builds that serialize each RNG's complete state.</summary>
internal static class GameApi
{
    internal static bool CanUseOrRemovePotions(Player player) => player.CanUseOrRemovePotions;

    internal static IReadOnlyDictionary<RunRngType, int> RunRngCounters(RunState state)
        => state.Rng.ToSerializable().Rngs.ToDictionary(pair => pair.Key, pair => pair.Value.counter);

    internal static IReadOnlyDictionary<PlayerRngType, int> PlayerRngCounters(Player player)
        => player.PlayerRng.ToSerializable().Rngs.ToDictionary(pair => pair.Key, pair => pair.Value.counter);
}
