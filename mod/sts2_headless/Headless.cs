using System;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Modding;
using Sts2Determinism;

namespace Sts2Headless;

/// <summary>
/// Entry point. The loader calls <see cref="Init"/> once, during game startup,
/// before a run exists.
/// </summary>
/// <remarks>
/// Reproducibility is not this mod's job — <c>sts2_determinism</c> owns it, and
/// the manifest declares a dependency so the loader runs that one first. The
/// loader refuses to load this mod at all if that one is absent, naming it.
/// </remarks>
[ModInitializer(nameof(Init))]
public static class Headless
{
    public static bool Active { get; private set; }

    public static void Init()
    {
        // Configured by environment rather than command line: Godot owns argv,
        // and unrecognised flags there are its business, not ours.
        string? mode = Environment.GetEnvironmentVariable("STS2_HEADLESS_MODE");
        if (mode is not ("serve" or "selftest"))
        {
            if (mode != null)
            {
                Log.Warn($"[headless] unknown STS2_HEADLESS_MODE '{mode}', doing nothing");
            }

            // Installed but not asked to do anything. Say nothing and touch
            // nothing — including when the determinism mod is inert, since
            // nobody is relying on it either.
            return;
        }

        // Loading first is not the same as having succeeded: a build mismatch
        // leaves the determinism mod loaded, inert, and the game untouched.
        // Driving a game nobody made reproducible is worse than not driving it.
        if (!Determinism.Active)
        {
            // Exit rather than return. Nothing else is going to drive this
            // process, and nothing else is going to stop it either — a headless
            // game with no driver runs until something external kills it, which
            // is how this refusal first presented: as a hang, with the reason
            // sitting in a log nobody was reading.
            Log.Error(
                "[headless] refusing to start: sts2_determinism loaded but did not activate, "
                + "so the game has not been made reproducible. Exiting.");
            Environment.Exit(1);
            return;
        }

        Active = true;
        Log.Info("[headless] active");

        // Not awaited: Init must return so startup can finish populating the
        // model database, which the driver's first stage waits for.
        _ = Driver.Start();
    }
}
