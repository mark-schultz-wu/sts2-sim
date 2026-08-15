using HarmonyLib;
using MegaCrit.Sts2.Core.Nodes.Vfx.Utilities;

namespace Sts2Headless;

/// <summary>
/// Stops a headless run logging an error every time something gets hit.
/// </summary>
/// <remarks>
/// <c>NScreenShake.Shake</c> and <c>Rumble</c> are:
///
/// <code>
/// if (ShakeTarget == null) Log.Error("Missing screenShake target!");
/// else _shakeInstance = new ScreenPunchInstance(...);
/// </code>
///
/// The else-branch builds a value whose only effect is
/// <c>ShakeTarget.Position = ...</c> in <c>_Process</c>. With no target there is
/// nothing to move, so the method already did nothing — this removes the log and
/// not the work. Skipping only when the target is null leaves a real shake
/// alone, in case anything ever instantiates a screen.
///
/// It matters because the model layer calls presentation inline:
/// <c>CreatureCmd</c> calls <c>NGame.Instance?.ScreenShake(...)</c> when damage
/// lands, so an ordinary card play produced an error-level line. Across the
/// captured runs of this session that message accounts for **352** of the
/// game's error output and every other error line is either our own test output
/// or a real failure.
///
/// That was the problem worth fixing. The README tells a reader not to trust an
/// observation carrying <c>gameErrors</c> — and the field fired on every card
/// played, so the rule was untrustworthy itself. A warning that fires every time
/// is one people learn to skip, which is exactly how the half-completed action
/// <c>gameErrors</c> exists to catch would get through.
///
/// This suppresses a known-benign message rather than the mechanism: anything
/// else the game logs at error level still reaches <c>gameErrors</c> untouched.
/// </remarks>
[HarmonyPatch(typeof(NScreenShake), nameof(NScreenShake.Shake))]
internal static class SilenceTargetlessShake
{
    private static bool Prefix(NScreenShake __instance) => __instance.ShakeTarget != null;
}

/// <inheritdoc cref="SilenceTargetlessShake"/>
[HarmonyPatch(typeof(NScreenShake), nameof(NScreenShake.Rumble))]
internal static class SilenceTargetlessRumble
{
    private static bool Prefix(NScreenShake __instance) => __instance.ShakeTarget != null;
}
