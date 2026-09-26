using System.Collections.Generic;
using System.Linq;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Entities.Rngs;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

/// <summary>Adapts game builds that serialize each RNG's complete state.</summary>
internal static class GameApi
{
    /// <summary>Whether <see cref="ScreenPathSelections"/> applies.</summary>
    /// <remarks>
    /// Not here. This family signals before it counts the cards, undoes an
    /// ended turn first, and has a <c>LocalSelector</c> that takes the screen's
    /// branch. The patch mirrors the other family's bodies, so this one keeps
    /// the selector's branch.
    /// </remarks>
    internal const bool AsksThroughTheScreen = false;

    /// <summary><c>SignalPlayerChoiceBegun</c>, which this family also names the player for.</summary>
    internal static System.Threading.Tasks.Task SignalChoiceBegun(
        MegaCrit.Sts2.Core.GameActions.Multiplayer.PlayerChoiceContext context,
        Player player,
        MegaCrit.Sts2.Core.Entities.Multiplayer.PlayerChoiceOptions options)
        => context.SignalPlayerChoiceBegun(player, options);

    internal static bool CanUseOrRemovePotions(Player player) => player.CanUseOrRemovePotions;

    internal static IReadOnlyDictionary<RunRngType, RngPosition> RunRngCounters(RunState state)
        => state.Rng.ToSerializable().Rngs.ToDictionary(pair => pair.Key, pair => Position(pair.Value));

    internal static IReadOnlyDictionary<PlayerRngType, RngPosition> PlayerRngCounters(Player player)
        => player.PlayerRng.ToSerializable().Rngs.ToDictionary(pair => pair.Key, pair => Position(pair.Value));

    /// <summary>The whole record, which this family saves and the other does not.</summary>
    private static RngPosition Position(MegaCrit.Sts2.Core.Saves.SerializableRng rng)
        => new(rng.counter, rng.state0, rng.state1, rng.state2, rng.state3);
}
