using System;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Saves;
using MegaCrit.Sts2.Core.Unlocks;
using Sts2Net;

namespace Sts2Headless;

/// <summary>
/// The start of a co-op seat, from what a lobby screen would gather, read from
/// the environment:
/// <list type="bullet">
/// <item><c>STS2_COOP_ROLE</c>: <c>host</c> or <c>client</c>.</item>
/// <item><c>STS2_COOP_PORT</c>: the port the host listens on.</item>
/// <item><c>STS2_COOP_NETID</c>: a client's net id.</item>
/// <item><c>STS2_COOP_PLAYERS</c>: how many seats the host waits for.</item>
/// <item><c>STS2_COOP_CHARACTER</c> and <c>STS2_COOP_SEED</c>.</item>
/// <item><c>STS2_COOP_WINS</c>: the runs the seat's profile has won with
/// each player's character, as <c>CHARACTER=wins</c> pairs joined by commas.
/// It is what <c>start</c> takes as <c>wins</c>, for every player.</item>
/// </list>
/// Every seat has everything unlocked, so no profile changes the run.
/// </summary>
internal static class CoopSeat
{
    private static string Env(string name, string fallback) =>
        Environment.GetEnvironmentVariable(name) ?? fallback;

    internal static Task<RunState> Begin()
    {
        CharacterModel character = Driver.Character(Environment.GetEnvironmentVariable("STS2_COOP_CHARACTER"));
        var session = new Session(
            Host: Env("STS2_COOP_ROLE", "host") == "host",
            Port: ushort.Parse(Env("STS2_COOP_PORT", "33771")),
            NetId: ulong.Parse(Env("STS2_COOP_NETID", "1000")),
            Seats: int.Parse(Env("STS2_COOP_PLAYERS", "2")),
            Character: character,
            Seed: Env("STS2_COOP_SEED", "C000000001"),
            Unlocks: UnlockState.all);
        // TestMode turns the game's divergence check off, as StartRun notes,
        // and a co-op seat wants it most.
        return Net.Begin(session, run =>
        {
            // The Architect picks the dialogue that ends a won run by the
            // local profile's count for its owner's character. Each player's
            // copy reads it, so every player's character is pinned, as a solo
            // run's start pins its own once its run has begun.
            foreach (string pair in Env("STS2_COOP_WINS", "").Split(',', StringSplitOptions.RemoveEmptyEntries))
            {
                string[] parts = pair.Split('=');
                SaveManager.Instance.Progress
                    .GetOrCreateCharacterStats(Driver.Character(parts[0]).Id).TotalWins = int.Parse(parts[1]);
            }
            run.ChecksumTracker.IsEnabled = true;
            Driver.Subscribe(run);
        });
    }
}
