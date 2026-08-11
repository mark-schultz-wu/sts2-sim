using System;
using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Hooks;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Models.Relics;
using MegaCrit.Sts2.Core.Runs;

namespace StsOracle;

/// <summary>
/// Checks that the oracle's reported state actually reflects the game's rules.
/// </summary>
/// <remarks>
/// The failure this defends against is the oracle quietly reimplementing a rule
/// the game already owns, and getting it wrong under conditions nobody tested.
/// The concrete instance: map legality was nearly derived as "the current node's
/// children", which is right until a run holds `Winged Boots` or the `Flight`
/// modifier — both override <c>ShouldAllowFreeTravel</c>, widening the legal set
/// to the whole next row.
///
/// R3's stated check — execute commands outside the reported legal set and
/// require the game to reject them — **cannot catch this**.
/// <c>MapTravel.GetTravelablePointsFrom</c> has exactly one caller,
/// <c>NMapScreen</c>, so the model layer never rejects an illegal map move and
/// the complement fuzz would pass vacuously.
///
/// What catches it is metamorphic: the game's own `override` sites enumerate
/// every place a rule varies, so for each one, put the game in a state where it
/// is active and require the oracle's output to *change*. An oracle that ignores
/// the hook reports the baseline twice, and the test fails.
///
/// The matrix is mechanically derivable — 2 overrides of
/// <c>ShouldAllowFreeTravel</c>, 7 of <c>ShouldPlay</c>, 55 of
/// <c>IsAllowed</c>. This covers the first.
/// </remarks>
internal static class SelfTest
{
    private static int _passed;
    private static int _failed;

    public static async Task Run(RunState state)
    {
        await FreeTravelWidensTheMap(state);

        Log.Info($"[selftest] {_passed} passed, {_failed} failed");
        if (_failed > 0)
        {
            throw new InvalidOperationException($"{_failed} self-test(s) failed");
        }
    }

    /// <summary>
    /// Granting Winged Boots must widen the reported travelable set.
    /// </summary>
    /// <remarks>
    /// Granted through <c>RelicCmd.Obtain</c> rather than by adding to
    /// <c>Player.Relics</c>, because the load path skips <c>AfterObtained</c>
    /// and would produce a state the game never produces.
    /// </remarks>
    private static async Task FreeTravelWidensTheMap(RunState state)
    {
        // At floor 1 the next row held exactly this node's two children, so free
        // travel and normal travel coincide and the test proved nothing while
        // appearing to run. A scenario that cannot distinguish the two rules is
        // not evidence about them, so find one that can — or fail saying so.
        MapPoint? spot = state.Map.GetAllMapPoints()
            .OrderBy(p => p.coord.row).ThenBy(p => p.coord.col)
            .FirstOrDefault(p => state.Map.GetPointsInRow(p.coord.row + 1).Count() > p.Children.Count);

        if (spot == null)
        {
            Check("a map position exists where free travel is observable", false);
            return;
        }

        state.AddVisitedMapCoord(spot.coord);
        Log.Info($"[selftest] testing from {spot.coord.col},{spot.coord.row}: "
            + $"{spot.Children.Count} children vs "
            + $"{state.Map.GetPointsInRow(spot.coord.row + 1).Count()} in the next row");

        List<string> before = Travelable(state);

        Player me = state.Players[0];
        RelicModel boots = ModelDb.Relic<WingedBoots>().ToMutable();
        await RelicCmd.Obtain(boots, me);

        List<string> after = Travelable(state);

        Check("winged boots is actually held", me.Relics.Any(r => r.Id.Entry == boots.Id.Entry));
        Check("the hook reports free travel", Hook.ShouldAllowFreeTravel(state));

        // The specific claim, not merely "something changed": free travel opens
        // the entire next row, so the new set must be a strict superset.
        bool widened = after.Count > before.Count && before.All(after.Contains);
        Check($"free travel widens the map ({before.Count} -> {after.Count} nodes)", widened);

        if (!widened)
        {
            Log.Error($"[selftest]   before: {string.Join(" ", before)}");
            Log.Error($"[selftest]   after:  {string.Join(" ", after)}");
        }
    }

    /// <summary>Reads the travelable set back out of the observation itself.</summary>
    /// <remarks>
    /// Deliberately parsed from the emitted observation rather than by calling
    /// <c>MapTravel</c> again: the point is to test what the oracle *reports*,
    /// and a test that re-derives the answer the same way the reporter did would
    /// agree with a broken reporter.
    /// </remarks>
    private static List<string> Travelable(RunState state)
    {
        using JsonDocument doc = JsonDocument.Parse(Observation.Capture(state));
        return doc.RootElement.GetProperty("map").GetProperty("travelable")
            .EnumerateArray()
            .Select(e => e.GetProperty("at").GetString() ?? "?")
            .OrderBy(s => s)
            .ToList();
    }

    private static void Check(string what, bool ok)
    {
        if (ok)
        {
            _passed++;
            Log.Info($"[selftest] PASS {what}");
        }
        else
        {
            _failed++;
            Log.Error($"[selftest] FAIL {what}");
        }
    }
}
