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
    /// <summary>Whether <see cref="ScreenPathSelections"/> applies, which mirrors this family's <c>CardSelectCmd</c>.</summary>
    internal const bool AsksThroughTheScreen = true;

    /// <summary>Whether the selector sits in <c>LocalSelector</c>, which this family lacks.</summary>
    internal const bool SelectsLocally = false;

    /// <summary><c>CardSelectCmd.UseSelector</c>, into the one slot this family has.</summary>
    internal static System.IDisposable UseSelector(MegaCrit.Sts2.Core.TestSupport.ICardSelector selector)
        => MegaCrit.Sts2.Core.Commands.CardSelectCmd.UseSelector(selector);

    /// <summary><c>SignalPlayerChoiceBegun</c>, which this family takes the options alone for.</summary>
    internal static System.Threading.Tasks.Task SignalChoiceBegun(
        MegaCrit.Sts2.Core.GameActions.Multiplayer.PlayerChoiceContext context,
        Player player,
        MegaCrit.Sts2.Core.Entities.Multiplayer.PlayerChoiceOptions options)
        => context.SignalPlayerChoiceBegun(options);

    internal static bool CanUseOrRemovePotions(Player player) => player.CanRemovePotions;

    internal static IReadOnlyDictionary<RunRngType, RngPosition> RunRngCounters(RunState state)
        => state.Rng.ToSerializable().Counters.ToDictionary(pair => pair.Key, pair => new RngPosition(pair.Value));

    internal static IReadOnlyDictionary<PlayerRngType, RngPosition> PlayerRngCounters(Player player)
        => player.PlayerRng.ToSerializable().Counters.ToDictionary(pair => pair.Key, pair => new RngPosition(pair.Value));
}
