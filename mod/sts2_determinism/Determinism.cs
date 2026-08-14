using MegaCrit.Sts2.Core.Debug;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Modding;
using MegaCrit.Sts2.Core.Saves;
using MegaCrit.Sts2.Core.TestSupport;

namespace Sts2Determinism;

/// <summary>
/// Makes Slay the Spire 2 reproducible: the same seed produces the same run.
/// </summary>
/// <remarks>
/// This mod adds no content, changes no rules, and drives nothing. It exists so
/// that "the game is deterministic" is one concern owned in one place, rather
/// than a handful of assignments scattered through whatever happens to be
/// driving the game — which is where they used to live, with the consequence
/// that two different drivers configured the game two different ways.
///
/// It was going to do this by patching, so that it held however the game was
/// driven. It cannot: Harmony throws
/// <c>DllNotFoundException: /tmp/mm-exhelper.so: undefined symbol
/// _Unwind_RaiseException</c> on the first patch, because MonoMod's native
/// exception helper will not load against this Godot .NET runtime. That is
/// almost certainly why this repository has referenced <c>0Harmony</c> since the
/// beginning and never once used it.
///
/// So the per-run settings are a call, and <see cref="PrepareRun"/> is the one
/// place to make it from. A driver that has to remember to call something can
/// forget — but one call to one named thing is a great deal harder to forget
/// than four assignments scattered through whatever happens to be driving.
/// </remarks>
[ModInitializer(nameof(Init))]
public static class Determinism
{
    /// <summary>
    /// The build this was verified against: v0.107.1, buildid 23811903, from
    /// <c>release_info.json</c>.
    /// </summary>
    /// <remarks>
    /// Steam can update the install underneath us. Every claim this mod makes is
    /// a claim about one build's behaviour, so a mismatch is refused rather than
    /// warned about.
    /// </remarks>
    private const int PinnedAssemblyHash = 1692500715;

    private const string PinnedVersion = "v0.107.1";

    /// <summary>
    /// Whether the game was actually made reproducible.
    /// </summary>
    /// <remarks>
    /// Dependent mods must check this rather than assuming. The loader
    /// guarantees this mod <em>loaded</em> before them — it refuses to load a
    /// mod whose dependencies are absent — but loading and succeeding are
    /// different things: a build mismatch leaves this false with the game
    /// untouched.
    /// </remarks>
    public static bool Active { get; private set; }

    public static void Init()
    {
        if (!VerifyPin())
        {
            return;
        }

        // The game's own test seam. Skips every Cmd.Wait and the animation paths
        // that check it, which is both a speed and a determinism matter: waits
        // that depend on frame timing are waits whose outcome depends on the
        // machine. Verified to gate presentation only — MapCmd applies its model
        // change unconditionally and skips just the icon refresh, and nothing
        // calls TestMode.AssertOff.
        TestMode.IsOn = true;

        Active = true;
        Log.Info($"[determinism] active on {PinnedVersion}; TestMode on");
    }

    /// <summary>
    /// Everything that has to be reapplied per run, rather than once per process.
    /// </summary>
    /// <remarks>
    /// Call this before starting a run and after any restart.
    ///
    /// First-time-user prompts both gate progress — a driver waits forever on a
    /// prompt nothing will answer — and write to disk, so whether a profile has
    /// seen one becomes state that persists across runs and that no seed
    /// accounts for.
    /// </remarks>
    public static void PrepareRun()
    {
        SaveManager.Instance.SetFtuesEnabled(enabled: false);
    }

    private static bool VerifyPin()
    {
        ReleaseInfo? release = ReleaseInfoManager.Instance.ReleaseInfo;
        if (release == null)
        {
            Log.Error("[determinism] refusing to start: no release_info.json, so the build cannot be identified");
            return false;
        }

        if (release.MainAssemblyHash != PinnedAssemblyHash)
        {
            Log.Error(
                $"[determinism] refusing to start: assembly hash {release.MainAssemblyHash} "
                + $"is not the pinned {PinnedAssemblyHash} (game reports {release.Version}, "
                + $"commit {release.Commit}). The install changed; re-verify before trusting it.");
            return false;
        }

        return true;
    }
}
