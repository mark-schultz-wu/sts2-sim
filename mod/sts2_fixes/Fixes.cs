using System;
using System.Collections.Generic;
using System.Reflection;
using System.Threading.Tasks;
using HarmonyLib;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.GameActions;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer;

namespace Sts2Fixes;

/// <summary>
/// Pops the cards a dead player's plays left on a choice context.
/// </summary>
/// <remarks>
/// <c>CardModel.OnPlayWrapper</c> pushes its card onto the choice context's
/// model stack, and pops it at the end. Once its owner is dead it returns
/// early, before that pop, so the card stays on the stack. Stampede shows it:
/// its post-play hook auto-plays Breakthrough, whose loss of HP kills the
/// player. The hook's own pop then finds the card on top, logs "Tried to pop
/// model", and leaves both on the stack. The fight is lost by then, and
/// nothing reads the stack again.
///
/// This pops the dead player's cards off the top first, in place of the pops
/// each wrapper skipped, so the pop that follows finds its own model. Any
/// other mismatch still logs.
/// </remarks>
[HarmonyPatch(typeof(PlayerChoiceContext), nameof(PlayerChoiceContext.PopModel))]
internal static class PopCardsADeathLeft
{
    private static readonly AccessTools.FieldRef<PlayerChoiceContext, Stack<AbstractModel>?> ModelStack =
        AccessTools.FieldRefAccess<PlayerChoiceContext, Stack<AbstractModel>?>("_modelStack");

    private static void Prefix(PlayerChoiceContext __instance, AbstractModel model)
    {
        var stack = ModelStack(__instance);
        if (stack == null || !stack.Contains(model))
        {
            return;
        }
        while (stack.TryPeek(out var top)
            && top != model
            && top is CardModel card
            && card.Owner?.Creature?.IsDead == true)
        {
            stack.Pop();
        }
    }
}

/// <summary>
/// Gives each message its own list of handlers to walk.
/// </summary>
/// <remarks>
/// <c>NetMessageBus.SendMessageToAllHandlers</c> copies a message's handlers
/// into one list the bus keeps, <c>_cachedPairList</c>, and walks it. A handler
/// that sends a message on, as a client's disconnect on hearing the run was
/// abandoned does, clears and refills that same list under the walk, which
/// throws "Collection was modified". The run is over by then. This hands each
/// call a list of its own, so a call inside another leaves the outer walk be.
/// </remarks>
[HarmonyPatch(typeof(NetMessageBus), nameof(NetMessageBus.SendMessageToAllHandlers))]
internal static class HandlersWalkTheirOwnList
{
    private static readonly FieldInfo CachedPairs =
        AccessTools.Field(typeof(NetMessageBus), "_cachedPairList")
        ?? throw new InvalidOperationException("NetMessageBus no longer has _cachedPairList");

    private static void Prefix(NetMessageBus __instance) =>
        CachedPairs.SetValue(__instance, Activator.CreateInstance(CachedPairs.FieldType));
}

/// <summary>
/// Marks the queues empty again after an action they turned away.
/// </summary>
/// <remarks>
/// <c>ActionQueueSet.EnqueueWithoutSynchronizing</c> opens a new
/// <c>_queuesEmptyCompletionSource</c> before it looks at the action. A queue
/// that is cancelling its combat actions then cancels the action rather than
/// add it, and nothing completes the source, since no action runs to call
/// <c>CheckIfQueuesEmpty</c>. So <c>IsEmpty</c> stays false with every queue
/// empty. A co-op fight won as a turn starts shows it: a teammate's paused
/// setup arrives as a hook action once the fight is over. This runs the
/// game's own check after each enqueue, which completes the source only where
/// every queue is empty.
/// </remarks>
[HarmonyPatch(typeof(ActionQueueSet), nameof(ActionQueueSet.EnqueueWithoutSynchronizing))]
internal static class EmptyAfterATurnedAwayAction
{
    private static readonly MethodInfo CheckIfQueuesEmpty =
        AccessTools.Method(typeof(ActionQueueSet), "CheckIfQueuesEmpty")
        ?? throw new InvalidOperationException("ActionQueueSet no longer has CheckIfQueuesEmpty");

    private static void Postfix(ActionQueueSet __instance) =>
        CheckIfQueuesEmpty.Invoke(__instance, null);
}

/// <summary>
/// Finishes a potion discard whose slot is empty without cancelling it.
/// </summary>
/// <remarks>
/// <c>DiscardPotionGameAction</c> cancels itself when the slot it names holds
/// no potion. That happens on another player's machine while the player's
/// own machine is still in a purchase, such as Cauldron's potions, whose
/// results have not come yet. The action then finishes in the cancelled
/// state, and <c>GameAction.Execute</c> reports that. The game in a window
/// logs it as an error and goes on. Without a window it throws, which stops
/// the action executor. The cancel does nothing else here: with no potion,
/// <c>CancelAction</c> touches nothing. This logs the game's own warning
/// and finishes the action, as the game in a window does.
/// </remarks>
[HarmonyPatch(typeof(DiscardPotionGameAction), "ExecuteAction")]
internal static class DiscardOfAnEmptySlotFinishes
{
    private static readonly AccessTools.FieldRef<DiscardPotionGameAction, Player> Owner =
        AccessTools.FieldRefAccess<DiscardPotionGameAction, Player>("_player");

    private static readonly AccessTools.FieldRef<DiscardPotionGameAction, uint> Slot =
        AccessTools.FieldRefAccess<DiscardPotionGameAction, uint>("_potionSlotIndex");

    private static bool Prefix(DiscardPotionGameAction __instance, ref Task __result)
    {
        var player = Owner(__instance);
        var slot = Slot(__instance);
        if (slot >= player.PotionSlots.Count || player.PotionSlots[(int)slot] != null)
        {
            return true;
        }
        Log.Warn($"DiscardPotionGameAction: potion at slot index {slot} is null for player {player.NetId}, canceling");
        __result = Task.CompletedTask;
        return false;
    }
}
