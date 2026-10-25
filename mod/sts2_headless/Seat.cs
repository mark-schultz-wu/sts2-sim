using System;
using MegaCrit.Sts2.Core.Context;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

/// <summary>
/// The player this process plays, which <c>LocalContext</c> names by its net
/// id. In a run of one that is the only player. In a co-op run each process
/// is one seat, and reports and decides for its own player alone.
/// </summary>
internal static class Seat
{
    internal static Player Me(RunState state) =>
        LocalContext.GetMe(state) ?? throw new InvalidOperationException(
            $"no player in the run has this process's net id {LocalContext.NetId}");

    /// <summary><see cref="Me"/>, for a run that may not be there.</summary>
    internal static Player? MeIn(RunState? state) => state == null ? null : LocalContext.GetMe(state);
}
