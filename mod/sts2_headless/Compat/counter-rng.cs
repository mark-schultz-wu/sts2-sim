using System.Collections.Generic;
using System.Linq;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Entities.Rngs;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

/// <summary>Adapts game builds that serialize each RNG as a draw counter.</summary>
/// <remarks>
/// These builds keep no saved generator state, so a position here carries the
/// count and nothing else. A reader compares what both sides report.
/// </remarks>
internal static class GameApi
{
    internal static bool CanUseOrRemovePotions(Player player) => player.CanRemovePotions;

    internal static IReadOnlyDictionary<RunRngType, RngPosition> RunRngCounters(RunState state)
        => state.Rng.ToSerializable().Counters.ToDictionary(pair => pair.Key, pair => new RngPosition(pair.Value));

    internal static IReadOnlyDictionary<PlayerRngType, RngPosition> PlayerRngCounters(Player player)
        => player.PlayerRng.ToSerializable().Counters.ToDictionary(pair => pair.Key, pair => new RngPosition(pair.Value));
}
