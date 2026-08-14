using System;
using System.Reflection;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Modding;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Models.Encounters;
using MegaCrit.Sts2.Core.Random;
using MegaCrit.Sts2.Core.Runs;
using Sts2Headless;

namespace Sts2DeterminismTest;

/// <summary>
/// Measures what <c>sts2_determinism</c> claims, rather than arguing it.
/// </summary>
/// <remarks>
/// Its own mod so the shipped determinism patches carry no tests, and so this
/// can be installed and removed independently of the driver's own suite.
///
/// It has to run in-process: the claims are about private state inside the
/// game's own objects — whether an encounter's RNG was seeded, and from what —
/// and nothing over the protocol can see that.
/// </remarks>
[ModInitializer(nameof(Init))]
public static class DeterminismTest
{
    private static int _passed;
    private static int _failed;

    /// <summary>
    /// Entry point. Claims <c>STS2_HEADLESS_MODE=determinism</c>.
    /// </summary>
    public static void Init()
    {
        if (Environment.GetEnvironmentVariable("STS2_HEADLESS_MODE") == "determinism")
        {
            // Not awaited: Init must return so startup can finish populating the
            // model database, which the driver's first stage waits for.
            _ = Driver.Run(Run);
        }
    }

    public static Task Run(RunState state)
    {
        EncounterRngIsSeededFromTheRun(state);

        Log.Info($"[determinism-test] {_passed} passed, {_failed} failed");
        if (_failed > 0)
        {
            // Not Environment.Exit: the driver runs on Godot's main thread and
            // exiting from under it hangs rather than exits. Driver.Run's own
            // shutdown reads this.
            Driver.ExitCode = 1;
        }

        return Task.CompletedTask;
    }

    /// <summary>
    /// The claim: <c>DebugRandomizeRng</c> no longer pre-empts the game's own
    /// seeding, so an encounter's RNG comes from the run rather than the clock.
    /// </summary>
    /// <remarks>
    /// Asserted against the seed actually used, not by building the same fight
    /// twice and checking the results match. That weaker test was the obvious
    /// one and it is close to worthless here: <c>BowlbugsWeak</c> picks its
    /// second monster from two options, so an unpatched build agrees with itself
    /// half the time by chance. A regression would show up as a coin flip, which
    /// is worse than not testing it — a flaky failure gets rerun until it
    /// passes.
    ///
    /// Reflection over <c>_rng</c> because the game exposes it only as a
    /// <c>protected</c> member. That is a real cost: this test knows a private
    /// field name and will break if the game renames it. It breaks loudly, which
    /// is the trade.
    /// </remarks>
    private static void EncounterRngIsSeededFromTheRun(RunState state)
    {
        EncounterModel encounter = ModelDb.Encounter<BowlbugsWeak>().ToMutable();

        Check("a fresh encounter has no rng yet", RngOf(encounter) == null);

        // Unpatched, this assigns a wall-clock Rng and the next check fails.
        encounter.DebugRandomizeRng();
        Check("DebugRandomizeRng leaves the rng unset", RngOf(encounter) == null);

        encounter.GenerateMonstersWithSlots(state);

        Rng? seeded = RngOf(encounter);
        if (seeded == null)
        {
            Check("generating monsters seeds the rng", false);
            return;
        }

        // The formula in EncounterModel.GenerateMonstersWithSlots.
        uint expected = (uint)((int)state.Rng.Seed
            + state.TotalFloor
            + StringHelper.GetDeterministicHashCode(encounter.Id.Entry));

        Check(
            $"the rng is seeded from the run, not the clock (got {seeded.Seed}, expected {expected})",
            seeded.Seed == expected);

        // A wall-clock seed is seconds since the epoch: about 1.7 billion, and
        // climbing. Worth stating separately, because it is the failure this
        // exists to catch and the number makes it unmistakable.
        uint now = (uint)DateTimeOffset.UtcNow.ToUnixTimeSeconds();
        Check(
            $"the rng is not a recent wall-clock value ({seeded.Seed} vs ~{now})",
            seeded.Seed < now - 86400 || seeded.Seed > now + 86400);
    }

    private static Rng? RngOf(EncounterModel encounter)
    {
        FieldInfo field = typeof(EncounterModel).GetField(
            "_rng", BindingFlags.Instance | BindingFlags.NonPublic)
            ?? throw new InvalidOperationException(
                "EncounterModel._rng is gone; this test needs rewriting against whatever replaced it");

        return (Rng?)field.GetValue(encounter);
    }

    private static void Check(string what, bool ok)
    {
        if (ok)
        {
            _passed++;
            Log.Info($"[determinism-test] PASS {what}");
        }
        else
        {
            _failed++;
            Log.Error($"[determinism-test] FAIL {what}");
        }
    }
}
