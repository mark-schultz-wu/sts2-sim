using System;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Modding;

namespace Sts2Headless;

/// <summary>
/// Entry point. The loader calls <see cref="Init"/> once, during game startup,
/// before a run exists.
/// </summary>
/// <remarks>
/// Reproducibility is not this mod's job — <c>sts2_determinism</c> owns it, and
/// the manifest declares a dependency so the loader runs that one first. The
/// loader refuses to load this mod at all if that one is absent, naming it.
///
/// Testing is not this mod's job either. <c>sts2_headless_test</c> carries the
/// legality suite and claims <c>selftest</c> for itself, so the shipped driver
/// contains no tests and installing it does not install them.
/// </remarks>
[ModInitializer(nameof(Init))]
public static class Headless
{
    public static void Init()
    {
        // Configured by environment rather than command line: Godot owns argv,
        // and unrecognised flags there are its business, not ours.
        string? mode = Environment.GetEnvironmentVariable("STS2_HEADLESS_MODE");

        // Before the switch, so it covers the modes this mod does not run
        // itself: a selftest that hangs is still this process, and the test mods
        // load after this one. Not armed for a null mode, which is a player's
        // game and must keep Godot's own signal handling.
        if (mode is "serve" or "selftest" or "determinism")
        {
            Deadline.Arm();
        }

        switch (mode)
        {
            case "serve":
                // Not awaited: Init must return so startup can finish populating
                // the model database, which the driver's first stage waits for.
                // Driver.Run owns the rest of the lifecycle, including the
                // reproducibility check.
                _ = Driver.Run(Protocol.Serve);
                break;

            // Claimed by the test mods, which may or may not be installed.
            // Named here so a typo still gets a warning rather than silence.
            case "selftest":
            case "determinism":
            case null:
                break;

            default:
                Log.Warn($"[headless] unknown STS2_HEADLESS_MODE '{mode}', doing nothing");
                break;
        }
    }
}
