using System.Collections.Generic;
using HarmonyLib;
using MegaCrit.Sts2.Core.Saves;
using MegaCrit.Sts2.Core.Logging;
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
/// D3 wants.
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

/// <summary>
/// Says something when a canonical monster's RNG is read.
/// </summary>
/// <remarks>
/// <c>MonsterModel.Rng</c> is:
///
/// <code>
/// get
/// {
///     if (!base.IsMutable) return Rng.Chaotic;
///     return _rng;
/// }
/// </code>
///
/// — so reading it on a canonical model silently yields the process-wide
/// clock-seeded stream. That is the same shape as the <c>DebugRandomizeRng</c>
/// bug: a substitution rather than an error, producing plausible values that
/// nothing in the run accounts for.
///
/// It does not currently bite. Monsters in a fight come from
/// <c>GenerateMonsters</c> via <c>ToMutable</c>, so they are mutable and use
/// their own seeded <c>_rng</c>. This exists because "does not currently bite"
/// is a claim with no evidence behind it, and a warning turns it into one.
///
/// A warning rather than a throw. Throwing would be the stronger signal and is
/// the wrong trade here: the game reads this property from presentation paths a
/// headless run does not exercise but may one day, and a determinism mod that
/// crashes the game to report a determinism risk has made things worse.
///
/// Once per monster id, because a getter is not somewhere to put an unbounded
/// log.
/// </remarks>
[HarmonyPatch(typeof(MonsterModel), nameof(MonsterModel.Rng), MethodType.Getter)]
internal static class NoticeCanonicalMonsterRng
{
    private static readonly HashSet<string> Reported = new HashSet<string>();

    private static void Postfix(MonsterModel __instance)
    {
        if (!__instance.IsCanonical)
        {
            return;
        }

        string id = __instance.Id.Entry;
        lock (Reported)
        {
            if (!Reported.Add(id))
            {
                return;
            }
        }

        Log.Warn(
            $"[determinism] canonical monster {id} had its Rng read; that returns "
            + "the process-wide clock-seeded Rng.Chaotic, so whatever used it is "
            + "not reproducible");
    }
}

/// <summary>A profile with no Wongo points, whatever the saved one holds.</summary>
/// <remarks>
/// <c>WelcomeToWongos.CheckObtainWongoBadge</c> reads
/// <c>SaveManager.Instance.Progress.WongoPoints</c>, which is the profile's and
/// not the run's. A purchase that takes the total past a multiple of two
/// thousand hands over a badge. The oracle's profile keeps what every run it
/// finishes earned, so the same seed would record a badge one day and none
/// the next.
///
/// So the profile reads as a fresh one, which is what the simulator models.
/// Besides the event, only the progress save reads the property.
/// </remarks>
[HarmonyPatch(typeof(ProgressState), nameof(ProgressState.WongoPoints), MethodType.Getter)]
internal static class FreshWongoPoints
{
    private static bool Prefix(ref int __result)
    {
        __result = 0;
        return false;
    }
}
