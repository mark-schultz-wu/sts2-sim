using HarmonyLib;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Saves.Managers;

namespace Sts2Determinism;

/// <summary>
/// Keeps an encounter's RNG seeded from the run rather than from the clock.
/// </summary>
/// <remarks>
/// <c>EncounterModel.DebugRandomizeRng</c> is
/// <c>_rng = new Rng((uint)(DateTime.UtcNow - DateTime.UnixEpoch).TotalSeconds)</c>
/// — wall clock, at one-second granularity. The damage is subtler than "adds
/// randomness": <c>GenerateMonstersWithSlots</c> seeds the encounter itself, but
/// only <c>if (_rng == null)</c>, from
/// <c>runState.Rng.Seed + runState.TotalFloor + hash(Id.Entry)</c>. Assigning
/// <c>_rng</c> early <em>pre-empts</em> that, so the wall-clock value is used
/// instead of the seed-derived one.
///
/// So this does not reimplement seeding. It gets out of the way and lets the
/// game do its own, by leaving <c>_rng</c> null.
///
/// <c>FightConsoleCmd</c> calls it unconditionally, which is why console-driven
/// fights were nondeterministic — reproducing for runs landing in the same
/// second and diverging for runs a second apart, which presents as flakiness
/// rather than as an obvious break.
/// </remarks>
[HarmonyPatch(typeof(EncounterModel), nameof(EncounterModel.DebugRandomizeRng))]
internal static class KeepEncounterRngSeeded
{
    /// <summary>Skips the original entirely.</summary>
    private static bool Prefix() => false;
}

/// <summary>
/// Reports every first-time-user tutorial as already seen.
/// </summary>
/// <remarks>
/// FTUEs both gate progress — a driver waits forever on a prompt nothing will
/// answer — and, once dismissed, write to the profile, so whether one has been
/// seen becomes state that persists across runs and that no seed accounts for.
///
/// <c>SeenFtue</c> rather than the <c>EnableFtues</c> property behind it, and
/// rather than calling <c>SetFtuesEnabled(false)</c> as the driver used to. Both
/// alternatives write: the setter calls <c>SaveProgress()</c>, and the property
/// is read by <c>ProgressState</c>'s own serialisation, so forcing it would
/// persist <c>EnableFtues = false</c> into the player's save. Patching the one
/// question the game actually asks changes no stored state at all, which is what
/// R13 wants.
/// </remarks>
[HarmonyPatch(typeof(ProgressSaveManager), nameof(ProgressSaveManager.SeenFtue))]
internal static class EveryTutorialAlreadySeen
{
    private static bool Prefix(ref bool __result)
    {
        __result = true;
        return false;
    }
}
