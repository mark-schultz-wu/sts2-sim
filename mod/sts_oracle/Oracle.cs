using System;
using MegaCrit.Sts2.Core.Debug;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Modding;
using MegaCrit.Sts2.Core.TestSupport;

namespace StsOracle;

/// <summary>
/// Entry point. The loader calls <see cref="Init"/> once, during game startup,
/// before a run exists.
/// </summary>
[ModInitializer(nameof(Init))]
public static class Oracle
{
    /// <summary>
    /// The build this oracle was built against and verified on: v0.107.1,
    /// buildid 23811903, from <c>release_info.json</c>.
    /// </summary>
    /// <remarks>
    /// Steam can update the install underneath us. An oracle that answers for a
    /// different build than the one we reason about is worse than no oracle, so
    /// a mismatch is refused loudly rather than warned about.
    /// </remarks>
    private const int PinnedAssemblyHash = 1692500715;

    private const string PinnedVersion = "v0.107.1";

    public static bool Active { get; private set; }

    public static void Init()
    {
        if (!VerifyPin())
        {
            return;
        }

        // Skips every Cmd.Wait and the animation paths that check it. This is
        // the game's own test seam, so it beats patching the waits ourselves.
        // Verified to gate presentation only: MapCmd applies its model change
        // unconditionally and skips just the icon refresh, and nothing calls
        // TestMode.AssertOff.
        TestMode.IsOn = true;

        Active = true;
        Log.Info($"[oracle] active on {PinnedVersion}; TestMode on");

        // Configured by environment rather than command line: Godot owns argv,
        // and unrecognised flags there are its business, not ours.
        string? mode = Environment.GetEnvironmentVariable("STS_ORACLE_MODE");
        if (mode == "drive")
        {
            string seed = Environment.GetEnvironmentVariable("STS_ORACLE_SEED") ?? "ORACLE1";
            // Not awaited: Init must return so startup can finish populating the
            // model database, which the driver's first stage waits for.
            _ = OracleDriver.Start(seed);
        }
        else if (mode != null)
        {
            Log.Warn($"[oracle] unknown STS_ORACLE_MODE '{mode}', doing nothing");
        }
    }

    private static bool VerifyPin()
    {
        ReleaseInfo? release = ReleaseInfoManager.Instance.ReleaseInfo;
        if (release == null)
        {
            Log.Error("[oracle] refusing to start: no release_info.json, so the build cannot be identified");
            return false;
        }

        if (release.MainAssemblyHash != PinnedAssemblyHash)
        {
            Log.Error(
                $"[oracle] refusing to start: assembly hash {release.MainAssemblyHash} "
                + $"is not the pinned {PinnedAssemblyHash} (game reports {release.Version}, "
                + $"commit {release.Commit}). The install changed; re-verify before trusting it.");
            return false;
        }

        return true;
    }
}
