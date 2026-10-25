using System;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Runs;
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
/// </list>
/// Every seat has everything unlocked, so no profile changes the run.
/// </summary>
internal static class CoopSeat
{
    private static string Env(string name, string fallback) =>
        Environment.GetEnvironmentVariable(name) ?? fallback;

    internal static Task<RunState> Begin()
    {
        var session = new Session(
            Host: Env("STS2_COOP_ROLE", "host") == "host",
            Port: ushort.Parse(Env("STS2_COOP_PORT", "33771")),
            NetId: ulong.Parse(Env("STS2_COOP_NETID", "1000")),
            Seats: int.Parse(Env("STS2_COOP_PLAYERS", "2")),
            Character: Driver.Character(Environment.GetEnvironmentVariable("STS2_COOP_CHARACTER")),
            Seed: Env("STS2_COOP_SEED", "COOP1"),
            Unlocks: UnlockState.all);
        // TestMode turns the game's divergence check off, as StartRun notes,
        // and a co-op seat wants it most.
        return Net.Begin(session, run =>
        {
            run.ChecksumTracker.IsEnabled = true;
            Driver.Subscribe(run);
        });
    }
}
