using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Reflection.Emit;
using System.Threading.Tasks;
using HarmonyLib;
using MegaCrit.Sts2.Core.CardSelection;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Context;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Multiplayer;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.GameActions;
using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer.Game;
using MegaCrit.Sts2.Core.Models.Events;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Models.Monsters;
using MegaCrit.Sts2.Core.Models.Relics;
using MegaCrit.Sts2.Core.Entities.Relics;
using MegaCrit.Sts2.Core.Entities.RestSite;
using MegaCrit.Sts2.Core.Entities.Merchant;
using MegaCrit.Sts2.Core.Rewards;
using MegaCrit.Sts2.Core.Saves;
using MegaCrit.Sts2.Core.Saves.Managers;
using MegaCrit.Sts2.Core.Rooms;
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

/// <summary>
/// Lets Trial pick its defendant without an event room to draw the portrait in.
/// </summary>
/// <remarks>
/// <c>Trial.Accept</c> and <c>AddVfxAnchoredToPortrait</c> guard their portrait
/// work with <c>LocalContext.IsMe(Owner)</c> alone, then write
/// <c>NEventRoom.Instance.Layout</c> and <c>NEventRoom.Instance.SetPortrait</c>.
/// Every other event that touches the room writes <c>NEventRoom.Instance?.</c>.
/// Headless there is no room, and the dereference takes the process down the
/// way <see cref="SoulNexusDiesWithoutAScene"/> describes: "the game closed the
/// connection" as the first Accept is taken.
///
/// The transpiler swaps each <c>IsMe</c> in those two methods for
/// <see cref="IsMeWithARoom"/>. Everything else in <c>Accept</c> runs as
/// written: the <c>Rng.NextInt(3)</c> for the defendant, and the page it sets.
/// </remarks>
[HarmonyPatch(typeof(Trial))]
internal static class TrialWithoutARoom
{
    private static readonly MethodInfo IsMe =
        AccessTools.Method(typeof(LocalContext), nameof(LocalContext.IsMe), new[] { typeof(Player) });

    private static IEnumerable<MethodBase> TargetMethods()
    {
        yield return AccessTools.Method(typeof(Trial), "Accept");
        yield return AccessTools.Method(typeof(Trial), "AddVfxAnchoredToPortrait");
    }

    private static IEnumerable<CodeInstruction> Transpiler(IEnumerable<CodeInstruction> instructions)
    {
        int swapped = 0;
        foreach (CodeInstruction instruction in instructions)
        {
            if (instruction.Calls(IsMe))
            {
                swapped++;
                yield return new CodeInstruction(
                    OpCodes.Call,
                    AccessTools.Method(typeof(TrialWithoutARoom), nameof(IsMeWithARoom)));
            }
            else
            {
                yield return instruction;
            }
        }

        if (swapped == 0)
        {
            throw new InvalidOperationException("Trial no longer calls LocalContext.IsMe(Player)");
        }
    }

    private static bool IsMeWithARoom(Player? player) =>
        LocalContext.IsMe(player) && NEventRoom.Instance != null;
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
/// <summary>
/// <c>CardSelectCmd.FromChooseABundleScreen</c> with a question where its
/// screen would be.
/// </summary>
/// <remarks>
/// The game's own early returns, choice id, sync and log, around a
/// <see cref="BundleChoice"/>. Left alone, <c>TestMode</c> takes the first
/// bundle without asking, which a player at the screen never does.
/// </remarks>
[HarmonyPatch(typeof(CardSelectCmd), nameof(CardSelectCmd.FromChooseABundleScreen))]
internal static class BundleWithoutAScreen
{
    private static bool Prefix(
        Player player,
        IReadOnlyList<IReadOnlyList<CardModel>> bundles,
        ref Task<IEnumerable<CardModel>> __result)
    {
        // The screen asks the local player alone. Another seat's bundle waits
        // for that seat's choice, as the game's own code does.
        if (!LocalContext.IsMe(player))
        {
            return true;
        }
        __result = Asked(player, bundles);
        return false;
    }

    private static async Task<IEnumerable<CardModel>> Asked(
        Player player, IReadOnlyList<IReadOnlyList<CardModel>> bundles)
    {
        if (CombatManager.Instance.IsEnding || bundles.Count == 0)
        {
            return Array.Empty<CardModel>();
        }
        uint choiceId = RunManager.Instance.PlayerChoiceSynchronizer.ReserveChoiceId(player);
        int index = await BundleChoice.Ask(bundles);
        IReadOnlyList<CardModel> chosen = bundles[index];
        RunManager.Instance.PlayerChoiceSynchronizer.SyncLocalChoice(
            player, choiceId, PlayerChoiceResult.FromIndex(index));
        Log.Info($"Player {player.NetId} chose cards [{string.Join(",", chosen.Select(c => c.Id.Entry))}]");
        return chosen;
    }
}

/// <summary>
/// Whether an option at this rest site went through, which is when
/// <c>NRestSiteRoom</c> puts its proceed button up.
/// </summary>
/// <remarks>
/// The room hears it from <c>NRestSiteButton.SelectOption</c>, which calls
/// <c>AfterSelectingOption</c> on success alone. The synchronizer's
/// <c>AfterPlayerOptionChosen</c> carries the same success, so this listens
/// there. <c>BeginRestSite</c> opens each rest site and resets it.
/// </remarks>
[HarmonyPatch(typeof(RestSiteSynchronizer), nameof(RestSiteSynchronizer.BeginRestSite))]
internal static class RestSiteProgress
{
    public static bool Rested { get; private set; }

    private static void Postfix(RestSiteSynchronizer __instance)
    {
        Rested = false;
        __instance.AfterPlayerOptionChosen -= Chosen;
        __instance.AfterPlayerOptionChosen += Chosen;
    }

    private static void Chosen(RestSiteOption option, bool success, ulong playerId)
    {
        Rested |= success;
    }
}

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

/// <summary>
/// Asks the combat selections the way the game's own screen does, so a
/// question paused in a hook lets the game go on around it.
/// </summary>
/// <remarks>
/// <c>CardSelectCmd</c> has two ways to ask. With a selector installed it
/// awaits the selector and nothing else. With none it reserves a choice id,
/// calls <c>SignalPlayerChoiceBegun</c>, shows its screen, syncs the choice and
/// calls <c>SignalPlayerChoiceEnded</c>. The signal is not bookkeeping. In a
/// hook's context it turns the rest of the hook into an action that waits for
/// the play phase, so <c>StartTurn</c> goes on to <c>AfterSideTurnStart</c> and
/// the orbs before the player is asked. The selector branch skips it, and the
/// turn start then waits for the answer, which a player at the screen never
/// sees.
///
/// So these take the screen's branch, with our parked question where the
/// screen would be. Everything before it is the game's own code: the early
/// returns, the filter, and the sort a draw pile gets.
///
/// The branch also sets what may be skipped. The selector is handed a
/// minimum of none for <c>FromChooseACardScreen</c>, whatever its
/// <c>canSkip</c> says, and <c>NChooseACardSelectionScreen</c> hides its skip
/// button unless <c>canSkip</c> is set.
/// </remarks>
/// <summary>
/// <c>FromChooseACardScreen</c>'s skip, for the builds whose selector branch
/// <see cref="ScreenPathSelections"/> leaves alone.
/// </summary>
/// <remarks>
/// That branch hands the selector a minimum of none whatever
/// <c>canSkip</c> says, and a player at the screen sees no skip button
/// unless it is set. So the selector is told what the screen would offer.
///
/// The screen's branch awaits <c>SignalPlayerChoiceBegun</c> before it asks
/// the selector, by which time the method has returned its task to the
/// caller. The prefix sets an <see cref="AsyncLocal{T}"/> as the method
/// starts, which flows into the method's continuation, and the postfix
/// clears it for the caller alone.
/// </remarks>
[HarmonyPatch(typeof(CardSelectCmd), nameof(CardSelectCmd.FromChooseACardScreen))]
internal static class ChooseACardSkip
{
    private static readonly System.Threading.AsyncLocal<bool?> Asked = new();

    /// <summary>The <c>canSkip</c> of the screen being asked, if one is.</summary>
    internal static bool? CanSkip
    {
        get => Asked.Value;
        set => Asked.Value = value;
    }

    [HarmonyPrepare]
    private static bool Prepare() => !GameApi.AsksThroughTheScreen;

    [HarmonyPrefix]
    private static void Prefix(bool canSkip) => CanSkip = canSkip;

    [HarmonyPostfix]
    private static void Postfix() => CanSkip = null;
}

/// <summary>
/// Whether the card selection being asked can be backed out of, which only
/// its prefs say.
/// </summary>
/// <remarks>
/// The selector is handed the cards and the bounds, never the prefs, and
/// <c>CardSelectorPrefs.Cancelable</c> is what puts a back button on Smith,
/// Cook, the shop's card removal and the card-removal reward. So every
/// <c>CardSelectCmd</c> method that takes prefs notes it as it starts, the
/// way <see cref="ChooseACardSkip"/> notes a skip, and clears it for the
/// caller as it returns.
/// </remarks>
[HarmonyPatch]
internal static class CancelableSelection
{
    private static readonly System.Threading.AsyncLocal<bool?> Asked = new();

    /// <summary>The <c>Cancelable</c> of the selection being asked, if one is.</summary>
    internal static bool? Cancelable
    {
        get => Asked.Value;
        set => Asked.Value = value;
    }

    [HarmonyTargetMethods]
    private static IEnumerable<System.Reflection.MethodBase> TargetMethods() =>
        typeof(CardSelectCmd)
            .GetMethods(System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.Static)
            .Where(m => !m.IsGenericMethodDefinition
                && m.GetParameters().Any(p => p.ParameterType == typeof(CardSelectorPrefs)));

    [HarmonyPrefix]
    private static void Prefix(object[] __args)
    {
        foreach (object? arg in __args)
        {
            if (arg is CardSelectorPrefs prefs)
            {
                Cancelable = prefs.Cancelable;
            }
        }
    }

    [HarmonyPostfix]
    private static void Postfix() => Cancelable = null;
}

/// <summary>
/// <c>CardReward.OnSelect</c> reads <c>LocalSelector</c> where it reads
/// <c>Selector</c>, for the builds that install the selector there.
/// </summary>
/// <remarks>
/// With no screen shown, a card reward asks <c>CardSelectCmd.Selector</c>
/// which card to take, and throws when none is set. The rest of
/// <c>CardSelectCmd</c> asks <c>LocalSelector</c> on the screen's branch,
/// which is where <see cref="GameApi.SelectsLocally"/> puts ours.
/// </remarks>
[HarmonyPatch]
internal static class RewardReadsLocalSelector
{
    [HarmonyPrepare]
    private static bool Prepare() => GameApi.SelectsLocally;

    [HarmonyTargetMethod]
    private static System.Reflection.MethodBase TargetMethod()
        => AccessTools.AsyncMoveNext(AccessTools.Method(typeof(CardReward), "OnSelect"));

    [HarmonyTranspiler]
    private static IEnumerable<CodeInstruction> Transpiler(IEnumerable<CodeInstruction> instructions)
    {
        var selector = AccessTools.PropertyGetter(typeof(CardSelectCmd), nameof(CardSelectCmd.Selector));
        var local = AccessTools.PropertyGetter(typeof(CardSelectCmd), "LocalSelector")
            ?? throw new InvalidOperationException("CardSelectCmd has no LocalSelector");
        var swapped = 0;
        foreach (var instruction in instructions)
        {
            if (instruction.Calls(selector))
            {
                instruction.operand = local;
                swapped++;
            }
            yield return instruction;
        }
        if (swapped == 0)
        {
            throw new InvalidOperationException("CardReward.OnSelect reads no CardSelectCmd.Selector");
        }
    }
}

[HarmonyPatch(typeof(CardSelectCmd))]
internal static class ScreenPathSelections
{
    [HarmonyPrepare]
    private static bool Prepare() => GameApi.AsksThroughTheScreen;

    /// <summary>
    /// Whether a selector other than ours stands on top, which the game's own
    /// body asks and nothing else. <c>WhisperingEarring</c> pushes a
    /// <c>VakuuCardSelector</c> while it plays the hand, and the player sees
    /// none of what it answers.
    /// </summary>
    private static bool SelectorPushed => CardSelectCmd.Selector is not HeadlessCardSelector;

    [HarmonyPrefix]
    [HarmonyPatch(nameof(CardSelectCmd.FromHand))]
    private static bool FromHand(
        PlayerChoiceContext context,
        Player player,
        CardSelectorPrefs prefs,
        Func<CardModel, bool>? filter,
        ref Task<IEnumerable<CardModel>> __result)
    {
        if (SelectorPushed)
        {
            return true;
        }
        __result = FromHandAsked(context, player, prefs, filter);
        return false;
    }

    [HarmonyPrefix]
    [HarmonyPatch(nameof(CardSelectCmd.FromCombatPile),
        typeof(PlayerChoiceContext), typeof(CardPile), typeof(Player), typeof(CardSelectorPrefs),
        typeof(Func<CardModel, bool>))]
    private static bool FromCombatPile(
        PlayerChoiceContext context,
        CardPile pile,
        Player player,
        CardSelectorPrefs prefs,
        Func<CardModel, bool> filter,
        ref Task<IEnumerable<CardModel>> __result)
    {
        if (SelectorPushed)
        {
            return true;
        }
        __result = FromCombatPileAsked(context, pile, player, prefs, filter);
        return false;
    }

    [HarmonyPrefix]
    [HarmonyPatch(nameof(CardSelectCmd.FromChooseACardScreen))]
    private static bool FromChooseACardScreen(
        PlayerChoiceContext context,
        IReadOnlyList<CardModel> cards,
        Player player,
        bool canSkip,
        ref Task<CardModel?> __result)
    {
        if (SelectorPushed)
        {
            return true;
        }
        __result = FromChooseACardScreenAsked(context, cards, player, canSkip);
        return false;
    }

    [HarmonyPrefix]
    [HarmonyPatch(nameof(CardSelectCmd.FromSimpleGridForRewards))]
    private static bool FromSimpleGridForRewards(
        PlayerChoiceContext context,
        List<CardCreationResult> cards,
        Player player,
        CardSelectorPrefs prefs,
        ref Task<IEnumerable<CardModel>> __result)
    {
        if (SelectorPushed)
        {
            return true;
        }
        __result = FromSimpleGridForRewardsAsked(context, cards, player, prefs);
        return false;
    }

    [HarmonyPrefix]
    [HarmonyPatch(nameof(CardSelectCmd.FromSimpleGrid))]
    private static bool FromSimpleGrid(
        PlayerChoiceContext context,
        IReadOnlyList<CardModel> cardsIn,
        Player player,
        CardSelectorPrefs prefs,
        ref Task<IEnumerable<CardModel>> __result)
    {
        if (SelectorPushed)
        {
            return true;
        }
        __result = FromSimpleGridAsked(context, cardsIn, player, prefs);
        return false;
    }

    [HarmonyPrefix]
    [HarmonyPatch(nameof(CardSelectCmd.FromHandForUpgrade))]
    private static bool FromHandForUpgrade(
        PlayerChoiceContext context,
        Player player,
        ref Task<CardModel?> __result)
    {
        if (SelectorPushed)
        {
            return true;
        }
        __result = FromHandForUpgradeAsked(context, player);
        return false;
    }

    private static async Task<CardModel?> FromChooseACardScreenAsked(
        PlayerChoiceContext context, IReadOnlyList<CardModel> cards, Player player, bool canSkip)
    {
        if (cards.Count > 3)
        {
            throw new ArgumentException("Only works with less than 3 cards", nameof(cards));
        }
        if (cards.Count == 0)
        {
            ReportSoftlock();
            return null;
        }
        uint choiceId = RunManager.Instance.PlayerChoiceSynchronizer.ReserveChoiceId(player);
        await GameApi.SignalChoiceBegun(context, player, PlayerChoiceOptions.None);
        CardModel? result = (await CardSelection.Ask(cards, canSkip ? 0 : 1, 1,
            RunManager.Instance.ActionExecutor.CurrentlyRunningAction?.ToString())).FirstOrDefault();
        RunManager.Instance.PlayerChoiceSynchronizer.SyncLocalChoice(
            player, choiceId, PlayerChoiceResult.FromIndex(result == null ? -1 : cards.ToList().IndexOf(result)));
        await context.SignalPlayerChoiceEnded();
        LogChoice(player, new[] { result });
        return result;
    }

    private static async Task<IEnumerable<CardModel>> FromSimpleGridForRewardsAsked(
        PlayerChoiceContext context, List<CardCreationResult> cards, Player player, CardSelectorPrefs prefs)
    {
        if (CombatManager.Instance.IsEnding)
        {
            return Array.Empty<CardModel>();
        }
        if (cards.Count == 0)
        {
            ReportSoftlock();
            return Array.Empty<CardModel>();
        }
        List<CardModel> result;
        if (!prefs.RequireManualConfirmation && cards.Count <= prefs.MinSelect)
        {
            result = cards.Select(c => c.Card).ToList();
        }
        else
        {
            uint choiceId = RunManager.Instance.PlayerChoiceSynchronizer.ReserveChoiceId(player);
            await GameApi.SignalChoiceBegun(context, player, PlayerChoiceOptions.None);
            result = (await CardSelection.Ask(cards.Select(c => c.Card).ToList(), prefs.MinSelect, prefs.MaxSelect,
                RunManager.Instance.ActionExecutor.CurrentlyRunningAction?.ToString())).ToList();
            List<int> indexes = result.Select(c => cards.FindIndex(r => r.Card == c)).ToList();
            RunManager.Instance.PlayerChoiceSynchronizer.SyncLocalChoice(
                player, choiceId, PlayerChoiceResult.FromIndexes(indexes));
            await context.SignalPlayerChoiceEnded();
        }
        LogChoice(player, result);
        return result;
    }

    private static async Task<IEnumerable<CardModel>> FromSimpleGridAsked(
        PlayerChoiceContext context, IReadOnlyList<CardModel> cardsIn, Player player, CardSelectorPrefs prefs)
    {
        if (CombatManager.Instance.IsEnding)
        {
            return Array.Empty<CardModel>();
        }
        List<CardModel> cards = cardsIn.ToList();
        if (cards.Count == 0)
        {
            return Array.Empty<CardModel>();
        }
        List<CardModel> result;
        if (!prefs.RequireManualConfirmation && cards.Count <= prefs.MinSelect)
        {
            result = cards.ToList();
        }
        else
        {
            uint choiceId = RunManager.Instance.PlayerChoiceSynchronizer.ReserveChoiceId(player);
            await GameApi.SignalChoiceBegun(context, player, PlayerChoiceOptions.None);
            result = (await CardSelection.Ask(cards, prefs.MinSelect, prefs.MaxSelect,
                RunManager.Instance.ActionExecutor.CurrentlyRunningAction?.ToString())).ToList();
            List<int> indexes = result.Select(c => cards.IndexOf(c)).ToList();
            RunManager.Instance.PlayerChoiceSynchronizer.SyncLocalChoice(
                player, choiceId, PlayerChoiceResult.FromIndexes(indexes));
            await context.SignalPlayerChoiceEnded();
        }
        LogChoice(player, result);
        return result;
    }

    private static async Task<CardModel?> FromHandForUpgradeAsked(PlayerChoiceContext context, Player player)
    {
        if (CombatManager.Instance.IsOverOrEnding)
        {
            return null;
        }
        List<CardModel> list = PileType.Hand.GetPile(player).Cards.Where(c => c.IsUpgradable).ToList();
        CardModel? result;
        if (list.Count <= 1)
        {
            result = list.FirstOrDefault();
        }
        else
        {
            uint choiceId = RunManager.Instance.PlayerChoiceSynchronizer.ReserveChoiceId(player);
            await GameApi.SignalChoiceBegun(context, player, PlayerChoiceOptions.CancelPlayCardActions);
            result = (await CardSelection.Ask(list, 1, 1,
                RunManager.Instance.ActionExecutor.CurrentlyRunningAction?.ToString())).FirstOrDefault();
            RunManager.Instance.PlayerChoiceSynchronizer.SyncLocalChoice(
                player, choiceId, PlayerChoiceResult.FromMutableCombatCard(result));
            await context.SignalPlayerChoiceEnded();
        }
        LogChoice(player, new[] { result });
        return result;
    }

    private static async Task<IEnumerable<CardModel>> FromHandAsked(
        PlayerChoiceContext context, Player player, CardSelectorPrefs prefs, Func<CardModel, bool>? filter)
    {
        if (CombatManager.Instance.IsOverOrEnding)
        {
            return Array.Empty<CardModel>();
        }
        List<CardModel> list = PileType.Hand.GetPile(player).Cards.Where(filter ?? (_ => true)).ToList();
        IEnumerable<CardModel> result;
        if (list.Count == 0 || (!prefs.RequireManualConfirmation && list.Count <= prefs.MinSelect))
        {
            result = list;
        }
        else
        {
            result = await AskAsTheScreenDoes(
                context, player, PlayerChoiceOptions.CancelPlayCardActions, list, prefs);
        }
        LogChoice(player, result);
        return result;
    }

    private static async Task<IEnumerable<CardModel>> FromCombatPileAsked(
        PlayerChoiceContext context, CardPile pile, Player player, CardSelectorPrefs prefs,
        Func<CardModel, bool> filter)
    {
        if (CombatManager.Instance.IsEnding)
        {
            return Array.Empty<CardModel>();
        }
        if (!pile.IsCombatPile)
        {
            throw new InvalidOperationException("Cannot perform on a non combat pile");
        }
        List<CardModel> list = pile.Cards.Where(filter).ToList();
        if (list.Count == 0)
        {
            return Array.Empty<CardModel>();
        }
        IEnumerable<CardModel> result;
        if (!prefs.RequireManualConfirmation && list.Count <= prefs.MinSelect)
        {
            result = list;
        }
        else
        {
            // The order the selector branch hands a draw pile over in, which
            // keeps the pile's own order hidden.
            if (pile.Type == PileType.Draw)
            {
                list = list.OrderBy(c => c.Rarity).ThenBy(c => c.Id).ToList();
            }
            result = await AskAsTheScreenDoes(context, player, PlayerChoiceOptions.None, list, prefs);
        }
        LogChoice(player, result);
        return result;
    }

    /// <summary>The screen branch, with the question parked where the screen would be.</summary>
    private static async Task<IEnumerable<CardModel>> AskAsTheScreenDoes(
        PlayerChoiceContext context, Player player, PlayerChoiceOptions options,
        List<CardModel> offered, CardSelectorPrefs prefs)
    {
        uint choiceId = RunManager.Instance.PlayerChoiceSynchronizer.ReserveChoiceId(player);
        await GameApi.SignalChoiceBegun(context, player, options);
        List<CardModel> result = (await CardSelection.Ask(offered, prefs.MinSelect, prefs.MaxSelect,
            RunManager.Instance.ActionExecutor.CurrentlyRunningAction?.ToString())).ToList();
        RunManager.Instance.PlayerChoiceSynchronizer.SyncLocalChoice(
            player, choiceId, PlayerChoiceResult.FromMutableCombatCards(result));
        await context.SignalPlayerChoiceEnded();
        return result;
    }

    /// <summary><c>CardSelectCmd.ReportSoftlock</c>, which is private.</summary>
    private static void ReportSoftlock() =>
        AccessTools.Method(typeof(CardSelectCmd), "ReportSoftlock").Invoke(null, null);

    /// <summary><c>CardSelectCmd.LogChoice</c>, which is private.</summary>
    private static void LogChoice(Player player, IEnumerable<CardModel?> cards)
    {
        string value = string.Join(",", cards.OfType<CardModel>().Select(c => c.Id.Entry));
        Log.Info($"Player {player.NetId} chose cards [{value}]");
    }
}

/// <summary>
/// Gives Calling Bell and Cauldron the rewards a real game gives them.
/// </summary>
/// <remarks>
/// Each builds its reward list behind <c>if (TestMode.IsOn)</c>, and every
/// headless run is in <c>TestMode</c>. Calling Bell then offers Anchor, Gremlin
/// Horn and Mummified Hand rather than a relic of each rarity from the bag.
/// Cauldron offers five named potions rather than five random ones. Both
/// change what a run holds, so the oracle would record a game nobody plays.
///
/// Each prefix builds the list the other branch builds, word for word, and
/// skips the original. Nothing else in either method reads <c>TestMode</c>.
/// </remarks>
internal static class RewardsOutsideTestMode;

/// <inheritdoc cref="RewardsOutsideTestMode"/>
[HarmonyPatch(typeof(CallingBell), "GenerateRewards")]
internal static class CallingBellRewards
{
    private static bool Prefix(CallingBell __instance, ref List<Reward> __result)
    {
        __result = new List<Reward>
        {
            new RelicReward(RelicRarity.Common, __instance.Owner),
            new RelicReward(RelicRarity.Uncommon, __instance.Owner),
            new RelicReward(RelicRarity.Rare, __instance.Owner),
        };
        return false;
    }
}

/// <inheritdoc cref="RewardsOutsideTestMode"/>
[HarmonyPatch(typeof(Cauldron), "GenerateRewards")]
internal static class CauldronRewards
{
    private static bool Prefix(Cauldron __instance, ref List<Reward> __result)
    {
        int count = __instance.DynamicVars["Potions"].IntValue;
        __result = new List<Reward>();
        for (int i = 0; i < count; i++)
        {
            __result.Add(new PotionReward(__instance.Owner));
        }
        return false;
    }
}

/// <summary>
/// Lord's Parasol buys the whole merchant, as the game does, with no screen.
/// </summary>
/// <remarks>
/// <c>PurchaseEverything</c> skips its first block of screen work while
/// <c>TestMode</c> is on. Each relic it buys then enables and disables the
/// map and deck buttons through <c>NRun.Instance</c> with no such guard, and
/// the removal at the end toggles travel through <c>NMapScreen.Instance</c>.
/// Headless those are null, so the purchases died at the first relic. This is
/// the game's own order of purchases without the screen calls and the waits
/// that pace them. It is tracked, as the <c>buy</c> handler's purchase is,
/// so the driver waits for it and for the removal's question. The removal is
/// marked used, as the shop screen's <c>OnCardRemovalUsed</c> marks it.
/// </remarks>
[HarmonyPatch(typeof(LordsParasol), "PurchaseEverything")]
internal static class ParasolWithoutAScreen
{
    private static bool Prefix(LordsParasol __instance, MerchantInventory inventory, ref Task __result)
    {
        __result = PurchaseEverything(__instance, inventory);
        BackgroundWork.Track(__result);
        return false;
    }

    private static async Task PurchaseEverything(LordsParasol parasol, MerchantInventory inventory)
    {
        if (inventory.Player != parasol.Owner)
        {
            return;
        }
        foreach (MerchantCardEntry entry in inventory.CharacterCardEntries)
        {
            if (entry.IsStocked)
            {
                await entry.OnTryPurchaseWrapper(inventory, ignoreCost: true);
            }
        }
        foreach (MerchantCardEntry entry in inventory.ColorlessCardEntries)
        {
            if (entry.IsStocked)
            {
                await entry.OnTryPurchaseWrapper(inventory, ignoreCost: true);
            }
        }
        foreach (MerchantRelicEntry entry in inventory.RelicEntries)
        {
            await entry.OnTryPurchaseWrapper(inventory, ignoreCost: true);
        }
        foreach (MerchantPotionEntry entry in inventory.PotionEntries)
        {
            await entry.OnTryPurchaseWrapper(inventory, ignoreCost: true);
        }
        if (inventory.CardRemovalEntry is { } removal
            && await removal.OnTryPurchaseWrapper(inventory, ignoreCost: true, cancelable: false))
        {
            removal.SetUsed();
        }
    }
}

/// <summary>
/// Trial's Double Down asks its abandon-run popup as a question.
/// </summary>
/// <remarks>
/// <c>DoubleDown</c> adds an <c>NAbandonRunConfirmPopup</c> to
/// <c>NModalContainer.Instance</c>, which is null headless. The popup's yes
/// calls <c>RunManager.Abandon</c>, which runs <c>AbandonInternal</c> for a
/// single player, and its no closes it on the Reject page. This parks the two
/// buttons as a <see cref="Confirmation"/>.
///
/// On a yes it runs what <c>AbandonInternal</c> does to the run: it marks the
/// run abandoned and awaits <c>GuaranteeKillAllPlayers</c>. It leaves out the
/// screens <c>AbandonInternal</c> closes first, which take the headless
/// process down with no message.
///
/// In a co-op run the option runs on every seat, for whichever player chose
/// it, so every seat shows the popup and asks here. Its yes is the popup's:
/// <c>RunManager.Abandon</c>, which on the host abandons the run for every
/// seat, through <see cref="AbandonedWithoutScreens"/>. On a client it throws,
/// as it does with the screen.
/// </remarks>
[HarmonyPatch(typeof(Trial), "DoubleDown")]
internal static class DoubleDownWithoutAPopup
{
    private static bool Prefix(ref Task __result)
    {
        __result = Asked();
        return false;
    }

    private static async Task Asked()
    {
        bool yes;
        try
        {
            yes = await Confirmation.Ask();
        }
        catch (OperationCanceledException)
        {
            // The popup went away unanswered: the host abandoned the run, and
            // a client's screens went back to the menu with it.
            return;
        }
        if (!yes)
        {
            return;
        }
        if (Peers.Shared)
        {
            RunManager.Instance.Abandon();
            return;
        }
        Log.Info("Abandoning an in-progress run (player-initiated)");
        await AbandonedWithoutScreens.Abandon();
    }
}

/// <summary>
/// A co-op run abandoned, with no screens to close.
/// </summary>
/// <remarks>
/// The host's <c>RunLobby.AbandonRun</c>, and a client hearing of it, both
/// tell <c>RunManager</c>'s <c>IRunLobbyListener.RunAbandoned</c>. That closes
/// the map screen and an overlay, then runs <c>AbandonInternal</c>, which
/// closes them again, marks the run abandoned and kills every player. A client
/// then shows an error popup. Headless the screens and the popup are null.
/// This runs what is left: the run marked abandoned and every player killed.
/// The host's message and each seat's disconnect are the game's own.
/// </remarks>
[HarmonyPatch]
internal static class AbandonedWithoutScreens
{
    private static readonly MethodInfo KillAllPlayers =
        AccessTools.Method(typeof(RunManager), "GuaranteeKillAllPlayers")
        ?? throw new InvalidOperationException("RunManager no longer has GuaranteeKillAllPlayers");

    private static readonly PropertyInfo Abandoned =
        AccessTools.Property(typeof(RunManager), nameof(RunManager.IsAbandoned))
        ?? throw new InvalidOperationException("RunManager no longer has IsAbandoned");

    private static MethodBase TargetMethod() =>
        AccessTools.Method(typeof(RunManager), "MegaCrit.Sts2.Core.Multiplayer.Game.Lobby.IRunLobbyListener.RunAbandoned")
        ?? throw new InvalidOperationException("RunManager no longer hears RunAbandoned");

    private static bool Prefix()
    {
        Log.Info("The host told us to abandon the run");
        TaskHelper.RunSafely(Abandon());
        return false;
    }

    internal static async Task Abandon()
    {
        // Leaving the run closes any popup still up, as the menu replaces it.
        Pending.CancelOutstanding("the run was abandoned");
        Abandoned.SetValue(RunManager.Instance, true);
        await (Task)KillAllPlayers.Invoke(RunManager.Instance, null)!;
    }
}

/// <summary>
/// A merchant's card removal, marked used where a listener refuses it.
/// </summary>
/// <remarks>
/// <c>NMerchantCardRemoval.FillSlot</c> asks
/// <c>Hook.ShouldAllowMerchantCardRemoval</c> as the shop screen fills the
/// removal's slot, and marks the removal used on a no. Headless there is no
/// screen, so a Hoarder run sold a removal the game never offers. This asks
/// as the merchant's stock is made instead, which the screen only shows.
/// </remarks>
[HarmonyPatch(typeof(MerchantInventory), nameof(MerchantInventory.CreateForNormalMerchant))]
internal static class RemovalRefusedWithoutAScreen
{
    private static void Postfix(Player player, MerchantInventory __result)
    {
        if (__result.CardRemovalEntry is { } removal
            && !MegaCrit.Sts2.Core.Hooks.Hook.ShouldAllowMerchantCardRemoval(player.RunState, player))
        {
            removal.SetUsed();
        }
    }
}

/// <summary>
/// Asks for a card reward that <c>Reward.SelectUnsynchronized</c> selects
/// outside any set, before its selection runs.
/// </summary>
/// <remarks>
/// Only <c>Draft</c> does. A set's own rewards go through
/// <c>RewardsSetSynchronizer</c>, which <see cref="RewardChoice"/> answers
/// first. The selection itself is the game's, run once the card is known.
/// </remarks>
[HarmonyPatch(typeof(Reward), nameof(Reward.SelectUnsynchronized))]
internal static class LoneCardRewardAsks
{
    private static bool _answered;

    private static bool Prefix(Reward __instance, ref Task<bool> __result)
    {
        // The screen asks the local player alone. Another seat's reward
        // waits for that seat's choice, as the game's own code does.
        if (_answered || __instance is not CardReward card || !LocalContext.IsMe(__instance.Player)
            || __instance.ParentRewardSet != null
            || RewardPlan.CardInFlight != null || RewardPlan.AlternativeInFlight != null)
        {
            return true;
        }

        __result = Asked(card);
        return false;
    }

    private static async Task<bool> Asked(CardReward card)
    {
        int? index = await LoneCardReward.Ask(card);
        try
        {
            if (index == null)
            {
                return false;
            }

            RewardPlan.CardInFlight = index;
            Task<bool> selected;
            _answered = true;
            try
            {
                selected = card.SelectUnsynchronized();
            }
            finally
            {
                _answered = false;
            }

            try
            {
                return await selected;
            }
            finally
            {
                RewardPlan.CardInFlight = null;
            }
        }
        finally
        {
            BackgroundWork.Release();
        }
    }
}

/// <summary>
/// Yields where a player's game yields to save the run.
/// </summary>
/// <remarks>
/// A player's run saves through <c>RunSaveManager.SaveRun</c>, whose write is
/// <c>Stream.WriteAsync</c> on a stream that does not override it, so it
/// finishes on the thread pool and the save resumes on a later frame. A
/// headless run starts through <c>SetUpTest</c> with saving off, and the
/// method returns at once instead.
///
/// The order of the game's own code depends on that yield. A won fight saves
/// in <c>EndCombatInternal</c>, before <c>CombatEnded</c> cancels the actions
/// still queued. When a hook's action is what ends the fight, its finish
/// resumes the turn's end on the same stack, and the fight ends before the
/// action pops itself off its queue. Without the yield, <c>CombatEnded</c>
/// removes the finished action, and its pop throws "didn't find it in any
/// queue". IRONCLAD C000001140 with Insanity does this, when Stampede
/// auto-plays Headbutt at a turn's end.
///
/// Nothing is written. The yield is the whole of what the save does here.
/// It counts as background work, so the driver does not read a position
/// while the code after the save is still to run. A won fight clears
/// <c>IsInProgress</c> before it saves, and offers its rewards after.
/// </remarks>
[HarmonyPatch(typeof(RunSaveManager), nameof(RunSaveManager.SaveRun), typeof(AbstractRoom))]
internal static class SaveYields
{
    private static bool Prefix(ref Task __result)
    {
        if (RunManager.Instance.ShouldSave)
        {
            return true;
        }
        __result = Yielded();
        return false;
    }

    private static async Task Yielded()
    {
        BackgroundWork.Hold();
        try
        {
            await Task.Yield();
        }
        finally
        {
            BackgroundWork.Release();
        }
    }
}

/// <summary>
/// Lets a Foul Potion be thrown at a merchant without the merchant's screen.
/// </summary>
/// <remarks>
/// <c>FoulPotion.PassesCustomUsabilityCheck</c> passes outside a fight only
/// when <c>GetFoulPotionMerchantTarget</c> finds the merchant's button: on an
/// <c>NMerchantRoom</c>, or the Fake Merchant's <c>NFakeMerchant</c>, whose
/// inventory is closed. Headless neither node exists, so the button is always
/// null and the throw is never offered.
///
/// Headless no inventory is ever open, so the button would be there in a
/// <c>MerchantRoom</c>, and in the Fake Merchant's event until its fight
/// starts. <c>NFakeMerchant</c> hides its button once
/// <c>StartedFight</c> is set.
/// </remarks>
[HarmonyPatch(typeof(MegaCrit.Sts2.Core.Models.Potions.FoulPotion),
    nameof(MegaCrit.Sts2.Core.Models.Potions.FoulPotion.PassesCustomUsabilityCheck),
    MethodType.Getter)]
internal static class FoulPotionAtTheMerchant
{
    private static void Postfix(MegaCrit.Sts2.Core.Models.Potions.FoulPotion __instance, ref bool __result)
    {
        if (__result || CombatManager.Instance.IsInProgress)
        {
            return;
        }

        switch (__instance.Owner.RunState.CurrentRoom)
        {
            case MegaCrit.Sts2.Core.Rooms.MerchantRoom:
                __result = true;
                break;
            case MegaCrit.Sts2.Core.Rooms.EventRoom { LocalMutableEvent: FakeMerchant { StartedFight: false } }:
                __result = true;
                break;
        }
    }
}

/// <summary>
/// Throws a Foul Potion at the Fake Merchant as <c>OnUse</c> does with the
/// event's node in place.
/// </summary>
/// <remarks>
/// <c>OnUse</c> returns without doing anything unless the event's
/// <c>Node</c> is an <c>NFakeMerchant</c>, and headless it is null. The
/// node is read for the throw's effect and nothing else, so this runs the
/// rest: <c>FoulPotionThrown</c> on each player's event, which skips its
/// own node and starts the fight. A merchant's room needs no help, since
/// its <c>OnUse</c> reads <c>NRun.Instance</c> with <c>?.</c> and then
/// gains the gold.
/// </remarks>
[HarmonyPatch(typeof(MegaCrit.Sts2.Core.Models.Potions.FoulPotion), "OnUse")]
internal static class ThrownAtTheFakeMerchant
{
    private static bool Prefix(MegaCrit.Sts2.Core.Models.Potions.FoulPotion __instance, ref Task __result)
    {
        if (CombatManager.Instance.IsInProgress
            || __instance.Owner.RunState.CurrentRoom is not MegaCrit.Sts2.Core.Rooms.EventRoom { CanonicalEvent: FakeMerchant } room
            || room.LocalMutableEvent?.Node != null)
        {
            return true;
        }

        __result = Task.WhenAll(__instance.Owner.RunState.Players.Select(player =>
            ((FakeMerchant)RunManager.Instance.EventSynchronizer.GetEventForPlayer(player))
                .FoulPotionThrown(__instance)));
        return false;
    }
}
