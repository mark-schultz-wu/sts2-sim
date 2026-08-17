using System;
using MegaCrit.Sts2.Core.Entities.Ascension;
using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Dump;

/// <summary>
/// Reading a value at every ascension level rather than at one.
/// </summary>
/// <remarks>
/// <c>AscensionManager.HasLevel</c> is <c>_level &gt;= (int)level</c>, so
/// ascension is a scalar and each level is a threshold: anything behind
/// <c>AscensionHelper.GetValueIfAscension</c> is a step function of it.
///
/// So a dump that wants an ascension-dependent number walks all eleven levels
/// and records where the value actually moves, rather than trusting somebody's
/// reading of which levels matter. That reading has already been wrong once:
/// damage steps at <c>DeadlyEnemies</c> for every monster except Gremlin Merc,
/// whose three attacks step at <c>ToughEnemies</c> instead.
/// </remarks>
internal static class Ascension
{
    /// <summary>Every level a run can be at, lowest first.</summary>
    public static readonly int[] Levels =
        Enumerable(AscensionManager.maxAscensionAllowed + 1);

    /// <summary>
    /// Sets the run's ascension level, so a value can be read at each one.
    /// </summary>
    /// <remarks>
    /// <c>RunManager.HasAscension</c> returns false outright when no run is in
    /// progress, so without one every level would read the same and a sweep
    /// would report — convincingly — that nothing scales. Hence the check
    /// rather than a silent zero.
    ///
    /// The setter is non-public, which is the whole reason this is reflection:
    /// the game has no other way to move a run's ascension after it starts,
    /// because nothing in the game ever wants to.
    /// </remarks>
    public static void SetLevel(int level)
    {
        RunManager runs = RunManager.Instance;
        if (!runs.IsInProgress)
        {
            throw new InvalidOperationException(
                "no run in progress: every ascension level would read as 0 and the sweep would find nothing");
        }

        typeof(RunManager)
            .GetProperty(nameof(RunManager.AscensionManager))!
            .GetSetMethod(nonPublic: true)!
            .Invoke(runs, new object[] { new AscensionManager(level) });

        // Read it back through the game's own predicate. A setter that silently
        // did nothing would make every level agree, which is indistinguishable
        // from a value that does not scale.
        if (AscensionHelper.HasAscension(AscensionLevel.DoubleBoss) != (level >= (int)AscensionLevel.DoubleBoss))
        {
            throw new InvalidOperationException($"setting ascension {level} did not take");
        }
    }

    private static int[] Enumerable(int count)
    {
        var levels = new int[count];
        for (int i = 0; i < count; i++)
        {
            levels[i] = i;
        }

        return levels;
    }
}
