using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Threading.Tasks;
using Godot;
using MegaCrit.Sts2.Core.Assets;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer;
using MegaCrit.Sts2.Core.Multiplayer.Connection;
using MegaCrit.Sts2.Core.Multiplayer.Game;
using MegaCrit.Sts2.Core.Multiplayer.Game.Lobby;
using MegaCrit.Sts2.Core.Nodes;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Unlocks;

namespace Sts2Net;

/// <summary>One seat's part in a co-op session, as a lobby screen gathers it.</summary>
/// <param name="Host">Whether this seat hosts. The host's net id is always 1.</param>
/// <param name="Port">The port the host listens on, on this machine.</param>
/// <param name="NetId">A client's net id, which the client picks.</param>
/// <param name="Seats">How many seats the host waits for.</param>
/// <param name="Character">The character this seat plays.</param>
/// <param name="Seed">The run's seed, which the host sets.</param>
/// <param name="Unlocks">What each player starts with unlocked.</param>
/// <param name="JoinAfter">A file the client before this one writes once it
/// has joined, which this client waits for.</param>
/// <param name="Joined">A file this client writes once it has joined.</param>
public sealed record Session(
    bool Host, ushort Port, ulong NetId, int Seats, CharacterModel Character, string Seed, UnlockState Unlocks,
    string? JoinAfter = null, string? Joined = null);

/// <summary>
/// Co-op over the game's ENet transport, with no Steam and no lobby screen.
/// </summary>
/// <remarks>
/// The lobby and the join are the game's own, as its debug multiplayer scene
/// drives them. What a scene does besides is done here: it pumps the net
/// service every frame, and it starts the run as
/// <c>NGame.StartNewMultiplayerRun</c> does, without the run scene.
/// </remarks>
public static class Net
{
    /// <summary>
    /// Hosts or joins, and returns the run once it stands on its first map.
    /// <paramref name="setUp"/> runs once the run is set up and before it
    /// launches.
    /// </summary>
    public static async Task<RunState> Begin(Session session, Action<RunManager>? setUp = null)
    {
        SceneTree tree = NGame.Instance!.GetTree();
        var begun = new TaskCompletionSource<(string, List<ActModel>)>();
        IStartRunLobbyListener listener = NetApi.Listener(begun);

        StartRunLobby lobby;
        if (session.Host)
        {
            NetHostGameService host = NetApi.NewHost();
            var error = host.StartENetHost(session.Port, session.Seats - 1);
            if (error != null)
            {
                throw new InvalidOperationException($"the ENet host did not start: {error}");
            }
            _ = Pump(host, tree);
            lobby = new StartRunLobby(GameMode.Standard, host, listener, session.Seats);
            lobby.AddLocalHostPlayer(session.Unlocks, 0);
            lobby.SetSeed(session.Seed);
            lobby.SetLocalCharacter(session.Character);
            Log.Info($"[net] hosting on {session.Port} for {session.Seats} seats");
            while (lobby.Players.Count < session.Seats)
            {
                await tree.ToSignal(tree, SceneTree.SignalName.ProcessFrame);
            }
            // The run's players follow the lobby's join order, which the
            // clients keep to seat order.
            if (!InSeatOrder(lobby.Players.Select(player => player.id).ToList(), session.Seats))
            {
                throw new InvalidOperationException(
                    $"the clients joined out of seat order: {string.Join(", ", lobby.Players.Select(player => player.id))}");
            }
            lobby.SetReady(true);
        }
        else
        {
            (JoinFlow join, JoinResult joined) = await Join(session, tree);
            INetGameService service = join.NetService
                ?? throw new InvalidOperationException("joined with no net service");
            _ = Pump(service, tree);
            lobby = new StartRunLobby(joined.gameMode, service, listener, -1);
            lobby.InitializeFromMessage(joined.joinResponse
                ?? throw new InvalidOperationException("the host answered the join with no lobby"));
            lobby.SetLocalCharacter(session.Character);
            lobby.SetReady(true);
            Log.Info($"[net] joined {session.Port} as {session.NetId}");
        }

        (string seed, List<ActModel> acts) = await begun.Task;
        Log.Info($"[net] the run begins, seed {seed}, {lobby.Players.Count} players");

        // As NGame.StartNewMultiplayerRun builds it, with the unlocks the
        // session names. No modifiers, as the game's own standard run passes
        // none.
        RunState state = RunState.CreateForNewRun(
            NetApi.Players(lobby, session.Unlocks),
            acts.Select(a => a.ToMutable()).ToList(),
            Array.Empty<ModifierModel>(),
            lobby.GameMode,
            lobby.Ascension,
            seed);
        RunManager run = RunManager.Instance;
        run.SetUpNewMultiplayer(state, lobby, false);
        setUp?.Invoke(run);
        await PreloadManager.LoadRunAssets(state.Players.Select(p => p.Character));
        await PreloadManager.LoadActAssets(state.Acts[0]);
        await run.FinalizeStartingRelics();
        run.Launch();
        await run.EnterAct(0, doTransition: false);
        lobby.CleanUp(disconnectSession: false);
        return state;
    }

    /// <summary>
    /// What a run scene's <c>_Process</c> does for the net service: one update a
    /// frame, for as long as the process lives.
    /// </summary>
    /// <summary>
    /// Joins the host, and again after a failed try, as a player does after
    /// the join screen's error. A seat can start before its host listens.
    /// </summary>
    private static async Task<(JoinFlow, JoinResult)> Join(Session session, SceneTree tree)
    {
        // The client before this one joins first, so the lobby seats the
        // clients in seat order.
        while (session.JoinAfter != null && !File.Exists(session.JoinAfter))
        {
            await tree.ToSignal(tree, SceneTree.SignalName.ProcessFrame);
        }
        while (true)
        {
            JoinFlow join = NetApi.NewJoin();
            try
            {
                JoinResult joined = await join.Begin(
                    new ENetClientConnectionInitializer(session.NetId, "127.0.0.1", session.Port), tree);
                if (session.Joined != null)
                {
                    File.WriteAllText(session.Joined, session.NetId.ToString());
                }
                return (join, joined);
            }
            catch (ClientConnectionFailedException e)
            {
                Log.Info($"[net] no host on {session.Port} yet: {e.Message}");
                await tree.ToSignal(tree.CreateTimer(1.0), SceneTreeTimer.SignalName.Timeout);
            }
        }
    }

    /// <summary>The net id the first client plays under, as the driver gives it.</summary>
    private const ulong FirstClientNetId = 1001;

    /// <summary>
    /// Whether the players' net ids, in join order, are the host's and then
    /// the clients' up to <paramref name="count"/> players, in seat order and
    /// no more.
    /// </summary>
    private static bool InSeatOrder(IReadOnlyList<ulong> ids, int count) =>
        ids.Count == count
        && ids.Select((id, at) => id == (at == 0 ? 1UL : FirstClientNetId + (ulong)at - 1))
            .All(inPlace => inPlace);

    private static async Task Pump(INetGameService service, SceneTree tree)
    {
        while (true)
        {
            service.Update();
            await tree.ToSignal(tree, SceneTree.SignalName.ProcessFrame);
        }
    }
}
