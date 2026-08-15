using System;
using System.IO;
using System.Linq;
using System.Reflection;
using HarmonyLib;
using MegaCrit.Sts2.Core.Debug;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Modding;
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
/// Everything here is a patch rather than a call, deliberately: a driver that
/// has to remember to configure the game is a driver that can forget, and the
/// second one always does.
///
/// That requires <c>LD_PRELOAD=libgcc_s.so.1</c> when launching the game. See
/// the README — without it every patch throws and this mod stays inert.
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

        // The loader only runs PatchAll by itself for assemblies with no
        // ModInitializer, and this one has one.
        new Harmony("sts2_determinism").PatchAll(Assembly.GetExecutingAssembly());

        Active = true;
        Log.Info($"[determinism] active on {PinnedVersion}; TestMode on, patches applied");

        SweepPatchHelpers();
    }

    /// <summary>
    /// Deletes the native helpers MonoMod leaves behind.
    /// </summary>
    /// <remarks>
    /// Patching extracts a helper to the temp directory, <c>dlopen</c>s it, and
    /// never unlinks it — one per process, forever. 384 of them totalling 7.5MB
    /// had accumulated here before anyone looked, over eleven days. We are the
    /// reason they exist, so we are the ones to clean them up.
    ///
    /// Only ones older than a minute, which leaves alone any that another
    /// process has just written and not yet opened. Deleting one already mapped
    /// is safe on Linux — the mapping outlives the directory entry — but there
    /// is no reason to race for it.
    /// </remarks>
    private static void SweepPatchHelpers()
    {
        int swept = 0;
        int failed = 0;
        DateTime cutoff = DateTime.UtcNow.AddMinutes(-1);

        try
        {
            foreach (string path in Directory.EnumerateFiles(Path.GetTempPath(), "mm-exhelper.so.*").ToList())
            {
                try
                {
                    if (File.GetLastWriteTimeUtc(path) >= cutoff)
                    {
                        continue;
                    }

                    File.Delete(path);
                    swept++;
                }
                catch (Exception)
                {
                    // Another user's, or gone between the listing and the
                    // delete. Counted rather than logged per file: this is
                    // housekeeping, and one failure is not worth a line.
                    failed++;
                }
            }
        }
        catch (Exception e)
        {
            Log.Warn($"[determinism] could not sweep MonoMod helpers: {e.Message}");
            return;
        }

        if (swept > 0 || failed > 0)
        {
            Log.Info($"[determinism] swept {swept} leftover MonoMod helper(s), {failed} skipped");
        }
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
