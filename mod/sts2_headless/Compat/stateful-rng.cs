using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Threading.Tasks;
using System;
using HarmonyLib;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Entities.Rngs;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.GameActions;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

/// <summary>Adapts game builds that serialize each RNG's complete state.</summary>
internal static class GameApi
{
    /// <summary>Whether <see cref="ScreenPathSelections"/> applies.</summary>
    /// <remarks>
    /// Not here. This family signals before it counts the cards, undoes an
    /// ended turn first, and has a <c>LocalSelector</c> that takes the screen's
    /// branch. The patch mirrors the other family's bodies, so this one puts
    /// the selector in that slot instead.
    /// </remarks>
    internal const bool AsksThroughTheScreen = false;

    /// <summary>Whether the selector sits in <c>LocalSelector</c>.</summary>
    /// <remarks>
    /// With a <c>Selector</c> set, <c>CardSelectCmd</c> asks it and skips
    /// <c>SignalPlayerChoiceBegun</c>, so a pick inside a turn-start hook holds
    /// the turn start until it is answered. In <c>LocalSelector</c> the game's
    /// own screen branch runs, signal and all, and the turn goes on to
    /// <c>AfterSideTurnStart</c> before the player is asked, as at the screen.
    /// <see cref="RewardReadsLocalSelector"/> points card rewards at it too.
    /// </remarks>
    internal const bool SelectsLocally = true;

    /// <summary><c>CardSelectCmd.UseSelector</c>, into <c>LocalSelector</c>.</summary>
    internal static System.IDisposable UseSelector(MegaCrit.Sts2.Core.TestSupport.ICardSelector selector)
        => MegaCrit.Sts2.Core.Commands.CardSelectCmd.UseSelector(selector, localOnly: true);

    /// <summary><c>SignalPlayerChoiceBegun</c>, which this family also names the player for.</summary>
    internal static System.Threading.Tasks.Task SignalChoiceBegun(
        MegaCrit.Sts2.Core.GameActions.Multiplayer.PlayerChoiceContext context,
        Player player,
        MegaCrit.Sts2.Core.Entities.Multiplayer.PlayerChoiceOptions options)
        => context.SignalPlayerChoiceBegun(player, options);

    internal static bool CanUseOrRemovePotions(Player player) => player.CanUseOrRemovePotions;

    internal static IReadOnlyDictionary<RunRngType, RngPosition> RunRngCounters(RunState state)
        => state.Rng.ToSerializable().Rngs.ToDictionary(pair => pair.Key, pair => Position(pair.Value));

    internal static IReadOnlyDictionary<PlayerRngType, RngPosition> PlayerRngCounters(Player player)
        => player.PlayerRng.ToSerializable().Rngs.ToDictionary(pair => pair.Key, pair => Position(pair.Value));

    /// <summary>The whole record, which this family saves and the other does not.</summary>
    private static RngPosition Position(MegaCrit.Sts2.Core.Saves.SerializableRng rng)
        => new(rng.counter, rng.state0, rng.state1, rng.state2, rng.state3);

    /// <summary>The shape a selection's choice goes to the other seats in.</summary>
    private enum Shape { Index, Indexes, CombatCard, CombatCards, DeckCards, Bundle }

    /// <summary>Each selection method, and the shape its screen branch sends.</summary>
    private static readonly Dictionary<string, Shape> Shapes = new()
    {
        [nameof(CardSelectCmd.FromChooseACardScreen)] = Shape.Index,
        [nameof(CardSelectCmd.FromSimpleGridForRewards)] = Shape.Indexes,
        [nameof(CardSelectCmd.FromSimpleGrid)] = Shape.Indexes,
        [nameof(CardSelectCmd.FromCombatPile)] = Shape.CombatCards,
        [nameof(CardSelectCmd.FromDeckForUpgrade)] = Shape.DeckCards,
        [nameof(CardSelectCmd.FromDeckForTransformation)] = Shape.DeckCards,
        [nameof(CardSelectCmd.FromDeckForEnchantment)] = Shape.DeckCards,
        [nameof(CardSelectCmd.FromDeckGeneric)] = Shape.DeckCards,
        [nameof(CardSelectCmd.FromHand)] = Shape.CombatCards,
        [nameof(CardSelectCmd.FromHandForUpgrade)] = Shape.CombatCard,
        [nameof(CardSelectCmd.FromChooseABundleScreen)] = Shape.Bundle,
    };

    /// <summary>
    /// The <c>CardSelectCmd</c> method asking now, as the stack names it. Its
    /// state machine calls the selector, and is never inlined.
    /// </summary>
    private static string? AskingMethod()
    {
        foreach (var frame in new System.Diagnostics.StackTrace().GetFrames())
        {
            Type? type = frame.GetMethod()?.DeclaringType;
            if (type?.DeclaringType == typeof(CardSelectCmd) && type.Name.StartsWith('<'))
            {
                return type.Name[1..type.Name.IndexOf('>')];
            }
        }
        return null;
    }

    /// <summary>
    /// A local selection, sent on once it is made, as the selection screen
    /// sends it.
    /// </summary>
    /// <remarks>
    /// <c>CardSelectCmd</c> reserves a choice id for the selection just before
    /// it asks <c>LocalSelector</c>, which this mod answers through. Its screen
    /// branch then sends the choice with <c>SyncLocalChoice</c>, in a shape
    /// each method picks, and a co-op seat waiting on it reads that. The
    /// selector's branch sends nothing. So this sends what the screen would
    /// have: the id just reserved, in the method's shape. Alone, the game
    /// sends it to nobody.
    /// </remarks>
    internal static async Task<IEnumerable<CardModel>> Sent(List<CardModel> options, Task<IEnumerable<CardModel>> asked)
    {
        // Before the first await, while the asking method is on the stack.
        string? method = AskingMethod();
        RunManager run = RunManager.Instance;
        RunState state = run.DebugOnlyGetState()!;
        Player me = Seat.Me(state);
        IReadOnlyList<uint> ids = run.PlayerChoiceSynchronizer.ChoiceIds;
        int slot = state.GetPlayerSlotIndex(me);
        List<CardModel> chosen = (await asked).ToList();
        if (method == null || !Shapes.TryGetValue(method, out Shape shape) || slot >= ids.Count)
        {
            if (Peers.Shared)
            {
                throw new NotSupportedException($"a co-op selection asked by {method ?? "no screen method"}");
            }
            return chosen;
        }
        PlayerChoiceResult result = shape switch
        {
            // No pick is -1, as the screen's IndexOf sends it. An empty index
            // reads back as 0 on the other seat, which is the first card.
            Shape.Index => PlayerChoiceResult.FromIndex(chosen.Count == 0 ? -1 : options.IndexOf(chosen[0])),
            Shape.Indexes => PlayerChoiceResult.FromIndexes(chosen.Select(c => options.IndexOf(c)).ToList()),
            Shape.CombatCard => PlayerChoiceResult.FromMutableCombatCard(chosen.FirstOrDefault()),
            Shape.CombatCards => PlayerChoiceResult.FromMutableCombatCards(chosen),
            Shape.DeckCards => PlayerChoiceResult.FromMutableDeckCards(chosen),
            _ => throw new NotSupportedException($"sending a selection shaped {shape}"),
        };
        run.PlayerChoiceSynchronizer.SyncLocalChoice(me, ids[slot] - 1, result);
        return chosen;
    }
}
