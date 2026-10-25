using System.Collections.Generic;
using System.Linq;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Entities.Multiplayer;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer;
using MegaCrit.Sts2.Core.Multiplayer.Game;
using MegaCrit.Sts2.Core.Multiplayer.Game.Lobby;
using MegaCrit.Sts2.Core.Unlocks;

namespace Sts2Net;

/// <summary>The lobby as this family spells it: its services carry a version, and a lobby player has its own type.</summary>
internal static class NetApi
{
    internal static NetHostGameService NewHost() => new(PeerVersionInfo.LocalDefault());

    internal static JoinFlow NewJoin() => new(new NetClientGameService(PeerVersionInfo.LocalDefault()));

    /// <summary>The lobby's players, in the host's order, as the run's.</summary>
    internal static List<Player> Players(StartRunLobby lobby, UnlockState unlocks) =>
        lobby.Players.Select(p => Player.CreateForNewRun(p.character, unlocks, p.id)).ToList();

    internal static IStartRunLobbyListener Listener(TaskCompletionSource<(string, List<ActModel>)> begun) =>
        new Lobby(begun);

    /// <summary>What a lobby screen hears, of which only the run's start matters here.</summary>
    private sealed class Lobby(TaskCompletionSource<(string, List<ActModel>)> begun) : IStartRunLobbyListener
    {
        public void PlayerConnected(StartRunLobbyPlayer player) => Log.Info($"[net] player {player.id} connected");

        public void PlayerChanged(StartRunLobbyPlayer player, bool isRandomCharacterResolution) { }

        public void AscensionChanged() { }

        public void SeedChanged() { }

        public void ModifiersChanged() { }

        public void MaxAscensionChanged() { }

        public void RemotePlayerDisconnected(StartRunLobbyPlayer player) =>
            Log.Warn($"[net] player {player.id} disconnected");

        public void LocalPlayerDisconnected(NetErrorInfo info) => Log.Warn($"[net] disconnected: {info}");

        public void BeginRun(string seed, List<ActModel> acts, IReadOnlyList<ModifierModel> modifiers) =>
            begun.TrySetResult((seed, acts));
    }
}
