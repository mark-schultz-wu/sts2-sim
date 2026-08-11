using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Text.Json;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.Hooks;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Models.Cards;
using MegaCrit.Sts2.Core.Models.Relics;
using MegaCrit.Sts2.Core.Runs;

namespace StsOracle;

/// <summary>
/// Checks that the oracle's reported state actually reflects the game's rules.
/// </summary>
/// <remarks>
/// The failure this defends against is the oracle quietly reimplementing a rule
/// the game already owns, and getting it wrong under conditions nobody tested.
/// The concrete instance: map legality was nearly derived as "the current node's
/// children", which is right until a run holds `Winged Boots` or the `Flight`
/// modifier — both override <c>ShouldAllowFreeTravel</c>, widening the legal set
/// to the whole next row.
///
/// R3's stated check — execute commands outside the reported legal set and
/// require the game to reject them — **cannot catch that**.
/// <c>MapTravel.GetTravelablePointsFrom</c> has exactly one caller,
/// <c>NMapScreen</c>, so the model layer never rejects an illegal map move and
/// the complement fuzz passes vacuously.
///
/// What works is metamorphic: put the game in a state where a rule is active and
/// require the oracle's output to *change*. An oracle that ignores the rule
/// reports the baseline twice and fails.
///
/// **The matrix is discovered, not written down.** Reflection over the game
/// assembly finds every type that overrides a legality hook, so a hook added in
/// a future build shows up here as UNCOVERED rather than silently going
/// untested. Teaching the suite to construct a case is manual; noticing that a
/// case exists is not.
/// </remarks>
internal static class SelfTest
{
    private static int _passed;
    private static int _failed;
    private static int _uncovered;

    /// <summary>Hooks whose effect must be visible in the observation.</summary>
    /// <remarks>
    /// <c>IsAllowed</c> has 55 overrides but is deliberately absent: it decides
    /// whether a relic may appear in a run at all, not what the legal command
    /// set is, so there is no observation field for it to move.
    /// </remarks>
    private static readonly (string Method, Type[] Signature)[] Hooks =
    {
        ("ShouldAllowFreeTravel", Array.Empty<Type>()),
        ("ShouldPlay", new[] { typeof(CardModel), typeof(AutoPlayType) }),
    };

    public static async Task Run(RunState state, Func<Task> enterCombat)
    {
        Dictionary<string, List<string>> matrix = Discover();
        int total = matrix.Values.Sum(v => v.Count);
        Log.Info($"[selftest] discovered {total} legality-hook overrides across {matrix.Count} hooks");

        var covered = new HashSet<string>();

        // Out of combat first: map legality needs a map position, not a fight.
        covered.Add("WingedBoots");
        await FreeTravelWidensTheMap(state);

        await enterCombat();
        covered.Add("Enthralled");
        await BlockedByHookIsReported(state);

        foreach ((string hook, List<string> types) in matrix)
        {
            foreach (string t in types.Where(t => !covered.Contains(t)))
            {
                _uncovered++;
                Log.Warn($"[selftest] UNCOVERED {hook} on {t} — no scenario constructs this yet");
            }
        }

        Log.Info($"[selftest] {_passed} passed, {_failed} failed, {_uncovered} uncovered");
        if (_failed > 0)
        {
            throw new InvalidOperationException($"{_failed} self-test(s) failed");
        }
    }

    /// <summary>
    /// Finds every type overriding a legality hook, from the game's own metadata.
    /// </summary>
    /// <remarks>
    /// A method is an override when its <c>GetBaseDefinition</c> is declared on
    /// some other type — that is what separates `WingedBoots.ShouldAllowFreeTravel`
    /// from `AbstractModel`'s own virtual base.
    /// </remarks>
    private static Dictionary<string, List<string>> Discover()
    {
        var found = new Dictionary<string, List<string>>();
        Assembly game = typeof(AbstractModel).Assembly;

        foreach ((string method, Type[] signature) in Hooks)
        {
            var types = new List<string>();
            foreach (Type t in game.GetTypes())
            {
                if (t.IsAbstract || !typeof(AbstractModel).IsAssignableFrom(t))
                {
                    continue;
                }

                MethodInfo? m = t.GetMethod(method,
                    BindingFlags.Public | BindingFlags.Instance | BindingFlags.DeclaredOnly,
                    binder: null, signature, modifiers: null);

                if (m != null && m.GetBaseDefinition().DeclaringType != t)
                {
                    types.Add(t.Name);
                }
            }
            found[method] = types;
            Log.Info($"[selftest]   {method}: {string.Join(", ", types)}");
        }

        return found;
    }

    /// <summary>Granting Winged Boots must widen the reported travelable set.</summary>
    /// <remarks>
    /// Granted through <c>RelicCmd.Obtain</c> rather than by adding to
    /// <c>Player.Relics</c>, because the load path skips <c>AfterObtained</c>
    /// and would produce a state the game never produces.
    /// </remarks>
    private static async Task<bool> FreeTravelWidensTheMap(RunState state)
    {
        // At floor 1 the next row held exactly this node's two children, so free
        // travel and normal travel coincide and the test proved nothing while
        // appearing to run. A scenario that cannot distinguish the two rules is
        // not evidence about them, so find one that can — or fail saying so.
        MapPoint? spot = state.Map.GetAllMapPoints()
            .OrderBy(p => p.coord.row).ThenBy(p => p.coord.col)
            .FirstOrDefault(p => state.Map.GetPointsInRow(p.coord.row + 1).Count() > p.Children.Count);

        if (spot == null)
        {
            return Check("a map position exists where free travel is observable", false);
        }

        state.AddVisitedMapCoord(spot.coord);
        List<string> before = Travelable(state);

        await RelicCmd.Obtain(ModelDb.Relic<WingedBoots>().ToMutable(), state.Players[0]);

        List<string> after = Travelable(state);
        Check("free travel hook is live", Hook.ShouldAllowFreeTravel(state));

        // The specific claim, not merely "something changed": free travel opens
        // the entire next row, so the new set must be a strict superset.
        bool widened = after.Count > before.Count && before.All(after.Contains);
        if (!widened)
        {
            Log.Error($"[selftest]   before: {string.Join(" ", before)} after: {string.Join(" ", after)}");
        }

        return Check($"WingedBoots widens the travelable set ({before.Count} -> {after.Count})", widened);
    }

    /// <summary>
    /// A hook that forbids card play must show up as BlockedByHook in `legal`.
    /// </summary>
    /// <remarks>
    /// `Enthralled` in hand forbids playing anything except itself, and does so
    /// on presence alone — no turn history or affliction setup, which is what
    /// makes it the cheapest of the seven <c>ShouldPlay</c> overrides to
    /// construct. The other six need a card carrying a specific affliction
    /// (`Smog`, `Ringing`, `Bound`) or a card already played this turn.
    ///
    /// The route matters: this goes through <c>Hook.ShouldPlay</c> into
    /// <c>CardModel.CanPlay</c>, which is what the observation's `legal` block
    /// reads. An oracle that reported raw energy affordability instead of the
    /// game's predicate would still call these cards playable.
    ///
    /// Added with <c>CardPileCmd.Add</c> rather than by pushing onto the pile,
    /// so the card is owned and hooked up the way the game does it.
    /// </remarks>
    private static async Task<bool> BlockedByHookIsReported(RunState state)
    {
        Player me = state.Players[0];
        if (me.PlayerCombatState == null)
        {
            return Check("in combat for the ShouldPlay case", false);
        }

        int playableBefore = PlayableCount(state);
        if (playableBefore == 0)
        {
            return Check("some card is playable before the hook applies", false);
        }

        CardModel enthralled = ModelDb.Card<Enthralled>().ToMutable();
        enthralled.Owner = me;
        await CardPileCmd.Add(enthralled, PileType.Hand.GetPile(me));

        int playableAfter = PlayableCount(state);
        List<string> reasons = ReasonsIn(state).ToList();

        Check($"Enthralled blocks other cards ({playableBefore} -> {playableAfter} playable)",
            playableAfter < playableBefore);

        if (!reasons.Contains(nameof(UnplayableReason.BlockedByHook)))
        {
            Log.Error($"[selftest]   reasons reported: {string.Join(" ", reasons.Distinct())}");
        }

        return Check("the reported reason is BlockedByHook",
            reasons.Contains(nameof(UnplayableReason.BlockedByHook)));
    }

    private static int PlayableCount(RunState state) =>
        Legal(state).Count(e => e.GetProperty("playable").GetBoolean());

    private static IEnumerable<string> ReasonsIn(RunState state) =>
        Legal(state).Select(e => e.GetProperty("reason").GetString() ?? "").SelectMany(r => r.Split(", "));

    /// <summary>
    /// Reads the legal block back out of the emitted observation.
    /// </summary>
    /// <remarks>
    /// Parsed from what the oracle reports rather than recomputed: a test that
    /// re-derives the answer the same way the reporter did would agree with a
    /// broken reporter.
    /// </remarks>
    private static List<JsonElement> Legal(RunState state)
    {
        JsonDocument doc = JsonDocument.Parse(Observation.Capture(state));
        return doc.RootElement.TryGetProperty("combat", out JsonElement c)
            ? c.GetProperty("legal").EnumerateArray().ToList()
            : new List<JsonElement>();
    }

    private static List<string> Travelable(RunState state)
    {
        using JsonDocument doc = JsonDocument.Parse(Observation.Capture(state));
        return doc.RootElement.GetProperty("map").GetProperty("travelable")
            .EnumerateArray()
            .Select(e => e.GetProperty("at").GetString() ?? "?")
            .OrderBy(s => s)
            .ToList();
    }

    private static bool Check(string what, bool ok)
    {
        if (ok)
        {
            _passed++;
            Log.Info($"[selftest] PASS {what}");
        }
        else
        {
            _failed++;
            Log.Error($"[selftest] FAIL {what}");
        }
        return ok;
    }
}
