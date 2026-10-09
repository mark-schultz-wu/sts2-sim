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
    /// Votes for a relic — or, one past the last, for none — and awards it.
    /// </summary>
    /// <remarks>
    /// Subscribed around the vote rather than once at startup because the
    /// synchronizer belongs to the run, not the process: a second `start` builds
    /// a new one, and a handler left on the first would be listening to a run
    /// that no longer exists.
    /// </remarks>
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
        List<RelicPickingResult> awarded = new List<RelicPickingResult>();
        void Collect(List<RelicPickingResult> results) => awarded.AddRange(results);

        sync.RelicsAwarded += Collect;
        try
        {
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
        finally
        {
            sync.RelicsAwarded -= Collect;
        }

        // The screen's own loop, minus the animation: obtain what was won, and
        // put what nobody won back where the other players' grab bags reach it.
        foreach (RelicPickingResult result in awarded)
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
                //
                // Not covered by a live test: reaching this needs a chest that
                // offers one of the ten relics whose AfterObtained enchants,
                // which is seed hunting. `grant` reaches the same hook directly
                // and is tested there.
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
