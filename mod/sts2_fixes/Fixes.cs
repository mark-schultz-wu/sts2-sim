using System;
using System.Collections.Generic;
using System.Reflection;
using HarmonyLib;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
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
