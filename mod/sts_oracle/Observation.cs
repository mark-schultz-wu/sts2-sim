using System.Collections.Generic;
using System.Linq;
using System.Text.Json;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Entities.Rngs;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;

namespace StsOracle;

/// <summary>
/// Everything the simulator is compared against after a decision (R5).
/// </summary>
/// <remarks>
/// Deliberately a superset of the game's own <c>NetFullCombatState</c>, which is
/// a divergence *fingerprint* rather than a state description: it carries piles
/// only while combat is in progress, drops the Rewards and Shops RNG streams
/// from what it compares, and omits monster intent entirely because two peers
/// can each re-derive it. None of those omissions are safe for us.
///
/// Nothing here may be defaulted when unavailable. A field the oracle cannot
/// fill is reported absent, so a simulator disagreement surfaces rather than
/// being papered over by a shared zero (R12).
/// </remarks>
internal static class Observation
{
    private static readonly JsonSerializerOptions Json = new JsonSerializerOptions { WriteIndented = false };

    public static string Capture(RunState state)
    {
        Player me = state.Players[0];
        CombatState? combat = (state.CurrentRoom as CombatRoom)?.CombatState;
        PlayerCombatState? pcs = me.PlayerCombatState;

        var obs = new Dictionary<string, object?>
        {
            ["act"] = state.CurrentActIndex,
            ["floor"] = state.TotalFloor,
            ["hp"] = me.Creature.CurrentHp,
            ["maxHp"] = me.Creature.MaxHp,
            ["gold"] = me.Gold,
            ["relics"] = me.Relics.Select(Ident).ToList(),
            ["potions"] = me.Potions.Select(p => p.Id.Entry).ToList(),
            ["deck"] = me.Deck.Cards.Select(Ident).ToList(),
            ["inCombat"] = CombatManager.Instance.IsInProgress,
            ["rng"] = Counters(state, me),
        };

        // Absent rather than empty: "not in combat" and "in combat with nothing
        // in hand" must not serialise the same way.
        if (combat != null && pcs != null)
        {
            obs["combat"] = new Dictionary<string, object?>
            {
                ["turn"] = pcs.TurnNumber,
                ["phase"] = pcs.Phase.ToString(),
                ["energy"] = pcs.Energy,
                ["round"] = combat.RoundNumber,
                ["hand"] = pcs.Hand.Cards.Select(Ident).ToList(),
                ["draw"] = pcs.DrawPile.Cards.Select(Ident).ToList(),
                ["discard"] = pcs.DiscardPile.Cards.Select(Ident).ToList(),
                ["exhaust"] = pcs.ExhaustPile.Cards.Select(Ident).ToList(),
                ["creatures"] = combat.Creatures.Select(Describe).ToList(),
                ["legal"] = Legal(combat, pcs).ToList(),
            };
        }

        return JsonSerializer.Serialize(obs, Json);
    }

    /// <summary>Card identity: the id alone is not enough to compare two decks.</summary>
    private static string Ident(CardModel card)
    {
        string s = card.Id.Entry;
        if (card.CurrentUpgradeLevel > 0)
        {
            s += "+" + card.CurrentUpgradeLevel;
        }
        if (card.Affliction != null)
        {
            s += "/" + card.Affliction.Id.Entry;
        }
        return s;
    }

    private static string Ident(RelicModel relic) => relic.Id.Entry;

    private static Dictionary<string, object?> Describe(Creature c)
    {
        return new Dictionary<string, object?>
        {
            ["id"] = c.CombatId,
            ["model"] = c.Monster?.Id.Entry ?? c.Player?.Character.Id.Entry,
            ["side"] = c.Side.ToString(),
            ["hp"] = c.CurrentHp,
            ["maxHp"] = c.MaxHp,
            ["block"] = c.Block,
            ["alive"] = c.IsAlive,
            ["powers"] = c.Powers.Select(p => new Dictionary<string, object?>
            {
                ["id"] = p.Id.Entry,
                ["amount"] = p.Amount,
            }).ToList(),

            // The game never ships intent — NetFullCombatState omits it because
            // peers re-derive it, and MonsterModel.NextMove is serialised
            // nowhere. A simulator that picks the wrong monster move is exactly
            // the bug this catches, so it is reported here.
            ["intent"] = c.Monster?.NextMove.Id,
        };
    }

    /// <summary>
    /// The legal command set, with the reason each illegal card is illegal.
    /// </summary>
    /// <remarks>
    /// Comparing reasons rather than booleans localises a simulator bug to the
    /// rule it got wrong. <c>UnplayableReason</c> is a [Flags] enum covering the
    /// unplayable keyword, energy and star cost, no-living-allies, hook blocks
    /// and card logic.
    /// </remarks>
    private static IEnumerable<Dictionary<string, object?>> Legal(CombatState combat, PlayerCombatState pcs)
    {
        List<Creature> alive = combat.Creatures.Where(c => c.IsAlive).ToList();

        foreach (CardModel card in pcs.Hand.Cards)
        {
            bool canPlay = card.CanPlay(out UnplayableReason reason, out AbstractModel? preventer);
            yield return new Dictionary<string, object?>
            {
                ["card"] = Ident(card),
                ["cost"] = card.EnergyCost.GetWithModifiers(CostModifiers.All),
                ["target"] = card.TargetType.ToString(),
                ["playable"] = canPlay,
                ["reason"] = reason == UnplayableReason.None ? null : reason.ToString(),
                ["preventer"] = preventer?.Id.Entry,
                ["targets"] = alive.Where(card.CanPlayTargeting).Select(c => c.CombatId).ToList(),
            };
        }
    }

    /// <summary>All fifteen RNG streams: twelve run-level, three per-player.</summary>
    /// <remarks>
    /// Two implementations can agree on every visible field while having
    /// consumed different numbers of draws. That divergence is invisible in the
    /// state and fatal three decisions later, so counters are compared directly.
    /// </remarks>
    private static Dictionary<string, int> Counters(RunState state, Player me)
    {
        var counters = new Dictionary<string, int>();
        foreach (KeyValuePair<RunRngType, int> c in state.Rng.ToSerializable().Counters)
        {
            counters[c.Key.ToString()] = c.Value;
        }
        foreach (KeyValuePair<PlayerRngType, int> c in me.PlayerRng.ToSerializable().Counters)
        {
            counters["player." + c.Key] = c.Value;
        }
        return counters;
    }
}
