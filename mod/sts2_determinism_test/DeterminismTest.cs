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
using MegaCrit.Sts2.Core.Saves;
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
        string? mode = Environment.GetEnvironmentVariable("STS2_HEADLESS_MODE");

        if (mode == "determinism")
        {
            // Not awaited: Init must return so startup can finish populating the
            // model database, which the driver's first stage waits for.
            _ = Driver.Run(Run);
        }
        else if (mode == "carddump")
        {
            _ = Driver.Run(_ =>
            {
                CardDump.Run();
                return Task.CompletedTask;
            });
        }
        else if (mode == "rngdump")
        {
            // A second mode rather than a second mod: it needs nothing from a
            // run, only the assembly, and it belongs beside the other thing here
            // that measures the game rather than driving it.
            _ = Driver.Run(_ =>
            {
                RngDump.Run();
                return Task.CompletedTask;
            });
        }
    }

    public static Task Run(RunState state)
    {
        EncounterRngIsSeededFromTheRun(state);
        TutorialsAreSuppressedWithoutWritingThem();

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

    /// <summary>
    /// The claim: tutorials never appear, and the profile is not written to say
    /// so.
    /// </summary>
    /// <remarks>
    /// The second half is the point. Both obvious alternatives write:
    /// <c>SetFtuesEnabled(false)</c> calls <c>SaveProgress()</c>, and the
    /// <c>EnableFtues</c> property behind it is read by <c>ProgressState</c>'s
    /// own serialisation, so forcing it would persist into the player's save.
    /// D3 wants the driver to leave no trace, and "we disabled tutorials in
    /// your profile" is a trace.
    ///
    /// So this sets the flag <em>on</em> and checks that tutorials stay
    /// suppressed anyway, and that asking has not turned it off. Asserting it
    /// was already on would test history rather than behaviour: this profile
    /// carries <c>EnableFtues = false</c>, written by the driver's old
    /// <c>SetFtuesEnabled(false)</c> call over the weeks before the patch
    /// replaced it — which is itself the evidence that the old approach wrote.
    ///
    /// Nor can this assert <c>progress.save</c> is untouched. A run writes it
    /// for reasons that have nothing to do with tutorials: <c>RelicCmd.Obtain</c>
    /// calls <c>MarkRelicAsSeen</c>, and fighting records a discovery.
    /// </remarks>
    private static void TutorialsAreSuppressedWithoutWritingThem()
    {
        // In memory only; nothing here calls SaveProgress. It also happens to
        // undo the residue described above, the next time the game saves for its
        // own reasons.
        SaveManager.Instance.Progress.EnableFtues = true;

        object manager = typeof(SaveManager)
            .GetField("_progressSaveManager", BindingFlags.Instance | BindingFlags.NonPublic)
            ?.GetValue(SaveManager.Instance)
            ?? throw new InvalidOperationException(
                "SaveManager._progressSaveManager is gone; this test needs rewriting");

        MethodInfo seen = manager.GetType().GetMethod("SeenFtue")
            ?? throw new InvalidOperationException(
                "ProgressSaveManager.SeenFtue is gone; this test needs rewriting");

        // A key that cannot be in the completed set. Written first with
        // "shuffle_ftue", which this profile had already completed, so unpatched
        // SeenFtue answered true from FtueCompleted and the mutation test
        // survived — the assertion looked right and checked nothing.
        string key = "determinism_test_" + Guid.NewGuid().ToString("N");
        bool answered = (bool)seen.Invoke(manager, new object[] { key })!;
        Check("an unseen tutorial reports as already seen, with FTUEs enabled", answered);

        Check(
            "asking did not turn the profile's FTUE flag off",
            SaveManager.Instance.Progress.EnableFtues);
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
