using System.Collections.Generic;
using System.Linq;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Merchant;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Entities.RestSite;
using MegaCrit.Sts2.Core.Events;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Models.Events;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

/// <summary>Every move the run accepts right now, as the commands that make them.</summary>
/// <remarks>
/// What the game's screens offer, read off the game. A card is offered where
/// <c>CanPlay</c> says it can be played, at each creature
/// <c>CanPlayTargeting</c> accepts, or at nobody where that is accepted. A
/// potion is offered where the belt's buttons would be enabled. A room offers
/// the options and stock its screen leaves enabled. The map offers the points
/// <see cref="MapLegality"/> allows, or proceeding where that is the way on.
///
/// The order is part of the answer, because a driver that picks by index from
/// a seeded die plays the same run only when the list comes back the same
/// way. It is the simulator's `Sim::legal`. A pending question's answers come
/// alone. Otherwise the belt comes first, then the room's options, then the
/// fight's plays and the end of the turn, or the map.
/// </remarks>
public static class Moves
{
    public static List<Dictionary<string, object?>> Legal(RunState state)
    {
        Player me = state.Players[0];
        CombatState? combat = (state.CurrentRoom as CombatRoom)?.CombatState;
        PlayerCombatState? pcs = me.PlayerCombatState;
        bool fighting = combat != null && pcs != null && CombatManager.Instance.IsInProgress;
        Pending? pending = Pending.Current;

        // A card selection is answered whoever is standing, inside a fight.
        if (pending is CardSelection && (me.Creature.IsAlive || fighting))
        {
            return pending.Answers();
        }

        if (fighting)
        {
            return Fighting(state, me, combat!, pcs!);
        }

        // `IsGameOver`: a run whose player is dead takes nothing more.
        if (!me.Creature.IsAlive)
        {
            return new List<Dictionary<string, object?>>();
        }

        if (pending != null)
        {
            return pending.Answers();
        }

        List<Dictionary<string, object?>> moves = Drinks(me, state);
        moves.AddRange(Room(state, me));
        if (MapLegality.CanProceed(state))
        {
            moves.Add(Command("proceed"));
        }
        else
        {
            moves.AddRange(MapLegality.TravelableFrom(state).Select(p => new Dictionary<string, object?>
            {
                ["cmd"] = "travel",
                ["col"] = p.coord.col,
                ["row"] = p.coord.row,
            }));
        }

        return moves;
    }

    /// <summary>The belt, then each playable card at each creature it takes, then the turn's end.</summary>
    private static List<Dictionary<string, object?>> Fighting(
        RunState state, Player me, CombatState combat, PlayerCombatState pcs)
    {
        List<Dictionary<string, object?>> moves = Drinks(me, state);
        List<Creature> alive = combat.Creatures.Where(c => c.IsAlive).ToList();
        IReadOnlyList<CardModel> hand = pcs.Hand.Cards;
        for (int i = 0; i < hand.Count; i++)
        {
            CardModel card = hand[i];
            if (!card.CanPlay(out UnplayableReason _, out AbstractModel? _))
            {
                continue;
            }

            List<uint?> targets = alive.Where(card.CanPlayTargeting).Select(c => c.CombatId).ToList();
            if (targets.Count == 0)
            {
                if (card.CanPlayTargeting(null))
                {
                    moves.Add(Aimed("play_card", i, null));
                }

                continue;
            }

            moves.AddRange(targets.Select(target => Aimed("play_card", i, target)));
        }

        moves.Add(Command("end_turn"));
        return moves;
    }

    /// <summary>Each potion the belt would let be drunk, at each creature it can be thrown at.</summary>
    private static List<Dictionary<string, object?>> Drinks(Player me, RunState state)
    {
        var moves = new List<Dictionary<string, object?>>();
        List<PotionModel> belt = me.Potions.ToList();
        for (int i = 0; i < belt.Count; i++)
        {
            if (Observation.Undrinkable(belt[i], me) != null)
            {
                continue;
            }

            List<uint> targets = Observation.Targets(belt[i], state);
            if (targets.Count == 0)
            {
                // `UsePotion` with no target falls back to the drinker. A
                // potion thrown at one other creature is valid at neither, so
                // with none to throw it at there is nothing to drink.
                if (belt[i].IsValidTarget(me.Creature) || belt[i].IsValidTarget(null))
                {
                    moves.Add(Aimed("use_potion", i, null));
                }
            }
            else
            {
                moves.AddRange(targets.Select(target => Aimed("use_potion", i, target)));
            }
        }

        return moves;
    }

    /// <summary>The options a room's screen leaves enabled, and the stock it lets the run buy.</summary>
    private static IEnumerable<Dictionary<string, object?>> Room(RunState state, Player me)
    {
        switch (state.CurrentRoom)
        {
            case RestSiteRoom:
            {
                IReadOnlyList<RestSiteOption> options =
                    RunManager.Instance.RestSiteSynchronizer.GetLocalOptions();
                return Enumerable.Range(0, options.Count)
                    .Where(i => options[i].IsEnabled)
                    .Select(Choosing)
                    .ToList();
            }

            // The relics, then the skip one past them, while the chest is open.
            case TreasureRoom:
            {
                IReadOnlyList<RelicModel>? relics =
                    RunManager.Instance.TreasureRoomRelicSynchronizer.CurrentRelics;
                if (relics == null || !Chest.Open(me))
                {
                    return new List<Dictionary<string, object?>>();
                }

                return Enumerable.Range(0, relics.Count + 1).Select(Choosing).ToList();
            }

            case EventRoom:
            {
                EventModel ev = RunManager.Instance.EventSynchronizer.GetLocalEvent();
                if (ev is FakeMerchant { Inventory: { } fake })
                {
                    return Buying(fake, me);
                }

                IReadOnlyList<EventOption> options = ev.CurrentOptions;
                return Enumerable.Range(0, options.Count)
                    .Where(i => !options[i].IsLocked && !options[i].WasChosen)
                    .Select(Choosing)
                    .ToList();
            }

            case MerchantRoom shop:
                return Buying(shop.GetLocalInventory(), me);

            default:
                return new List<Dictionary<string, object?>>();
        }
    }

    /// <summary>Each entry stocked, affordable, and not blocked, which is what <c>buy</c> refuses on.</summary>
    private static List<Dictionary<string, object?>> Buying(MerchantInventory inventory, Player me)
    {
        List<MerchantEntry> entries = inventory.AllEntries.ToList();
        return Enumerable.Range(0, entries.Count)
            .Where(i => entries[i].IsStocked && entries[i].EnoughGold
                && Observation.Blocked(entries[i], me) == null)
            .Select(i => new Dictionary<string, object?> { ["cmd"] = "buy", ["index"] = i })
            .ToList();
    }

    private static Dictionary<string, object?> Choosing(int index) =>
        new Dictionary<string, object?> { ["cmd"] = "choose_option", ["index"] = index };

    private static Dictionary<string, object?> Aimed(string cmd, int index, uint? target) =>
        new Dictionary<string, object?> { ["cmd"] = cmd, ["index"] = index, ["target"] = target };

    private static Dictionary<string, object?> Command(string cmd) =>
        new Dictionary<string, object?> { ["cmd"] = cmd };
}
