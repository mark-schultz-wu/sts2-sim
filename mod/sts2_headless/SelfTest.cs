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
using MegaCrit.Sts2.Core.Models.Afflictions;
using MegaCrit.Sts2.Core.Models.Cards;
using MegaCrit.Sts2.Core.Models.Modifiers;
using MegaCrit.Sts2.Core.Models.Powers;
using MegaCrit.Sts2.Core.Models.Relics;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Headless;

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

    public static async Task Run(RunState state)
    {
        Dictionary<string, List<string>> matrix = Discover();
        int total = matrix.Values.Sum(v => v.Count);
        Log.Info($"[selftest] discovered {total} legality-hook overrides across {matrix.Count} hooks");

        var covered = new HashSet<string>();

        // Before anything grants free travel, since this case needs both halves.
        // It leaves the relic behind and stands on the last row, so the rest of
        // the suite gets a clean run rather than inheriting either.
        await BossSurvivesFreeTravel(state);
        state = await Driver.Restart();

        // Out of combat first: map legality needs a map position, not a fight.
        covered.Add("WingedBoots");
        await FreeTravelWidensTheMap(state, "WingedBoots", async () =>
            await RelicCmd.Obtain(ModelDb.Relic<WingedBoots>().ToMutable(), state.Players[0]));
        // Each in-combat case gets its own fight. The first version shared one,
        // and the cases interfered: `Enthralled` left its card in hand, which
        // forbids playing anything else, so the later cases found no playable
        // card to afflict and failed for a reason that had nothing to do with
        // the rule under test. Order-dependent tests are worse than missing
        // ones, because they fail in ways that read as product bugs.
        covered.Add("Enthralled");
        await Driver.EnterFight(state);
        await BlockedByHookIsReported(state);

        covered.Add("SmoggyPower");
        await Driver.EnterFight(state);
        await AfflictedCardIsBlocked<SmoggyPower, Smog>(state, "SmoggyPower");

        covered.Add("ChainsOfBindingPower");
        await Driver.EnterFight(state);
        await AfflictedCardIsBlocked<ChainsOfBindingPower, Bound>(state, "ChainsOfBindingPower", afflict: 2, playAfflictedFirst: true);

        covered.Add("RingingPower");
        await Driver.EnterFight(state);
        await AfflictedCardIsBlocked<RingingPower, Ringing>(state, "RingingPower", playAnyFirst: true);

        covered.Add("SlothPower");
        await Driver.EnterFight(state);
        await BlocksAfterNPlays(state, "SlothPower", plays: 1, arrange: async me =>
            await PowerCmd.Apply<SlothPower>(new BlockingPlayerChoiceContext(), me.Creature,
                amount: 1m, applier: null, cardSource: null, silent: true));

        covered.Add("Normality");
        await Driver.EnterFight(state);
        await BlocksAfterNPlays(state, "Normality", plays: 3, arrange: async me =>
        {
            CardModel normality = ModelDb.Card<Normality>().ToMutable();
            normality.Owner = me;
            await CardPileCmd.Add(normality, PileType.Hand.GetPile(me));
        });

        covered.Add("VelvetChoker");
        await Driver.EnterFight(state);
        await BlocksAfterNPlays(state, "VelvetChoker", plays: 6, arrange: async me =>
            await RelicCmd.Obtain(ModelDb.Relic<VelvetChoker>().ToMutable(), me));

        // Last, because it discards the run every case above is standing in.
        covered.Add("Flight");
        await FlightWidensTheMap();

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

    /// <summary>
    /// The boss must stay reachable from the last row, free travel or not.
    /// </summary>
    /// <remarks>
    /// The two map rules compose badly, which is why this is its own case rather
    /// than a variant of the one below. <c>BossMapPoint</c> sits at row
    /// <c>GetRowCount()</c> — outside the grid — so from the last row, free
    /// travel's <c>GetPointsInRow(row + 1)</c> returns *nothing*, while the
    /// ordinary path finds the boss among <c>Children</c>. An oracle built on
    /// <c>MapTravel</c> alone therefore reports no legal move at all on the floor
    /// before the boss, but only for runs holding `Winged Boots` or `Flight`.
    ///
    /// Two conditions, each individually handled, that are wrong only together:
    /// exactly the shape that survives ordinary testing.
    /// </remarks>
    private static async Task<bool> BossSurvivesFreeTravel(RunState state)
    {
        ActMap map = state.Map;
        MapPoint? spot = map.GetPointsInRow(map.GetRowCount() - 1).OrderBy(p => p.coord.col).FirstOrDefault();
        if (spot == null)
        {
            return Check("boss: the last grid row has a point", false);
        }

        string boss = $"{map.BossMapPoint.coord.col},{map.BossMapPoint.coord.row}";
        state.AddVisitedMapCoord(spot.coord);

        List<string> before = Travelable(state);
        Check($"boss: from the last row the only move is the boss (got {string.Join(" ", before)})",
            before.Count == 1 && before[0] == boss);

        await RelicCmd.Obtain(ModelDb.Relic<WingedBoots>().ToMutable(), state.Players[0]);
        Check("boss: free travel is live", Hook.ShouldAllowFreeTravel(state));

        List<string> after = Travelable(state);
        if (!after.SequenceEqual(before))
        {
            Log.Error($"[selftest]   before: [{string.Join(" ", before)}] after: [{string.Join(" ", after)}]");
        }

        return Check($"boss: free travel does not erase it ({after.Count} node(s))", after.SequenceEqual(before));
    }

    /// <summary>Turning free travel on must widen the reported travelable set.</summary>
    /// <remarks>
    /// Winged Boots is granted through <c>RelicCmd.Obtain</c> rather than by
    /// adding to <c>Player.Relics</c>, because the load path skips
    /// <c>AfterObtained</c> and would produce a state the game never produces.
    /// </remarks>
    private static async Task<bool> FreeTravelWidensTheMap(RunState state, string label, Func<Task> grant)
    {
        MapPoint? spot = Discriminating(state);
        if (spot == null)
        {
            return Check($"{label}: a map position exists where free travel is observable", false);
        }

        state.AddVisitedMapCoord(spot.coord);
        Check($"{label}: free travel is off to begin with", !Hook.ShouldAllowFreeTravel(state));
        List<string> before = Travelable(state);

        await grant();

        List<string> after = Travelable(state);
        Check($"{label}: free travel hook is live", Hook.ShouldAllowFreeTravel(state));

        return Widened(label, before, after);
    }

    /// <summary>
    /// The `Flight` run modifier must widen the travelable set the same way.
    /// </summary>
    /// <remarks>
    /// Unlike every other case here, this rule cannot be switched on mid-run:
    /// modifiers are fixed when the run is created (see
    /// <c>Driver.Restart</c>). So the metamorphic pair is two runs at the
    /// same seed rather than two moments in one, and the comparison is only
    /// meaningful if the runs are otherwise identical — which is exactly R1's
    /// determinism claim, asserted here on the map before it is relied on.
    ///
    /// That makes this case do double duty. It is also the only exercise the
    /// oracle's multi-scenario claim gets: if the second run's map matched the
    /// first's after a <c>CleanUp</c>, no map or RNG state leaked across the
    /// boundary.
    /// </remarks>
    private static async Task<bool> FlightWidensTheMap()
    {
        RunState plain = await Driver.Restart();
        MapPoint? spot = Discriminating(plain);
        if (spot == null)
        {
            return Check("Flight: a map position exists where free travel is observable", false);
        }

        string map = Fingerprint(plain);
        plain.AddVisitedMapCoord(spot.coord);
        Check("Flight: free travel is off without the modifier", !Hook.ShouldAllowFreeTravel(plain));
        List<string> before = Travelable(plain);

        RunState flying = await Driver.Restart(ModelDb.Modifier<Flight>().ToMutable());

        // Without this the case could "prove" that Flight widens the map when
        // all it did was generate a different one.
        if (!Check("Flight: the restarted run generates the same map", Fingerprint(flying) == map))
        {
            return false;
        }

        flying.AddVisitedMapCoord(spot.coord);
        List<string> after = Travelable(flying);
        Check("Flight: free travel hook is live", Hook.ShouldAllowFreeTravel(flying));

        return Widened("Flight", before, after);
    }

    /// <summary>
    /// A map position where free travel and normal travel differ.
    /// </summary>
    /// <remarks>
    /// At floor 1 the next row held exactly the current node's two children, so
    /// the two rules coincide and the test proved nothing while appearing to
    /// run. A scenario that cannot distinguish the rules is not evidence about
    /// them, so the position is searched for and its absence is a failure.
    /// </remarks>
    private static MapPoint? Discriminating(RunState state) =>
        state.Map.GetAllMapPoints()
            .OrderBy(p => p.coord.row).ThenBy(p => p.coord.col)
            .FirstOrDefault(p => state.Map.GetPointsInRow(p.coord.row + 1).Count() > p.Children.Count);

    /// <summary>
    /// The specific claim, not merely "something changed": free travel opens the
    /// entire next row, so the new set must be a strict superset.
    /// </summary>
    private static bool Widened(string label, List<string> before, List<string> after)
    {
        bool widened = after.Count > before.Count && before.All(after.Contains);
        if (!widened)
        {
            Log.Error($"[selftest]   before: {string.Join(" ", before)} after: {string.Join(" ", after)}");
        }

        return Check($"{label} widens the travelable set ({before.Count} -> {after.Count})", widened);
    }

    /// <summary>The whole map, edges included, as a comparable string.</summary>
    private static string Fingerprint(RunState state) =>
        string.Join("|", state.Map.GetAllMapPoints()
            .OrderBy(p => p.coord.row).ThenBy(p => p.coord.col)
            .Select(p => $"{p.coord.col},{p.coord.row}:{p.PointType}>"
                + string.Join(",", p.Children.Select(c => $"{c.coord.col},{c.coord.row}").OrderBy(s => s))));

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

    /// <summary>
    /// Powers that forbid playing a card carrying a particular affliction.
    /// </summary>
    /// <remarks>
    /// `Smoggy`, `Ringing` and `Chains of Binding` share a shape: the power sits
    /// on the creature, the prohibition is keyed to an affliction on the card,
    /// and neither half alone does anything. They differ in *when* they bite,
    /// which the first version of this case got wrong by assuming all three
    /// behaved like Smoggy:
    ///
    /// - `Smoggy` blocks on presence alone.
    /// - `Ringing` blocks only once some card has been played this turn.
    /// - `Chains of Binding` blocks only once a *bound* card has been played.
    ///
    /// So the case takes how many cards to afflict and what must be played
    /// first. Applying the power and asserting immediately would have reported
    /// two false failures — which it did, before the source said otherwise.
    /// </remarks>
    private static async Task<bool> AfflictedCardIsBlocked<TPower, TAffliction>(
        RunState state, string label, int afflict = 1, bool playAfflictedFirst = false, bool playAnyFirst = false)
        where TPower : PowerModel
        where TAffliction : AfflictionModel
    {
        Player me = state.Players[0];
        PlayerCombatState? pcs = me.PlayerCombatState;
        if (pcs == null)
        {
            return Check($"{label}: in combat", false);
        }

        List<CardModel> victims = pcs.Hand.Cards.Where(c => c.CanPlay()).Take(afflict).ToList();
        if (victims.Count < afflict)
        {
            return Check($"{label}: {afflict} playable card(s) exist to afflict", false);
        }

        await PowerCmd.Apply<TPower>(
            new BlockingPlayerChoiceContext(), me.Creature, amount: 1m, applier: null, cardSource: null, silent: true);

        foreach (CardModel v in victims)
        {
            await CardCmd.Afflict<TAffliction>(v, 1m);
        }

        string afflictionId = ModelDb.Affliction<TAffliction>().Id.Entry;

        // Only meaningful for the hooks that need a prior play. Smoggy blocks
        // on presence alone, so asserting this of it was simply wrong.
        if (playAfflictedFirst || playAnyFirst)
        {
            Check($"{label}: not blocked before the triggering play", victims.Last().CanPlay());
        }

        if (playAfflictedFirst)
        {
            Check($"{label}: an afflicted card was played",
                await Driver.PlayOneCard(state, c => c.Affliction?.Id.Entry == afflictionId));
        }
        else if (playAnyFirst)
        {
            // Any card, including an afflicted one. RingingPower afflicts the
            // whole hand when applied, so "play something not afflicted" was
            // unsatisfiable — the diagnostic showed all five cards as /RINGING.
            // The first play is permitted regardless; it is the next one the
            // hook forbids.
            bool played = await Driver.PlayOneCard(state);
            if (!played)
            {
                Log.Error($"[selftest]   hand: {string.Join(" | ", pcs.Hand.Cards.Select(c => $"{c.Id.Entry}"
                    + $"{(c.Affliction != null ? "/" + c.Affliction.Id.Entry : "")}"
                    + $" playable={c.CanPlay(out UnplayableReason r, out AbstractModel? _)} reason={r}"))}");
                Log.Error($"[selftest]   energy={pcs.Energy}");
            }
            Check($"{label}: a card was played", played);
        }

        CardModel? survivor = pcs.Hand.Cards.FirstOrDefault(c => c.Affliction?.Id.Entry == afflictionId);
        if (survivor == null)
        {
            return Check($"{label}: an afflicted card remains in hand to check", false);
        }

        bool blocked = !survivor.CanPlay(out UnplayableReason reason, out AbstractModel? _)
            && reason.HasFlag(UnplayableReason.BlockedByHook);

        // And it must be visible in what the oracle reports, not merely true of
        // the model — the observation is the thing under test.
        bool reported = Legal(state).Any(e =>
            (e.GetProperty("card").GetString() ?? "").Contains(afflictionId)
            && !e.GetProperty("playable").GetBoolean()
            && (e.GetProperty("reason").GetString() ?? "").Contains(nameof(UnplayableReason.BlockedByHook)));

        // The blocker names itself, with its category, and it is whichever model
        // the game itself picked — compared against `CanPlay`'s own answer
        // rather than against a guess.
        //
        // Worth knowing: it is *not* the affliction. `IterateHookListeners`
        // yields `creature.Powers` before cards and their afflictions, and
        // `ShouldPlay` returns the first listener that refuses, so an afflicted
        // card is blocked by the power the affliction applied —
        // BOUND reports POWER/CHAINS_OF_BINDING_POWER, not AFFLICTION/BOUND.
        // Which makes `preventer` order-sensitive: a simulator has to walk the
        // listeners in the game's order to name the same one.
        //
        // The category has to travel with it because entries are not unique
        // across categories: BYRDPIP and PAELS_LEGION are each both a MONSTER
        // and a RELIC, and both are hook listeners.
        survivor.CanPlay(out UnplayableReason _, out AbstractModel? blocker);
        string expected = blocker is null ? "null" : Observation.Id(blocker.Id);
        bool named = Legal(state).Any(e =>
            (e.GetProperty("card").GetString() ?? "").Contains(afflictionId)
            && e.GetProperty("preventer").GetString() == expected);
        if (!named)
        {
            Log.Error($"[selftest]   expected {expected}; reported: {string.Join(" | ", Legal(state)
                .Where(e => !e.GetProperty("playable").GetBoolean())
                .Select(e => $"{e.GetProperty("card").GetString()} <- {e.GetProperty("preventer").GetString() ?? "null"}"))}");
        }

        Check($"{label}: afflicted card is blocked by hook", blocked);
        Check($"{label}: the observation reports it blocked", reported);
        return Check($"{label}: the observation names {expected} as the preventer", named);
    }

    /// <summary>
    /// Hooks that forbid play only after N cards have been played this turn.
    /// </summary>
    /// <remarks>
    /// `Sloth`, `Normality` and `Velvet Choker` all gate on a count of cards
    /// played this turn — 1, 3 and 6 respectively — differing only in where the
    /// rule lives (a power, a card in hand, a relic). The count is the whole
    /// mechanism, so they are one case.
    ///
    /// Energy is granted through <c>PlayerCmd.GainEnergy</c> first: Velvet
    /// Choker's threshold of 6 is unreachable on a starting hand's worth of
    /// energy, so without it the scenario could not reach the state it means to
    /// test — the same unsatisfiable-scenario trap the Ringing case fell into.
    /// </remarks>
    private static async Task<bool> BlocksAfterNPlays(
        RunState state, string label, int plays, Func<Player, Task> arrange)
    {
        Player me = state.Players[0];
        PlayerCombatState? pcs = me.PlayerCombatState;
        if (pcs == null)
        {
            return Check($"{label}: in combat", false);
        }

        await arrange(me);
        await PlayerCmd.GainEnergy(30m, me);

        int played = 0;
        for (int i = 0; i < plays; i++)
        {
            // Top the hand up as well as the energy. Velvet Choker needs six
            // plays and a starting hand holds five, so the first attempt ran the
            // hand dry at five with 27 energy to spare — the limit was cards,
            // not cost, which the diagnostic said outright.
            if (pcs.Hand.Cards.Count == 0)
            {
                await CardPileCmd.Draw(new BlockingPlayerChoiceContext(), 5m, me);
            }

            if (!await Driver.PlayOneCard(state))
            {
                break;
            }
            played++;
        }

        if (played < plays)
        {
            Log.Error($"[selftest]   only played {played} of {plays}; energy={pcs.Energy} hand={pcs.Hand.Cards.Count}");
            return Check($"{label}: {plays} cards could be played", false);
        }

        // Something must remain in hand to be forbidden, or the check is vacuous.
        if (pcs.Hand.Cards.Count == 0)
        {
            return Check($"{label}: a card remains in hand to be blocked", false);
        }

        bool blocked = pcs.Hand.Cards.Any(c =>
            !c.CanPlay(out UnplayableReason r, out AbstractModel? _) && r.HasFlag(UnplayableReason.BlockedByHook));

        bool reported = Legal(state).Any(e =>
            !e.GetProperty("playable").GetBoolean()
            && (e.GetProperty("reason").GetString() ?? "").Contains(nameof(UnplayableReason.BlockedByHook)));

        if (!blocked)
        {
            Log.Error($"[selftest]   hand after {played} plays: {string.Join(" | ", pcs.Hand.Cards.Select(c => { c.CanPlay(out UnplayableReason r, out AbstractModel? _); return $"{c.Id.Entry} {r}"; }))}");
        }

        Check($"{label}: blocked after {plays} play(s)", blocked);
        return Check($"{label}: the observation reports it blocked", reported);
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
