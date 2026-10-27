using HarmonyLib;
using System.Collections.Generic;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Entities.TreasureRelicPicking;
using MegaCrit.Sts2.Core.Multiplayer.Game;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

/// <summary>
/// Answering a treasure room, and receiving what it gives.
/// </summary>
/// <remarks>
/// Its own file because it is two halves that only work together, and because
/// the protocol handler and the self-test both need the pair.
///
/// The first half is that a chest <em>must</em> be answered.
/// <c>TreasureRoom.EnterInternal</c> calls <c>BeginRelicPicking</c>, which opens
/// a session and waits for a vote; <c>OnRoomExited</c> only closes one that was
/// skipped. So a run that walks away from a chest carries the open session to
/// the next one, which throws — one room too late to say what caused it.
///
/// The second is that the synchronizer decides who gets what and announces it
/// without handing anything over: <c>NTreasureRoomRelicCollection</c> calls
/// <c>RelicCmd.Obtain</c> from its <c>RelicsAwarded</c> handler, in among the
/// animation. Headless, the vote lands, the chest closes, and no relic arrives.
/// Measured before this existed: picking `CHANDELIER` left the relic list
/// unchanged.
/// </remarks>
public static class Chest
{
    /// <summary>
    /// Votes for a relic, or, one past the last, for none. The relics are
    /// awarded by <see cref="Award"/> once every player has voted.
    /// </summary>
    public static async Task Pick(RunState state, int index)
    {
        // `NTreasureRoom.OpenChest` gives the chest's gold, and pays the Spoils
        // Map, before the relics are shown. A pick or a skip opens the chest
        // first, which is the only way a player gets at either. Its
        // `DoExtraRewardsIfNeeded` comes next on screen, and is left out: it
        // shows a rewards screen only for rewards a relic added to the room.
        if (state.CurrentRoom is TreasureRoom room)
        {
            await room.DoNormalRewards();
        }
        TreasureRoomRelicSynchronizer sync = RunManager.Instance.TreasureRoomRelicSynchronizer;
        int relics = sync.CurrentRelics?.Count ?? 0;
        if (index == relics)
        {
            sync.SkipRelicLocally();
        }
        else
        {
            sync.PickRelicLocally(index);
        }
        await Driver.WaitForDecisionPoint();
    }

    /// <summary>
    /// <c>NTreasureRoomRelicCollection.AnimateRelicAwards</c>, minus the
    /// animation: the results sorted by type, each one won obtained, and each
    /// relic moved to the fallback of every other player's bag.
    /// </summary>
    /// <remarks>
    /// The awards come when the last vote does, which in a run of several
    /// players can be another player's. So this hears them for the whole room,
    /// from <c>BeginRelicPicking</c>, rather than around the local pick.
    /// </remarks>
    internal static void Award(List<RelicPickingResult> results)
    {
        RunState? state = RunManager.Instance.DebugOnlyGetState();
        if (state == null)
        {
            return;
        }
        results.Sort((r1, r2) => r1.type.CompareTo(r2.type));
        foreach (RelicPickingResult result in results)
        {
            if (result.type != RelicPickingResultType.Skipped && result.player != null)
            {
                // Tracked, never awaited, for the reason `Protocol.Grant` gives:
                // a relic's AfterObtained can park a question -- Royal Stamp
                // opens a one-card selection to enchant -- and this runs inside
                // the protocol handler, which cannot send that question while
                // it is blocked here. The caller's WaitForDecisionPoint covers
                // both cases, so a relic that asks nothing still lands before
                // the observation.
                BackgroundWork.Track(RelicCmd.Obtain(result.relic.ToMutable(), result.player));
            }
            foreach (Player other in state.Players)
            {
                if (other != result.player)
                {
                    other.RelicGrabBag.MoveToFallback(result.relic);
                }
            }
        }
    }

    /// <summary>Whether this chest still wants an answer.</summary>
    /// <remarks>
    /// Taking a relic ends the session and clears <c>CurrentRelics</c>, so that
    /// case answers itself. Skipping does not: in a single-player run the skip is
    /// remembered and the session is only torn down when the room is left, so the
    /// chest would go on offering relics it has already refused. The vote is what
    /// separates the two.
    /// </remarks>
    public static bool Open(Player me)
    {
        TreasureRoomRelicSynchronizer sync = RunManager.Instance.TreasureRoomRelicSynchronizer;
        return sync.CurrentRelics != null && !sync.GetPlayerVote(me).voteReceived;
    }
}

/// <summary>
/// <see cref="Chest.Award"/> heard for each chest, from the moment the picking
/// opens.
/// </summary>
/// <remarks>
/// Unhooked and hooked again each time, as the synchronizer belongs to the run
/// and a second run builds a new one.
/// </remarks>
[HarmonyPatch(typeof(TreasureRoomRelicSynchronizer), nameof(TreasureRoomRelicSynchronizer.BeginRelicPicking))]
internal static class HearChestAwards
{
    [HarmonyPrefix]
    private static void Hear(TreasureRoomRelicSynchronizer __instance)
    {
        __instance.RelicsAwarded -= Chest.Award;
        __instance.RelicsAwarded += Chest.Award;
    }
}
