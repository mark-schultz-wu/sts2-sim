using System;
using System.Reflection;
using System.Threading.Tasks;
using HarmonyLib;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Models.Monsters;
using MegaCrit.Sts2.Core.Events.Custom.CrystalSphereEvent;
using MegaCrit.Sts2.Core.Nodes.Events.Custom.CrystalSphere;
using MegaCrit.Sts2.Core.Nodes.Rooms;
using MegaCrit.Sts2.Core.Nodes.Vfx.Backgrounds;
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

/// <summary>
/// Lets the Kaiser Crab boss be fought without a scene to draw it in.
/// </summary>
/// <remarks>
/// <c>Crusher</c> and <c>Rocket</c> — the two arms of `KAISER_CRAB_BOSS` — reach
/// into their own scene background from model code:
///
/// <code>
/// private NKaiserCrabBossBackground Background =>
///     _background ??= ((Control)(NCombatRoom.Instance?.Background
///         ?? NBestiary.Instance?.Layout ?? throw new InvalidOperationException()))
///             .GetNode&lt;NKaiserCrabBossBackground&gt;("%KaiserCrab");
/// </code>
///
/// <c>NCombatRoom.Instance</c> is <c>NRun.Instance?.CombatRoom</c> and no headless
/// run has an <c>NRun</c>, so the getter always throws. It is called from
/// <c>AfterAddedToRoom</c>, so <c>CombatManager.StartCombatInternal</c> aborts
/// part-way: measured, the fight begins with both creatures in the room, the
/// Crusher holding the two powers applied before the throw, the Rocket holding
/// none of its three, no hand drawn and both monsters on <c>UNSET_MOVE</c>. The
/// exception is swallowed by <c>TaskHelper.LogTaskExceptions</c>, so the run
/// carries on into a fight that never started.
///
/// **This is an unguarded case, not an impossible one.** Every other monster that
/// touches the scene in <c>AfterAddedToRoom</c> — <c>Guardbot</c>,
/// <c>Noisebot</c>, <c>Stabbot</c>, <c>Zapbot</c>, <c>ToughEgg</c> — wraps it in
/// <c>if (TestMode.IsOff)</c>. These two do not, and they are the only two.
///
/// So the background is supplied rather than the callers rewritten: one detached
/// node, with all seven of its methods emptied. That is sound because all seven
/// are presentation and nothing else — they set spine animation states and
/// <c>await Cmd.Wait(duration)</c>, and <c>Cmd.Wait</c> already returns
/// immediately under <c>TestMode</c>, which every headless run is in. No damage,
/// no power, no RNG draw and no timing the model can observe passes through any
/// of them. <c>SetVisible</c> is Godot's own and works on a detached
/// <c>Node2D</c>.
///
/// The alternative was prefixing all ten of the two monsters' moves to re-issue
/// their damage without the animation, which would mean maintaining a second
/// copy of the boss.
/// </remarks>
internal static class KaiserCrabNeedsNoScene
{
    /// <summary>
    /// One node for the process. It holds no state that a run could dirty —
    /// every method that would write to it is patched away — and building it per
    /// call would leak a Godot object per attack.
    /// </summary>
    private static NKaiserCrabBossBackground? _stand;

    internal static NKaiserCrabBossBackground Stand => _stand ??= new NKaiserCrabBossBackground();
}

/// <inheritdoc cref="KaiserCrabNeedsNoScene"/>
[HarmonyPatch(typeof(Crusher), "Background", MethodType.Getter)]
internal static class CrusherBackground
{
    private static bool Prefix(ref NKaiserCrabBossBackground __result)
    {
        if (NCombatRoom.Instance != null)
        {
            return true;
        }
        __result = KaiserCrabNeedsNoScene.Stand;
        return false;
    }
}

/// <summary>
/// Lets the Soul Nexus die without a scene to stop animating it in.
/// </summary>
/// <remarks>
/// <c>SoulNexus.AfterDeath</c> is subscribed to <c>Creature.Died</c> and reads
/// <c>NCombatRoom.Instance.GetCreatureNode(...)</c>. The <c>?.</c> is on the
/// result, not on <c>Instance</c> — and <c>BeforeRemovedFromRoom</c>, eleven
/// lines further down the same file, writes <c>NCombatRoom.Instance?.</c> for
/// the same call. So this is a missing character rather than a case nobody
/// considered, and headless is where it lands: there is no combat room, and
/// killing the monster dereferences null.
///
/// It does not surface as a <c>NullReferenceException</c>. <c>Instance</c> is a
/// Godot object and the interop stub reads through the native pointer, so the
/// process takes SIGSEGV at address 0 and goes away without printing anything.
/// The fuzzer found it as "the game closed the connection" on move 1058, with
/// the Soul Nexus on 1 HP.
///
/// The prefix keeps the unsubscribe and drops only the animation. Skipping the
/// whole body would leave the handler attached, which is a second bug waiting
/// on whatever makes a creature die twice; and the line being dropped sets a
/// spine animation and nothing else — no damage, no power, no RNG draw.
/// </remarks>
[HarmonyPatch(typeof(SoulNexus), "AfterDeath")]
internal static class SoulNexusDiesWithoutAScene
{
    private static bool Prefix(SoulNexus __instance)
    {
        if (NCombatRoom.Instance != null)
        {
            return true;
        }

        // What the body would have done first. A delegate over the same target
        // and method compares equal to the one that was added, which is what
        // `-=` removes by.
        MethodInfo? handler = typeof(SoulNexus).GetMethod(
            "AfterDeath",
            BindingFlags.Instance | BindingFlags.NonPublic);
        if (handler != null)
        {
            __instance.Creature.Died -=
                (Action<Creature>)Delegate.CreateDelegate(typeof(Action<Creature>), __instance, handler);
        }

        return false;
    }
}

/// <inheritdoc cref="KaiserCrabNeedsNoScene"/>
[HarmonyPatch(typeof(Rocket), "Background", MethodType.Getter)]
internal static class RocketBackground
{
    private static bool Prefix(ref NKaiserCrabBossBackground __result)
    {
        if (NCombatRoom.Instance != null)
        {
            return true;
        }
        __result = KaiserCrabNeedsNoScene.Stand;
        return false;
    }
}

/// <inheritdoc cref="KaiserCrabNeedsNoScene"/>
[HarmonyPatch(typeof(NKaiserCrabBossBackground))]
internal static class SilenceKaiserCrabAnimation
{
    [HarmonyPrefix]
    [HarmonyPatch(nameof(NKaiserCrabBossBackground.PlayAttackAnim))]
    private static bool Attack(ref Task __result) => Done(ref __result);

    [HarmonyPrefix]
    [HarmonyPatch(nameof(NKaiserCrabBossBackground.PlayRightSideChargeUpAnim))]
    private static bool ChargeUp(ref Task __result) => Done(ref __result);

    [HarmonyPrefix]
    [HarmonyPatch(nameof(NKaiserCrabBossBackground.PlayRightSideHeavy))]
    private static bool Heavy(ref Task __result) => Done(ref __result);

    [HarmonyPrefix]
    [HarmonyPatch(nameof(NKaiserCrabBossBackground.PlayRightRecharge))]
    private static bool Recharge(ref Task __result) => Done(ref __result);

    [HarmonyPrefix]
    [HarmonyPatch(nameof(NKaiserCrabBossBackground.PlayHurtAnim))]
    private static bool Hurt() => Skip();

    [HarmonyPrefix]
    [HarmonyPatch(nameof(NKaiserCrabBossBackground.PlayArmDeathAnim))]
    private static bool ArmDeath() => Skip();

    [HarmonyPrefix]
    [HarmonyPatch(nameof(NKaiserCrabBossBackground.PlayBodyDeathAnim))]
    private static bool BodyDeath() => Skip();

    /// <summary>Only stand in when there is no scene; a real one still animates.</summary>
    private static bool Skip() => NCombatRoom.Instance != null;

    private static bool Done(ref Task __result)
    {
        if (NCombatRoom.Instance != null)
        {
            return true;
        }
        __result = Task.CompletedTask;
        return false;
    }
}

/// <summary>
/// Divines through the protocol instead of through a screen.
/// </summary>
/// <remarks>
/// <c>ShowScreen</c> instantiates
/// <c>res://scenes/events/custom/crystal_sphere/crystal_sphere_screen.tscn</c>,
/// and headless that scene is not in the preload cache. The process dies where
/// it asks for it — no exception, no stack, the log simply stops after
/// <c>Asset not cached</c>. Reaching the Crystal Sphere and choosing either of
/// its two options was a hard end to the run.
///
/// The scene cannot be made to load: it is a <c>Control</c> pushed onto
/// <c>NOverlayStack.Instance</c>, which is <c>NRun.Instance?.GlobalUi.Overlays</c>
/// — nothing a headless run has. Nor can the minigame be skipped, because both
/// options await it and its result is what pays out.
///
/// So the click arrives over the wire. <see cref="Divination"/> parks the
/// question; the return value is discarded by the only caller
/// (<c>CrystalSphereMinigame.PlayMinigame</c>), which awaits the minigame's own
/// completion source rather than the screen.
/// </remarks>
[HarmonyPatch(typeof(NCrystalSphereScreen), nameof(NCrystalSphereScreen.ShowScreen))]
internal static class DivineWithoutAScreen
{
    private static bool Prefix(CrystalSphereMinigame grid, ref NCrystalSphereScreen? __result)
    {
        Divination.Ask(grid);
        __result = null;
        return false;
    }
}
