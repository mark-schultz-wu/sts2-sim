using System.Collections.Generic;
using System.Linq;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Models.Events;
using MegaCrit.Sts2.Core.Rooms;
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
        if (!TravelEnabled(state))
        {
            return new List<MapPoint>();
        }

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

    /// <summary>
    /// Whether the room the run stands in lets it go to the map.
    /// </summary>
    /// <remarks>
    /// A mirror of <c>NMapScreen.IsTravelEnabled</c>, which only screens set.
    /// <c>EnterMapCoord</c> enters any coordinate it is handed, whatever the
    /// flag says, so without this the oracle would leave a room the game
    /// keeps the player in.
    ///
    /// Each room's screen turns it on when it is done with the run.
    /// <c>NMapRoom</c> and <c>NMerchantRoom</c> do as they open, and so does
    /// <c>NFakeMerchant</c>. <c>NRestSiteRoom</c> does once an option went
    /// through, or at once when it offers none. <c>NTreasureRoom</c> does once
    /// the relic is taken or skipped, and <c>NEventRoom.Proceed</c> once the
    /// event is finished. A fight's turns it on when its rewards screen
    /// proceeds, which is once no reward is left to answer.
    /// </remarks>
    public static bool TravelEnabled(RunState state)
    {
        // The map stays shut until the event the fight belongs to is resumed.
        if (ResumesEvent(state)
            || Pending.Current != null
            || CombatManager.Instance.IsInProgress
            || state.Players[0].Creature.IsDead)
        {
            return false;
        }
        return state.CurrentRoom switch
        {
            EventRoom => RunManager.Instance.EventSynchronizer.GetLocalEvent() is var model
                && (model.IsFinished || model is FakeMerchant),
            RestSiteRoom => RestSiteProgress.Rested
                || RunManager.Instance.RestSiteSynchronizer.GetLocalOptions().Count == 0,
            TreasureRoom => !Chest.Open(state.Players[0]),
            _ => true,
        };
    }

    /// <summary>
    /// Whether the run is standing on a beaten boss, with the act behind it.
    /// </summary>
    /// <remarks>
    /// The room half of the precondition
    /// <c>NRewardsScreen.OnProceedButtonPressed</c> tests: a boss room. Kept
    /// here rather than in <c>Protocol</c> because it is a map rule — the answer
    /// to "where can this run go", asked about the one move the grid cannot
    /// express.
    ///
    /// The screen accepts the run's victory room too. That is The Architect's
    /// event room, which shows no rewards screen, so its own <c>PROCEED</c>
    /// option is the only way out.
    ///
    /// The other half is that the fight is over, which the caller checks: there
    /// is no <c>IsFinished</c> on a room, and a boss room with its combat still
    /// running is an ordinary fight. Refusing during combat is what stops this
    /// from being a way to skip one.
    ///
    /// A fight the player died in is over too, and shows the game over screen
    /// rather than rewards. So a dead player proceeds nowhere, which is the
    /// game's <c>IsGameOver</c>.
    /// </remarks>
    public static bool CanProceed(RunState state)
    {
        if (!state.Players[0].Creature.IsAlive)
        {
            return false;
        }

        AbstractRoom? room = state.CurrentRoom;
        return room is { RoomType: RoomType.Boss } || ResumesEvent(state);
    }

    /// <summary>
    /// Whether the run stands in a fight an event entered, which goes back to
    /// that event once it is over.
    /// </summary>
    /// <remarks>
    /// <c>RunManager.ProceedFromTerminalRewardsScreen</c>'s own test, which the
    /// rewards screen's proceed button calls for every terminal set. After any
    /// other fight that call only opens the map. After this one it is the
    /// <c>ResumePreviousRoom</c> that hands the event its <c>Resume</c>, so it is
    /// a move the client makes.
    /// </remarks>
    public static bool ResumesEvent(RunState state) =>
        state.CurrentRoomCount > 1
        && state.CurrentRoom is CombatRoom { ShouldResumeParentEventAfterCombat: true };

    private static IReadOnlyList<MapPoint> One(MapPoint p) => new[] { p };

    /// <summary>Stable order, so two observations of one state compare equal.</summary>
    private static IReadOnlyList<MapPoint> Sorted(IEnumerable<MapPoint> points) =>
        points.OrderBy(p => p.coord.row).ThenBy(p => p.coord.col).ToList();
}
