using System.Collections.Generic;
using System.Linq;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

/// <summary>
/// Which map points may be travelled to next.
/// </summary>
/// <remarks>
/// A mirror of <c>NMapScreen.RecalculateTravelability</c>, which is the only
/// place in the game that decides this. There is no model-layer equivalent:
/// <c>MapTravel.GetTravelablePointsFrom</c> is one branch of the rule, not the
/// rule, and it has exactly one caller — that screen.
///
/// The oracle originally called <c>MapTravel</c> directly, which was already an
/// improvement on walking <c>Children</c> (the `Winged Boots` bug). It was still
/// wrong at the top of an act, because the boss sits **outside the grid**:
/// <c>BossMapPoint</c> is at row <c>GetRowCount()</c>, and <c>StartingMapPoint</c>
/// and <c>SecondBossMapPoint</c> are likewise absent from <c>Grid</c>, so
/// <c>GetAllMapPoints</c> and <c>GetPointsInRow</c> never yield them.
///
/// The concrete failure that forces this file to exist: standing on the last
/// grid row while free travel is active, <c>MapTravel</c> returns
/// <c>GetPointsInRow(rowCount)</c> — off the end of the grid, so **empty**. The
/// oracle would report no legal move at all on the floor before the boss. Free
/// travel is the exact condition under which the fallback to `Children` (which
/// does contain the boss) is skipped, so the two bugs compose.
///
/// The order of the checks below is the game's and is load-bearing: the boss-row
/// case is tested *before* free travel is consulted, so it wins.
/// </remarks>
public static class MapLegality
{
    public static IReadOnlyList<MapPoint> TravelableFrom(RunState state)
    {
        ActMap map = state.Map;
        IReadOnlyList<MapCoord> visited = state.VisitedMapCoords;

        // Nothing visited yet: the act begins on the starting node, which is not
        // in the grid and is not the same thing as `startMapPoints` — that is
        // the starting node's children, one step further along.
        if (visited.Count == 0)
        {
            return One(map.StartingMapPoint);
        }

        MapCoord last = visited[visited.Count - 1];

        if (map.SecondBossMapPoint != null && last == map.BossMapPoint.coord)
        {
            return One(map.SecondBossMapPoint);
        }

        if (last.row == map.GetRowCount() - 1)
        {
            return One(map.BossMapPoint);
        }

        MapPoint? current = map.GetPoint(last);
        if (current == null)
        {
            // The screen logs an error and falls back to the starting point when
            // the last visited coord is not in the map, which happens when a map
            // is regenerated under a loaded run. Mirrored rather than thrown so
            // the oracle reports what the game would do.
            return One(map.StartingMapPoint);
        }

        return Sorted(MapTravel.GetTravelablePointsFrom(state, current));
    }

    private static IReadOnlyList<MapPoint> One(MapPoint p) => new[] { p };

    /// <summary>Stable order, so two observations of one state compare equal.</summary>
    private static IReadOnlyList<MapPoint> Sorted(IEnumerable<MapPoint> points) =>
        points.OrderBy(p => p.coord.row).ThenBy(p => p.coord.col).ToList();
}
