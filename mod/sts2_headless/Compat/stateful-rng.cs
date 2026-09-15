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

    internal static IReadOnlyDictionary<RunRngType, RngPosition> RunRngCounters(RunState state)
        => state.Rng.ToSerializable().Rngs.ToDictionary(pair => pair.Key, pair => Position(pair.Value));

    internal static IReadOnlyDictionary<PlayerRngType, RngPosition> PlayerRngCounters(Player player)
        => player.PlayerRng.ToSerializable().Rngs.ToDictionary(pair => pair.Key, pair => Position(pair.Value));

    /// <summary>The whole record, which this family saves and the other does not.</summary>
    private static RngPosition Position(MegaCrit.Sts2.Core.Saves.SerializableRng rng)
        => new(rng.counter, rng.state0, rng.state1, rng.state2, rng.state3);
}
