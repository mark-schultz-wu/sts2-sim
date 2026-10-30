using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Context;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.GameActions;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.Multiplayer.Game;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

/// <summary>
/// What a co-op run's screens show about the other seats: who has ended the
/// turn, where each player voted to go, and whose choice the game waits on.
/// A run of one has no other seats, and none of this applies to it.
/// </summary>
/// <remarks>
/// Facts, not conclusions. Whether a seat should wait for the others is the
/// client's to work out from them. This mod reads them for one thing more:
/// whether the game here can go on without another seat, which is when a
/// command has done all it can and answers.
/// </remarks>
internal static class Peers
{
    /// <summary>Whether this process is one seat of several.</summary>
    internal static bool Shared =>
        RunManager.Instance.NetService.Type is NetGameType.Host or NetGameType.Client;

    /// <summary>How many actions this seat queued that have not started here yet.</summary>
    private static int _sent;

    /// <summary>
    /// Whether an action this seat queued has not come back to run here yet.
    /// A client's goes to the host first, which sends it back, and the queue
    /// can stand empty meanwhile.
    /// </summary>
    internal static bool InFlight => _sent > 0 || _hooks > 0;

    /// <summary>
    /// How many of the local player's hook actions this client asked the
    /// host to queue, whose word has not come back yet.
    /// </summary>
    private static int _hooks;

    /// <summary>
    /// Queues one of the local player's actions, as a screen does: through
    /// <c>ActionQueueSynchronizer.RequestEnqueue</c>, which in a co-op run sends
    /// it to every seat and alone queues it here.
    /// </summary>
    internal static void Enqueue(GameAction action)
    {
        if (Shared)
        {
            _sent++;
        }
        RunManager.Instance.ActionQueueSynchronizer.RequestEnqueue(action);
    }

    /// <summary>
    /// The local player's vote to move to the next act, which
    /// <c>ActChangeSynchronizer.SetLocalPlayerReady</c> queues as
    /// <see cref="Enqueue"/> does. It builds the action, whose constructor
    /// differs between builds.
    /// </summary>
    internal static void VoteForNextAct()
    {
        if (Shared)
        {
            _sent++;
        }
        RunManager.Instance.ActChangeSynchronizer.SetLocalPlayerReady();
    }

    private static readonly FieldInfo ReadyPlayers = typeof(ActChangeSynchronizer)
        .GetField("_readyPlayers", BindingFlags.NonPublic | BindingFlags.Instance)!;

    /// <summary>
    /// Whether <paramref name="player"/> voted to move to the next act, and
    /// waits for the rest. The flags clear as the run moves on.
    /// </summary>
    internal static bool VotedNextAct(RunState state, Player player)
    {
        var ready = (List<bool>)ReadyPlayers.GetValue(RunManager.Instance.ActChangeSynchronizer)!;
        return ready[state.GetPlayerSlotIndex(player)];
    }

    /// <summary>
    /// A hook action of the local player's that a client asks the host to
    /// queue, as a paused hook's choice does. It comes back to the queue here
    /// only once the host sends it back, and the queue can stand empty
    /// meanwhile.
    /// </summary>
    internal static void HookRequested(GenericHookGameAction action)
    {
        if (RunManager.Instance.NetService.Type == NetGameType.Client
            && action.OwnerId == LocalContext.NetId)
        {
            _hooks++;
        }
    }

    /// <summary>
    /// A hook action of the local player's back from the host, queued here
    /// or turned away, as a queue cancelling its combat actions does.
    /// </summary>
    internal static void HookCameBack(GameAction action)
    {
        if (_hooks > 0 && action is GenericHookGameAction && action.OwnerId == LocalContext.NetId)
        {
            _hooks--;
        }
    }

    /// <summary>One of the local player's actions starting, which is its copy come back.</summary>
    internal static void Started(GameAction action)
    {
        if (_sent > 0 && action.OwnerId == LocalContext.NetId)
        {
            _sent--;
        }
    }

    /// <summary>Whether <paramref name="player"/> has ended its turn and waits for the rest.</summary>
    internal static bool EndedTurn(Player player) =>
        CombatManager.Instance.IsInProgress && player.PlayerCombatState != null
        && CombatManager.Instance.IsPlayerReadyToEndTurn(player);

    /// <summary>Where <paramref name="player"/> voted to go from here, if it has.</summary>
    internal static MapVote? Vote(Player player)
    {
        var votes = RunManager.Instance.MapSelectionSynchronizer;
        try
        {
            MapVote? vote = votes.GetVote(player);
            return vote.HasValue && vote.Value.mapGenerationCount == votes.MapGenerationCount ? vote : null;
        }
        catch (System.ArgumentOutOfRangeException)
        {
            // No votes are open, as before the first room.
            return null;
        }
    }

    private static readonly FieldInfo ReceivedChoices = typeof(PlayerChoiceSynchronizer)
        .GetField("_receivedChoices", BindingFlags.NonPublic | BindingFlags.Instance)!;

    /// <summary>
    /// The players whose choice <c>PlayerChoiceSynchronizer.WaitForRemoteChoice</c>
    /// waits on, by net id.
    /// </summary>
    internal static IReadOnlyList<ulong> AwaitedChoices()
    {
        var awaited = new List<ulong>();
        var received = (System.Collections.IList)ReceivedChoices.GetValue(RunManager.Instance.PlayerChoiceSynchronizer)!;
        foreach (object choice in received)
        {
            var type = choice.GetType();
            object? source = type.GetField("completionSource")!.GetValue(choice);
            if (source?.GetType().GetProperty("Task")!.GetValue(source) is Task { IsCompleted: false })
            {
                awaited.Add((ulong)type.GetField("senderId")!.GetValue(choice)!);
            }
        }
        return awaited;
    }

    /// <summary>
    /// Whether the game here cannot go on until another seat acts: this seat
    /// has ended its turn or voted, or the game waits on another player's
    /// choice.
    /// </summary>
    internal static bool HeldByOthers(RunState state)
    {
        if (!Shared)
        {
            return false;
        }
        Player me = Seat.Me(state);
        return EndedTurn(me) || Vote(me) != null || AwaitedChoices().Count > 0;
    }

    /// <summary>The facts above, for the observation.</summary>
    internal static Dictionary<string, object?> Report(RunState state) => new()
    {
        ["players"] = state.Players.Select(p => new Dictionary<string, object?>
        {
            ["netId"] = p.NetId,
            ["endedTurn"] = EndedTurn(p),
            ["vote"] = Vote(p) is MapVote vote ? $"{vote.coord.col},{vote.coord.row}" : null,
        }).ToList(),
        ["awaiting"] = AwaitedChoices(),
        ["inFlight"] = InFlight,
        ["still"] = Driver.Still(new HashSet<ulong>(), null),
        ["links"] = Traffic.Report(),
    };
}

/// <summary>Counts a client's hook action request as in flight, as <see cref="Peers.Enqueue"/> counts an action.</summary>
[HarmonyLib.HarmonyPatch(typeof(ActionQueueSynchronizer), nameof(ActionQueueSynchronizer.RequestEnqueueHookAction))]
internal static class HookRequestInFlight
{
    private static void Prefix(GenericHookGameAction action) => Peers.HookRequested(action);
}

/// <summary>Takes a client's hook action request out of flight once it comes back to the queue.</summary>
[HarmonyLib.HarmonyPatch(typeof(ActionQueueSet), nameof(ActionQueueSet.EnqueueWithoutSynchronizing))]
internal static class HookRequestBack
{
    private static void Postfix(GameAction gameAction) => Peers.HookCameBack(gameAction);
}
