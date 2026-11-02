using System.Collections.Generic;
using System.IO;
using System.Text.Json;
using Godot;
using HarmonyLib;
using MegaCrit.Sts2.Core.GameActions;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.Multiplayer;
using MegaCrit.Sts2.Core.Multiplayer.Game;
using MegaCrit.Sts2.Core.Multiplayer.Transport;
using MegaCrit.Sts2.Core.Multiplayer.Transport.ENet;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

/// <summary>
/// What crossed the network, and the order the host gave the shared actions.
/// </summary>
/// <remarks>
/// The client that drives the seats moves only once the run stands still on
/// every machine. A machine alone cannot tell that, because a packet can be
/// on its way to it. So each machine counts the reliable packets it sent to
/// each other machine, and the packets it took in from each. Once every
/// machine is still and every link's counts agree, no packet is on its way,
/// and nothing can move until the client moves. Unreliable packets carry
/// heartbeats, pings and cursors, which change nothing in the run.
///
/// The order log is for reading a split. It lists the shared actions this
/// machine took in, in order, in <c>order.jsonl</c> under the game's user
/// directory. On the host, that order is the one every machine follows.
/// </remarks>
internal static class Traffic
{
    private static readonly Dictionary<ulong, ulong> _sent = new();
    private static readonly Dictionary<ulong, ulong> _received = new();

    /// <summary>How many shared actions this machine took in.</summary>
    private static ulong _ordered;

    private static StreamWriter? _order;

    /// <summary>
    /// Whether a packet went on the reliable channel. ENet hands a packet up
    /// with no transfer mode, only its channel, so both ends read that.
    /// </summary>
    private static bool Reliable(int channel) => channel == NetTransferMode.Reliable.ToChannelId();

    internal static void Sent(ulong peer, int channel)
    {
        if (Reliable(channel))
        {
            _sent[peer] = _sent.GetValueOrDefault(peer) + 1;
        }
    }

    internal static void Received(ulong peer, int channel)
    {
        if (Reliable(channel))
        {
            _received[peer] = _received.GetValueOrDefault(peer) + 1;
        }
    }

    /// <summary>Appends one shared action to the order log, in a co-op run.</summary>
    internal static void Ordered(string kind, ulong? owner, string what)
    {
        _ordered++;
        if (!Peers.Shared)
        {
            return;
        }
        _order ??= new StreamWriter(Path.Combine(OS.GetUserDataDir(), "order.jsonl"), append: false)
        {
            AutoFlush = true,
        };
        _order.WriteLine(JsonSerializer.Serialize(new Dictionary<string, object?>
        {
            ["at"] = _ordered,
            ["kind"] = kind,
            ["owner"] = owner,
            ["what"] = what,
            ["location"] = RunManager.Instance.RunLocationTargetedBuffer?.CurrentLocation.ToString(),
        }));
    }

    /// <summary>Each other machine's counts, by net id, for the observation.</summary>
    internal static List<Dictionary<string, object?>> Report()
    {
        var peers = new SortedSet<ulong>(_sent.Keys);
        peers.UnionWith(_received.Keys);
        var report = new List<Dictionary<string, object?>>();
        foreach (ulong peer in peers)
        {
            report.Add(new Dictionary<string, object?>
            {
                ["netId"] = peer,
                ["sent"] = _sent.GetValueOrDefault(peer),
                ["received"] = _received.GetValueOrDefault(peer),
            });
        }
        return report;
    }
}

[HarmonyPatch(typeof(ENetHost), nameof(ENetHost.SendMessageToClient))]
internal static class CountHostSends
{
    private static void Prefix(ulong peerId, int channel) => Traffic.Sent(peerId, channel);
}

[HarmonyPatch(typeof(ENetClient), nameof(ENetClient.SendMessageToHost))]
internal static class CountClientSends
{
    private static void Prefix(ENetClient __instance, int channel) =>
        Traffic.Sent(__instance.HostNetId, channel);
}

[HarmonyPatch(typeof(NetHostGameService), nameof(NetHostGameService.OnPacketReceived))]
internal static class CountHostReceipts
{
    private static void Prefix(ulong senderId, int channel) => Traffic.Received(senderId, channel);
}

[HarmonyPatch(typeof(NetClientGameService), nameof(NetClientGameService.OnPacketReceived))]
internal static class CountClientReceipts
{
    private static void Prefix(ulong senderId, int channel) => Traffic.Received(senderId, channel);
}

/// <summary>
/// Logs the shared actions this machine takes in, in order.
/// </summary>
/// <remarks>
/// Each of the three is where <c>ActionQueueSynchronizer</c> puts a shared
/// action into the queues here, after it has passed it on to the clients
/// where this machine hosts. Logging changes nothing the game does.
/// </remarks>
[HarmonyPatch(typeof(ActionQueueSynchronizer))]
internal static class LogSharedOrder
{
    [HarmonyPostfix]
    [HarmonyPatch("EnqueueAction")]
    private static void Enqueued(GameAction action, ulong actionOwnerId) =>
        Traffic.Ordered("action", actionOwnerId, action.ToString() ?? "");

    [HarmonyPostfix]
    [HarmonyPatch("EnqueueHookAction")]
    private static void HookEnqueued(GenericHookGameAction gameAction) =>
        Traffic.Ordered("hook", gameAction.OwnerId, $"hook {gameAction.HookId}");

    [HarmonyPostfix]
    [HarmonyPatch("ResumeActionAfterPlayerChoice")]
    private static void Resumed(uint id) => Traffic.Ordered("resume", null, $"action {id}");
}

/// <summary>Logs an action cancelled here, which the turn's end or a won fight does.</summary>
[HarmonyPatch(typeof(GameAction), nameof(GameAction.Cancel))]
internal static class LogCancelled
{
    private static void Postfix(GameAction __instance) =>
        Traffic.Ordered("cancel", __instance.OwnerId, __instance.ToString() ?? "");
}
