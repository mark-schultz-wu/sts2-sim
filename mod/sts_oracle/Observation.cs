using System;
using System.Collections.Generic;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Entities.Rngs;
using MegaCrit.Sts2.Core.Entities.Merchant;
using MegaCrit.Sts2.Core.Entities.RestSite;
using MegaCrit.Sts2.Core.Events;
using MegaCrit.Sts2.Core.Map;
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

    /// <summary>
    /// A digest of the full observation, for step-by-step comparison (R6).
    /// </summary>
    /// <remarks>
    /// The full observation runs ~1.6KB per decision — 27% creatures, 27% legal
    /// moves, 19% RNG counters, 11% deck, most of it repeated key names for
    /// values that rarely change. At 250 decisions/sec that is 400KB/s, which is
    /// wasteful when almost every step agrees.
    ///
    /// So the steady state compares digests, and the full observation is only
    /// needed for the step that disagrees. **This is affordable precisely
    /// because R1 holds**: a diverging run can be replayed with
    /// <c>STS_ORACLE_OBS=full</c> and will reproduce exactly, so nothing is lost
    /// by not having shipped the bytes the first time.
    ///
    /// Compacting the JSON was the other option and is the worse one: it would
    /// save perhaps 40% while making the failure path harder to read, and the
    /// failure path is the one that matters.
    /// </remarks>
    public static string Digest(RunState state) => DigestOf(Capture(state));

    /// <summary>
    /// The observation and its digest, built from a single capture.
    /// </summary>
    /// <remarks>
    /// Callers that want both must use this rather than calling
    /// <see cref="Capture"/> and <see cref="Digest"/> in turn: the latter
    /// captures again internally, so a full observation cost two serialisations
    /// of the same state and measurably halved protocol throughput.
    /// </remarks>
    public static (string Json, string Digest) Snapshot(RunState state)
    {
        string json = Capture(state);
        return (json, DigestOf(json));
    }

    private static string DigestOf(string capture)
    {
        byte[] hash = SHA256.HashData(Encoding.UTF8.GetBytes(capture));
        return Convert.ToHexString(hash, 0, 8);
    }

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

            ["alive"] = me.Creature.IsAlive,

            // A win kills every player too — RunManager.WinRun calls
            // GuaranteeKillAllPlayers — so "the run is over" and "you lost" are
            // not the same question. IsVictoryRoom is what separates them, and
            // it is only meaningful once the run has ended.
            ["runOver"] = state.IsGameOver,
            ["victory"] = state.IsGameOver ? state.CurrentRoom?.IsVictoryRoom ?? false : null,
            ["gold"] = me.Gold,
            ["relics"] = me.Relics.Select(Ident).ToList(),
            ["potions"] = me.Potions.Select(p => p.Id.Entry).ToList(),
            ["deck"] = me.Deck.Cards.Select(Ident).ToList(),
            ["inCombat"] = CombatManager.Instance.IsInProgress,
            ["map"] = MapPosition(state),
            ["rng"] = Counters(state, me),
        };

        Dictionary<string, object?>? room = Room(state, me);
        if (room != null)
        {
            obs["room"] = room;
        }

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

                // Ordered, because an orb queue is a queue: the same orbs in a
                // different order evoke differently. Capacity is separate from
                // count, since empty slots are part of the state. Only Defect
                // uses these, which is why they went unwritten until `start`
                // could ask for a character.
                ["orbs"] = new Dictionary<string, object?>
                {
                    ["capacity"] = pcs.OrbQueue.Capacity,
                    ["queue"] = pcs.OrbQueue.Orbs.Select(o => new Dictionary<string, object?>
                    {
                        ["id"] = o.Id.Entry,
                        ["passive"] = o.PassiveVal,
                        ["evoke"] = o.EvokeVal,
                    }).ToList(),
                },
            };
        }

        return JsonSerializer.Serialize(obs, Json);
    }

    /// <summary>Card identity: the id alone is not enough to compare two decks.</summary>
    public static string Ident(CardModel card)
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

    /// <summary>
    /// The choices the current room offers, if it is one that offers any.
    /// </summary>
    /// <remarks>
    /// Rest sites, events and shops are **not** pending decisions: their
    /// triggers are model-owned (`EnterInternal` calls `BeginRestSite`,
    /// `BeginEvent`, or populates `Inventories`), so after travelling the game
    /// is quiescent and idle exactly as at the start of a combat turn. They are
    /// state to be observed and answered with an ordinary command, which is why
    /// they need none of `Pending`'s machinery.
    ///
    /// Legality is taken from the model in every case — `RestSiteOption.IsEnabled`,
    /// `EventOption.IsLocked`, `MerchantEntry.EnoughGold`/`IsStocked` — rather
    /// than recomputed here. Unlike map travel, these rules do not live in a
    /// screen.
    ///
    /// Absent rather than empty when the room offers nothing, so "a shop with no
    /// stock" and "not in a shop" cannot serialise the same way (R12).
    /// </remarks>
    private static Dictionary<string, object?>? Room(RunState state, Player me)
    {
        switch (state.CurrentRoom)
        {
            case RestSiteRoom:
            {
                IReadOnlyList<RestSiteOption> options =
                    RunManager.Instance.RestSiteSynchronizer.GetLocalOptions();
                return new Dictionary<string, object?>
                {
                    ["type"] = "RestSite",
                    ["options"] = options.Select((o, i) => new Dictionary<string, object?>
                    {
                        ["index"] = i,
                        ["id"] = o.OptionId,
                        ["enabled"] = o.IsEnabled,
                    }).ToList(),
                };
            }

            case EventRoom eventRoom:
            {
                EventModel ev = RunManager.Instance.EventSynchronizer.GetLocalEvent();
                return new Dictionary<string, object?>
                {
                    ["type"] = "Event",
                    ["id"] = ev.Id.Entry,
                    ["finished"] = ev.IsFinished,
                    ["options"] = ev.CurrentOptions.Select((o, i) => new Dictionary<string, object?>
                    {
                        ["index"] = i,
                        ["key"] = o.TextKey,
                        ["locked"] = o.IsLocked,
                        ["proceed"] = o.IsProceed,
                        ["chosen"] = o.WasChosen,
                        ["relic"] = o.Relic?.Id.Entry,
                    }).ToList(),
                };
            }

            case MerchantRoom shop:
            {
                MerchantInventory inventory = shop.GetLocalInventory();
                return new Dictionary<string, object?>
                {
                    ["type"] = "Shop",
                    ["entries"] = inventory.AllEntries.Select((e, i) => new Dictionary<string, object?>
                    {
                        ["index"] = i,
                        ["kind"] = e.GetType().Name,
                        ["id"] = EntryId(e),
                        ["cost"] = e.Cost,
                        ["stocked"] = e.IsStocked,
                        ["affordable"] = e.EnoughGold,
                    }).ToList(),
                };
            }

            default:
                return null;
        }
    }

    private static string? EntryId(MerchantEntry entry) => entry switch
    {
        MerchantCardEntry card => card.CreationResult?.Card is { } c ? Ident(c) : null,
        MerchantRelicEntry relic => relic.Model?.Id.Entry,
        MerchantPotionEntry potion => potion.Model?.Id.Entry,
        _ => null,
    };

    /// <summary>
    /// Where we are on the map, and which nodes may be travelled to next.
    /// </summary>
    /// <remarks>
    /// Legality is delegated to <see cref="MapLegality"/> rather than derived
    /// here. Two rules hide in it that a graph walk misses — free travel, and
    /// the boss sitting outside the grid — and both are invisible until the run
    /// reaches the state that triggers them. See that file for the details.
    /// </remarks>
    private static Dictionary<string, object?> MapPosition(RunState state)
    {
        MapPoint? current = state.CurrentMapPoint;
        return new Dictionary<string, object?>
        {
            ["at"] = current == null ? null : Coord(current),
            ["visited"] = state.VisitedMapCoords.Select(c => $"{c.col},{c.row}").ToList(),
            ["travelable"] = MapLegality.TravelableFrom(state)
                .Select(p => new Dictionary<string, object?>
                {
                    ["at"] = Coord(p),
                    ["type"] = p.PointType.ToString(),
                }).ToList(),
        };
    }

    private static string Coord(MapPoint p) => $"{p.coord.col},{p.coord.row}";

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
