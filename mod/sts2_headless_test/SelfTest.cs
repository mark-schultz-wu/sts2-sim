using System;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Text.Json;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Commands;
using MegaCrit.Sts2.Core.DevConsole;
using MegaCrit.Sts2.Core.DevConsole.ConsoleCommands;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.Hooks;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Map;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Rewards;
using MegaCrit.Sts2.Core.Models.Afflictions;
using MegaCrit.Sts2.Core.Models.Cards;
using MegaCrit.Sts2.Core.Models.Modifiers;
using MegaCrit.Sts2.Core.Models.Powers;
using MegaCrit.Sts2.Core.Models.Relics;
using MegaCrit.Sts2.Core.Runs;

using MegaCrit.Sts2.Core.Modding;
using Sts2Headless;

namespace Sts2HeadlessTest;

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
/// H2's stated check — execute commands outside the reported legal set and
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
[ModInitializer(nameof(Init))]
public static class SelfTest
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

    /// <summary>
    /// Entry point. Claims <c>STS2_HEADLESS_MODE=selftest</c>; the driver mod
    /// claims <c>serve</c> and ignores this one.
    /// </summary>
    /// <remarks>
    /// Its own mod so the shipped driver carries no tests. The suite has to run
    /// inside the game — it reflects over the assembly to discover every
    /// legality hook, which is what makes an unlisted hook show up as
    /// <c>UNCOVERED</c> rather than silently untested — so it cannot become an
    /// ordinary out-of-process test.
    /// </remarks>
    public static void Init()
    {
        if (Environment.GetEnvironmentVariable("STS2_HEADLESS_MODE") == "selftest")
        {
            // Not awaited: Init must return so startup can finish populating the
            // model database, which the driver's first stage waits for.
            _ = Driver.Run(Run);
        }
    }

    public static async Task Run(RunState state)
    {
        Dictionary<string, List<string>> matrix = Discover();
        int total = matrix.Values.Sum(v => v.Count);
        Log.Info($"[selftest] discovered {total} legality-hook overrides across {matrix.Count} hooks");

        var covered = new HashSet<string>();

        // Regression guard. These used to be installed inside Protocol.Serve,
        // so this suite -- which runs instead of it -- drove a game with no
        // selector at all, and passed anyway because none of its cases happened
        // to reach a card-selection prompt. The first one that did would have
        // blocked on a UI that does not exist headless.
        Check("a reward selector is installed", RewardsSet.testSelector != null);

        // First, on an untouched run: this is about the shape of what we report,
        // not about anything the other cases arrange.
        await ObservationIsComplete(state);

        await CommandsAnswerOnlyWhenSettled(state);
        state = await Driver.Restart();

        await ACardThatAsksParksAndCanBeAnswered(state);
        state = await Driver.Restart();

        ErrorsAreCapturedAndHandedOverOnce();

        await ARestartInheritsNothing(state);
        state = await Driver.Restart();

        await WinningAFightOffersRewards(state);
        state = await Driver.Restart();

        await AFightCanBePlayedToTheEnd(state);
        state = await Driver.Restart();

        await ADeadPlayerEndsTheRun(state);
        state = await Driver.Restart();

        await ReplayingAScriptGivesTheSameSequence(state);
        state = await Driver.Restart();

        await EveryCharacterCanBeDriven();
        state = await Driver.Restart();

        await TravellingMovesAndRecordsWhereYouWent(state);
        state = await Driver.Restart();

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
        await EnterFight(state);
        await BlockedByHookIsReported(state);

        covered.Add("SmoggyPower");
        await EnterFight(state);
        await AfflictedCardIsBlocked<SmoggyPower, Smog>(state, "SmoggyPower");

        covered.Add("ChainsOfBindingPower");
        await EnterFight(state);
        await AfflictedCardIsBlocked<ChainsOfBindingPower, Bound>(state, "ChainsOfBindingPower", afflict: 2, playAfflictedFirst: true);

        covered.Add("RingingPower");
        await EnterFight(state);
        await AfflictedCardIsBlocked<RingingPower, Ringing>(state, "RingingPower", playAnyFirst: true);

        covered.Add("SlothPower");
        await EnterFight(state);
        await BlocksAfterNPlays(state, "SlothPower", plays: 1, arrange: async me =>
            await PowerCmd.Apply<SlothPower>(new BlockingPlayerChoiceContext(), me.Creature,
                amount: 1m, applier: null, cardSource: null, silent: true));

        covered.Add("Normality");
        await EnterFight(state);
        await BlocksAfterNPlays(state, "Normality", plays: 3, arrange: async me =>
        {
            CardModel normality = ModelDb.Card<Normality>().ToMutable();
            normality.Owner = me;
            await CardPileCmd.Add(normality, PileType.Hand.GetPile(me));
        });

        covered.Add("VelvetChoker");
        await EnterFight(state);
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
    /// <summary>
    /// Every field the observation promises is actually in it (H4).
    /// </summary>
    /// <remarks>
    /// The other thirty-odd assertions here are about legality — whether the
    /// oracle says the right thing about a card. None of them would notice the
    /// observation quietly losing a field, and that is the failure this project
    /// is least able to survive: a simulator defaults what it is not told, the
    /// oracle has the real value, and the two agree because neither knows the
    /// field exists. A divergence that never appears is worse than one that does.
    ///
    /// The key lists are written out rather than derived. Deriving them from the
    /// capture would assert that the code does what the code does. Written down,
    /// removing a field fails here and adding one is a deliberate edit — which
    /// is the only way this test can be evidence rather than decoration.
    /// </remarks>
    private static async Task ObservationIsComplete(RunState state)
    {
        string[] always =
        {
            "act", "alive", "deck", "floor", "gold", "hp", "inCombat",
            "map", "maxHp", "potions", "relics", "rng", "runOver", "victory",
        };
        string[] inCombat =
        {
            "creatures", "discard", "draw", "energy", "exhaust",
            "hand", "legal", "phase", "round", "turn",
        };
        string[] perCreature =
        {
            "alive", "block", "hp", "id", "intent", "intents",
            "maxHp", "model", "powers", "side",
        };

        JsonElement before = Capture(state);
        foreach (string key in always)
        {
            Check($"observation: '{key}' is reported out of combat", before.TryGetProperty(key, out _));
        }
        Check("observation: a room is reported out of combat", before.TryGetProperty("room", out _));
        Check("observation: the map is reported out of combat",
            before.TryGetProperty("map", out JsonElement map) && map.TryGetProperty("travelable", out _));

        await EnterFight(state);
        JsonElement during = Capture(state);
        foreach (string key in always)
        {
            Check($"observation: '{key}' is reported in combat", during.TryGetProperty(key, out _));
        }

        if (!during.TryGetProperty("combat", out JsonElement combat))
        {
            Check("observation: combat is reported in combat", false);
            return;
        }

        foreach (string key in inCombat)
        {
            Check($"observation: combat reports '{key}'", combat.TryGetProperty(key, out _));
        }

        JsonElement creatures = combat.GetProperty("creatures");
        Check("observation: the fight has creatures in it", creatures.GetArrayLength() > 0);
        foreach (string key in perCreature)
        {
            Check($"observation: each creature reports '{key}'",
                creatures.EnumerateArray().All(c => c.TryGetProperty(key, out _)));
        }

        // A monster's intent is the one thing the game never ships itself, so a
        // simulator picking the wrong move is invisible without it.
        Check("observation: a monster reports what it intends",
            creatures.EnumerateArray()
                .Where(c => c.GetProperty("side").GetString() == "Enemy")
                .All(c => c.GetProperty("intent").ValueKind != JsonValueKind.Null));
    }

    /// <summary>
    /// A command answers only once the game has finished reacting (H3).
    /// </summary>
    /// <remarks>
    /// The standing hazard of this codebase, and the one with the worst record:
    /// the README lists four separate bugs where the driver answered before the
    /// work finished — an action not yet picked up, a turn not yet changed,
    /// rewards not yet applied, an event option not yet resolved. Each handed
    /// the client a state its own decision had not reached.
    ///
    /// Asserting <c>ReadyForInput</c> would prove nothing: that is the predicate
    /// <c>Quiesce</c> waits on, so checking it afterwards asks whether a loop
    /// exited when its condition was true. Every one of those four bugs was
    /// <em>that predicate returning true too early</em>, so it cannot be its own
    /// witness.
    ///
    /// This uses a signal the predicate does not consult: take the digest, let
    /// the engine run, take it again. If work was still outstanding the state
    /// moves and the digests differ. A stable digest across a wait is real
    /// evidence the state had settled; an unstable one is proof it had not.
    ///
    /// It can still miss something slower than the wait, which is the honest
    /// limit of testing quiescence from outside.
    /// </remarks>
    private static async Task CommandsAnswerOnlyWhenSettled(RunState state)
    {
        await EnterFight(state);
        await SettledAfter(state, "entering a fight");

        Check("quiescence: a card was available to play", await Driver.PlayOneCard(state));
        await SettledAfter(state, "playing a card");

        // The case that matters. An action's own completion carries a card play
        // most of the way, which is why the two checks above survived a mutation
        // that made the driver stop waiting at all. Ending a turn is different:
        // the queue drains and the phase returns to Play at the moment of issue,
        // so only the turn counter advancing says the next turn has begun.
        Check("quiescence: the turn ended and resolved", await Driver.EndTurn(state));
        await SettledAfter(state, "ending a turn");
    }

    /// <summary>
    /// A card that needs the player to choose parks, and the choice comes back.
    /// </summary>
    /// <remarks>
    /// <c>HeadlessCardSelector</c> and the whole parking path had no coverage at
    /// all. Nothing in this suite played a card that asks a question, so the
    /// selector the driver installs was only ever asserted to exist.
    ///
    /// <c>BurningPact</c> asks: it calls
    /// <c>CardSelectCmd.FromHand(prefs: new CardSelectorPrefs(..., 1))</c>, so
    /// exactly one card, from hand. The game blocks on that selection, which is
    /// what makes it a decision point rather than part of the play.
    ///
    /// Answering goes through <c>Pending.Accept</c> with the same JSON a client
    /// would send, rather than reaching into the selector — the point is that
    /// the protocol's own path works, not that a task can be completed.
    /// </remarks>
    private static async Task ACardThatAsksParksAndCanBeAnswered(RunState state)
    {
        await EnterFight(state);
        await Driver.GrantCard(state, ModelDb.Card<BurningPact>().ToMutable());

        Check("selector: nothing is pending before the card is played", Pending.Current == null);
        Check("selector: the asking card was played",
            await Driver.PlayOneCard(state, c => c is BurningPact));

        Pending? parked = Pending.Current;
        if (parked == null)
        {
            Check("selector: playing it parked a decision", false);
            return;
        }

        Check("selector: the parked decision is a card selection", parked.Kind == "select_cards");

        Dictionary<string, object?> described = parked.Describe();
        Check("selector: it says how many cards it wants",
            described.TryGetValue("min", out object? min) && (int)min! == 1
            && described.TryGetValue("max", out object? max) && (int)max! == 1);
        Check("selector: it offers the hand to choose from",
            described.TryGetValue("options", out object? options)
            && options is System.Collections.ICollection { Count: > 0 });

        // Out of range first: the selector must validate, because whatever it
        // returns is taken as the player's choice and a bad index would silently
        // become a different move rather than an error.
        using (JsonDocument bad = JsonDocument.Parse("{\"cards\":[999]}"))
        {
            Check("selector: an out-of-range choice is refused",
                parked.Accept(bad.RootElement) != null);
        }

        Check("selector: refusing an answer leaves the decision parked", Pending.Current == parked);

        using (JsonDocument good = JsonDocument.Parse("{\"cards\":[0]}"))
        {
            Check("selector: a valid choice is accepted", parked.Accept(good.RootElement) == null);
        }

        Check("selector: answering resolves it", await Driver.WaitForDecisionPoint());
        Check("selector: nothing is pending afterwards", Pending.Current == null);
    }

    /// <summary>
    /// Travelling moves the run, records the move, and changes what is next.
    /// </summary>
    /// <remarks>
    /// The map cases in this file are all about <em>travelability</em> — which
    /// points the rules allow — and none of them travel. So the three fields a
    /// simulator has to reproduce after a move (`at`, `visited`, and the next
    /// legal set) were never observed changing.
    ///
    /// The last of those is the interesting one: a simulator that tracked
    /// position but not the map's shape would pass a test that only checked
    /// `at`, and diverge on the floor after.
    /// </remarks>
    private static async Task TravellingMovesAndRecordsWhereYouWent(RunState state)
    {
        List<string> before = Travelable(state);
        Check($"travel: somewhere to go from the start ({before.Count} point(s))", before.Count > 0);

        JsonElement start = Capture(state);
        string from = start.GetProperty("map").GetProperty("at").GetString() ?? "?";
        int visitedBefore = start.GetProperty("map").GetProperty("visited").GetArrayLength();

        MapPoint target = MapLegality.TravelableFrom(state).First();
        string to = $"{target.coord.col},{target.coord.row}";
        await RunManager.Instance.EnterMapCoord(target.coord);
        await Driver.WaitForDecisionPoint();

        JsonElement after = Capture(state);
        Check($"travel: the run moved ({from} -> {to})",
            after.GetProperty("map").GetProperty("at").GetString() == to);

        List<string> visited = after.GetProperty("map").GetProperty("visited")
            .EnumerateArray().Select(e => e.GetString() ?? "?").ToList();
        Check($"travel: the move was recorded ({visitedBefore} -> {visited.Count} visited)",
            visited.Count == visitedBefore + 1 && visited.Contains(to));
        Check("travel: where we came from is still recorded", visited.Contains(from));

        // The point of a map is that arriving somewhere changes where you can go.
        List<string> next = Travelable(state);
        Check($"travel: the legal set moved with us ({string.Join(" ", before)} -> {string.Join(" ", next)})",
            next.Count > 0 && !next.SequenceEqual(before));
        Check("travel: you cannot go back where you came from", !next.Contains(from));
    }

    /// <summary>
    /// All five characters can be started, fought with, and observed.
    /// </summary>
    /// <remarks>
    /// Everything else in this file is the Ironclad, and the mod's README lists
    /// four of the five characters beyond their opening state as unverified.
    /// That is not a small gap: each has its own card pool, and the Defect has
    /// orbs — a whole region of the observation that was reported as empty for
    /// as long as nobody had driven a Defect run, because the fields existed and
    /// had never been written or read. The gap was reachability, not difficulty.
    ///
    /// The Gorget scenario is what makes five playthroughs affordable: the same
    /// dumb policy survives on any character, so this is about whether the
    /// game's other four halves can be driven at all, not about playing them
    /// well.
    /// </remarks>
    private static async Task EveryCharacterCanBeDriven()
    {
        foreach (string who in new[] { "IRONCLAD", "SILENT", "REGENT", "NECROBINDER", "DEFECT" })
        {
            RunState state = await Driver.Restart(seed: null, character: who, ascension: 0);
            Player me = state.Players[0];

            Check($"{who}: the run started", me.Creature.IsAlive && me.Deck.Cards.Count > 0);

            // Thirty is 120 Plating, still far more than a weak fight can spend,
            // and five characters' worth of grants is the slowest part of this.
            for (int i = 0; i < 30; i++)
            {
                await RelicCmd.Obtain(ModelDb.Relic<Gorget>().ToMutable(), me);
            }

            await EnterFight(state);

            int turns = 0;
            while (CombatManager.Instance.IsInProgress && turns < 40)
            {
                await AnswerAnythingParked();

                while (await Driver.PlayOneCard(state))
                {
                    await AnswerAnythingParked();
                    if (!CombatManager.Instance.IsInProgress)
                    {
                        break;
                    }
                }

                if (!CombatManager.Instance.IsInProgress)
                {
                    break;
                }

                await Driver.EndTurn(state);
                await AnswerAnythingParked();
                turns++;
            }

            // Survival, not victory. Four of the five win with this policy; the
            // Necrobinder plays out the cap without finishing, which says
            // something about playing the first card in hand and nothing about
            // whether the game can be driven. Asserting a win here would be
            // asserting that a deliberately stupid policy is good enough, and
            // would fail on the next character whose deck rewards thinking.
            //
            // That a fight *can* be finished is covered by the single-character
            // playthrough above; this one is about breadth.
            string outcome = CombatManager.Instance.IsInProgress
                ? $"still fighting after {turns} turn(s)"
                : $"won in {turns} turn(s)";
            Check($"{who}: driven through a fight, {outcome}, at {me.Creature.CurrentHp} hp",
                me.Creature.IsAlive);
        }

        // The Defect last, so its orbs can be inspected without another restart.
        RunState defect = await Driver.Restart(seed: null, character: "DEFECT", ascension: 0);
        await EnterFight(defect);

        JsonElement combat = Capture(defect).GetProperty("combat");
        Check("DEFECT: orbs are reported rather than omitted",
            combat.TryGetProperty("orbs", out JsonElement orbs) && orbs.ValueKind != JsonValueKind.Null);
        Check("DEFECT: the orb slots are described",
            orbs.TryGetProperty("capacity", out _) && orbs.TryGetProperty("queue", out _));
    }

    /// <summary>
    /// The same decisions produce the same states, step for step.
    /// </summary>
    /// <remarks>
    /// D1 is the claim everything else rests on, and until now it was checked
    /// one step deep: two runs agreeing on the state at run start, and two
    /// fights agreeing on the state at combat start. A run that agrees at step
    /// one and diverges at step five passes both.
    ///
    /// So this compares the whole sequence. It is also the shape the diff
    /// harness will use — feed both sides the same decisions and compare after
    /// every one — which makes it a rehearsal for the thing the project exists
    /// to do, not only a test of it.
    ///
    /// The distinctness check is what stops it being vacuous. Comparing two
    /// lists of identical digests would pass while proving nothing, and that is
    /// exactly what this would degrade into if the digest ever stopped covering
    /// the parts of the state a decision changes.
    /// </remarks>
    private static async Task ReplayingAScriptGivesTheSameSequence(RunState state)
    {
        List<string> first = await ScriptedFight(state);
        RunState replayed = await Driver.Restart();
        List<string> second = await ScriptedFight(replayed);

        Check($"replay: the script ran the same number of steps ({first.Count} vs {second.Count})",
            first.Count == second.Count && first.Count > 3);

        Check($"replay: the states are distinct step to step ({first.Distinct().Count()} of {first.Count})",
            first.Distinct().Count() > 3);

        int diverged = -1;
        for (int i = 0; i < System.Math.Min(first.Count, second.Count); i++)
        {
            if (first[i] != second[i])
            {
                diverged = i;
                break;
            }
        }

        Check(
            diverged < 0
                ? $"replay: {first.Count} steps agree exactly"
                : $"replay: diverged at step {diverged} ({first[diverged]} vs {second[diverged]})",
            diverged < 0);
    }

    /// <summary>
    /// Answers whatever the game is waiting on, as blandly as possible.
    /// </summary>
    /// <remarks>
    /// The first version of the multi-character playthrough hung on the Silent.
    /// The Ironclad's opening deck asks nothing, so playing the first card every
    /// turn never parks a decision — the moment a character's cards do ask, the
    /// game waits for an answer that a policy of "play cards" cannot give, and
    /// nothing moves again.
    ///
    /// Which is the same lesson the driver already carries: a decision point is
    /// not a failure, it is the game doing its job. Any policy that drives more
    /// than one character has to be able to say something.
    ///
    /// Deliberately the least interesting answer available — the minimum cards,
    /// and decline rewards — so that what is under test stays "can this
    /// character be driven" rather than "is this a good choice".
    /// </remarks>
    private static async Task AnswerAnythingParked()
    {
        while (Pending.Current is { } parked)
        {
            string answer;
            if (parked.Kind == "rewards")
            {
                answer = "{\"skip\":true}";
            }
            else if (parked.Kind == "select_cards")
            {
                Dictionary<string, object?> described = parked.Describe();
                int min = described.TryGetValue("min", out object? m) ? (int)m! : 0;
                int offered = described.TryGetValue("options", out object? o)
                    && o is System.Collections.ICollection c ? c.Count : 0;
                int take = System.Math.Min(min, offered);
                answer = $"{{\"cards\":[{string.Join(",", Enumerable.Range(0, take))}]}}";
            }
            else
            {
                throw new InvalidOperationException(
                    $"selftest has no answer for a '{parked.Kind}' decision");
            }

            using JsonDocument doc = JsonDocument.Parse(answer);
            string? refused = parked.Accept(doc.RootElement);
            if (refused != null)
            {
                throw new InvalidOperationException($"selftest answer refused: {refused}");
            }

            await Driver.WaitForDecisionPoint();
        }
    }

    /// <summary>
    /// A short fight, played by a fixed policy, reporting the state after every
    /// decision.
    /// </summary>
    private static async Task<List<string>> ScriptedFight(RunState state)
    {
        var digests = new List<string>();
        await EnterFight(state);
        digests.Add(Observation.Digest(state));

        for (int turn = 0; turn < 3 && CombatManager.Instance.IsInProgress; turn++)
        {
            while (await Driver.PlayOneCard(state))
            {
                digests.Add(Observation.Digest(state));
                if (!CombatManager.Instance.IsInProgress)
                {
                    return digests;
                }
            }

            await Driver.EndTurn(state);
            digests.Add(Observation.Digest(state));
        }

        return digests;
    }

    /// <summary>
    /// Dying ends the run, and the observation says so and says how.
    /// </summary>
    /// <remarks>
    /// This rule has failed before: a dead player was observed healing at a rest
    /// site, because the driver kept accepting commands after a defeat and every
    /// observation afterwards described a state the real game cannot reach.
    ///
    /// Win and loss are not distinguishable by <c>IsGameOver</c> alone —
    /// <c>RunManager.WinRun</c> calls <c>GuaranteeKillAllPlayers</c>, so both end
    /// with every player dead, and <c>CurrentRoom.IsVictoryRoom</c> is what
    /// separates them. So `victory` is asserted as well as `runOver`: reporting
    /// the run over without saying which is half an answer.
    ///
    /// `victory` is null before the run ends rather than false, because before
    /// that it is not a meaningful question, and a simulator reading `false`
    /// would be reading a claim nobody made.
    /// </remarks>
    private static async Task ADeadPlayerEndsTheRun(RunState state)
    {
        JsonElement before = Capture(state);
        Check("run over: a live run does not claim to be over",
            before.GetProperty("runOver").GetBoolean() == false);
        Check("run over: victory is unanswered while the run is live",
            before.GetProperty("victory").ValueKind == JsonValueKind.Null);
        Check("run over: the player starts alive", before.GetProperty("alive").GetBoolean());

        await EnterFight(state);
        CmdResult died = new DieConsoleCmd().Process(state.Players[0], System.Array.Empty<string>());
        Check($"run over: the player could be killed ({died.msg})", died.success);
        if (died.task != null)
        {
            await died.task;
        }

        await Driver.WaitForDecisionPoint();

        JsonElement after = Capture(state);
        Check("run over: the run reports itself over", after.GetProperty("runOver").GetBoolean());
        Check("run over: it reports a defeat rather than a victory",
            after.GetProperty("victory").ValueKind == JsonValueKind.False);
        Check("run over: the player reports as dead", !after.GetProperty("alive").GetBoolean());
    }

    /// <summary>
    /// A whole fight, played rather than skipped.
    /// </summary>
    /// <remarks>
    /// Every other case here takes one or two decisions. This one takes a
    /// fight's worth, which is the only way to exercise what happens between
    /// them: enemy turns, turn transitions, the draw and discard piles cycling,
    /// and quiescence holding across dozens of decisions rather than two.
    ///
    /// The scenario is what makes it safe to write as a test rather than a
    /// gamble. A hundred Gorgets is 400 Plating at combat start — Plating is a
    /// counter that becomes Block each turn and decrements by one — so the
    /// player cannot lose inside a few hundred turns, and the test can play the
    /// simplest possible policy without the outcome depending on it. That is
    /// scenario construction earning its keep: not reaching a state the game
    /// would produce, but reaching one where the thing under test is the only
    /// variable.
    ///
    /// The policy is deliberately the dumbest available — first playable card,
    /// then end the turn. A test whose policy needs to be good is a test that
    /// fails when the policy is wrong rather than when the driver is.
    /// </remarks>
    private static async Task AFightCanBePlayedToTheEnd(RunState state)
    {
        Player me = state.Players[0];

        // Before entering: Gorget applies its Plating in AfterRoomEntered.
        for (int i = 0; i < 100; i++)
        {
            await RelicCmd.Obtain(ModelDb.Relic<Gorget>().ToMutable(), me);
        }

        Check($"playthrough: the relics were granted ({me.Relics.Count} held)", me.Relics.Count > 50);

        await EnterFight(state);
        int startingHp = me.Creature.CurrentHp;

        int turns = 0;
        const int Cap = 100;
        while (CombatManager.Instance.IsInProgress && turns < Cap)
        {
            while (await Driver.PlayOneCard(state))
            {
                if (!CombatManager.Instance.IsInProgress)
                {
                    break;
                }
            }

            if (!CombatManager.Instance.IsInProgress)
            {
                break;
            }

            await Driver.EndTurn(state);
            turns++;
        }

        Check($"playthrough: the fight ended in {turns} turn(s), inside the cap", turns < Cap);
        Check("playthrough: the fight is over", !CombatManager.Instance.IsInProgress);
        Check("playthrough: the player survived it", me.Creature.IsAlive);
        Check($"playthrough: the scenario held ({me.Creature.CurrentHp}/{startingHp} hp)",
            me.Creature.CurrentHp == startingHp);
    }

    /// <summary>
    /// Winning a fight parks a rewards decision, and it can be declined.
    /// </summary>
    /// <remarks>
    /// <c>RewardChoice</c> and <c>TakeRewards</c> were asserted to be installed
    /// and never exercised, because reaching them means winning a fight and
    /// nothing here could. The console's <c>kill all</c> can, now that
    /// <c>sts2_determinism</c> has made the console usable.
    ///
    /// Worth having beyond the coverage: with <c>TestMode</c> on and no
    /// <c>testSelector</c>, <c>RewardsSet.Offer</c> takes every reward itself.
    /// So the thing under test is not that a reward can be taken, it is that the
    /// game asks instead of helping itself — an oracle that quietly pockets the
    /// rewards is one whose observations a simulator cannot reproduce.
    /// </remarks>
    private static async Task WinningAFightOffersRewards(RunState state)
    {
        await EnterFight(state);

        CmdResult killed = new KillConsoleCmd().Process(state.Players[0], new[] { "all" });
        Check($"rewards: the fight could be won ({killed.msg})", killed.success);
        if (killed.task != null)
        {
            await killed.task;
        }

        await Driver.WaitForDecisionPoint();

        Pending? parked = Pending.Current;
        if (parked == null)
        {
            Check("rewards: winning parked a decision", false);
            return;
        }

        Check("rewards: the parked decision is a reward choice", parked.Kind == "rewards");

        Dictionary<string, object?> described = parked.Describe();
        Check("rewards: it says whether it can be skipped", described.ContainsKey("skippable"));
        Check("rewards: it lists what is on offer",
            described.TryGetValue("rewards", out object? offered)
            && offered is System.Collections.ICollection { Count: > 0 });

        using (JsonDocument nonsense = JsonDocument.Parse("{}"))
        {
            Check("rewards: an answer naming neither take nor skip is refused",
                parked.Accept(nonsense.RootElement) != null);
        }

        using (JsonDocument skip = JsonDocument.Parse("{\"skip\":true}"))
        {
            Check("rewards: declining is accepted", parked.Accept(skip.RootElement) == null);
        }

        await Driver.WaitForDecisionPoint();
        Check("rewards: answering resolves it", Pending.Current == null);
    }

    /// <summary>
    /// A restarted run is the run that was started, not the one that was left.
    /// </summary>
    /// <remarks>
    /// Every case in this file restarts between scenarios and assumes the next
    /// one begins clean. Nothing checked that. It is also the shape of a bug the
    /// suite already had: cases sharing one fight interfered, and the failures
    /// read as product bugs rather than as test bugs. If a restart leaked, every
    /// assertion after the first would be measuring a state nobody described.
    ///
    /// The whole-state digest is the assertion rather than a hand-picked list of
    /// fields, because a hand-picked list only catches leaks somebody already
    /// thought of. Relics, deck, hp, gold, floor, the RNG counters and the map
    /// are all in it.
    /// </remarks>
    private static async Task ARestartInheritsNothing(RunState state)
    {
        string fresh = Observation.Digest(state);

        // Change as many kinds of thing as cheaply as possible: an inventory, a
        // pile, a position, and a fight in progress.
        await RelicCmd.Obtain(ModelDb.Relic<WingedBoots>().ToMutable(), state.Players[0]);
        await EnterFight(state);
        await Driver.GrantCard(state, ModelDb.Card<BurningPact>().ToMutable());
        await Driver.PlayOneCard(state);

        string dirtied = Observation.Digest(state);
        Check("restart: the run was actually changed before restarting", fresh != dirtied);

        RunState restarted = await Driver.Restart();
        Check("restart: a restarted run matches a fresh one",
            Observation.Digest(restarted) == fresh);
    }

    /// <summary>
    /// An error logged at error level reaches the client, once.
    /// </summary>
    /// <remarks>
    /// This tests the plumbing — capture, drain, drain-once — and not a real
    /// swallowed failure, because provoking one turned out to be harder than
    /// expected and the difference is worth writing down.
    ///
    /// The obvious provocation is the bug that motivated <c>GameErrors</c>: a
    /// card added to hand without registering it with the <c>CombatState</c>,
    /// which the README records as making <c>OnPlayWrapper</c> throw, the game
    /// swallow it, and the protocol answer "resolved": true over a state that
    /// had not changed. Tried here, it does not swallow: it throws
    /// <c>InvalidOperationException: BURNING_PACT has no owner</c> straight out
    /// through the driver, which is a loud failure and a different one. So
    /// either that path changed or the original account is imprecise — recorded
    /// as an open question rather than guessed at.
    ///
    /// What this does assert is that an error-level log is captured, is handed
    /// over once, and does not come back a second time. A leak in either
    /// direction is silent: errors that never surface leave a bad observation
    /// looking clean, and errors that surface twice attach a stale failure to an
    /// innocent command.
    /// </remarks>
    private static void ErrorsAreCapturedAndHandedOverOnce()
    {
        GameErrors.Drain();
        Check("gameErrors: nothing is outstanding to begin with", GameErrors.Drain().Count == 0);

        const string Marker = "selftest marker: this is a deliberate error";
        Log.Error(Marker);

        List<string> reported = GameErrors.Drain();
        Check($"gameErrors: an error-level log is captured ({reported.Count} reported)",
            reported.Any(e => e.Contains(Marker)));
        Check("gameErrors: draining again does not report it twice", GameErrors.Drain().Count == 0);
    }

    private static async Task SettledAfter(RunState state, string what)
    {
        string before = Observation.Digest(state);

        // Long enough for a frame-driven continuation to run several times over;
        // the protocol measures a decision at roughly 8ms.
        await Task.Delay(500);

        string after = Observation.Digest(state);
        Check($"quiescence: the state is settled after {what}", before == after);
        if (before != after)
        {
            Log.Error($"[selftest]   digest moved {before} -> {after} with nothing asked of it");
        }
    }

    private static JsonElement Capture(RunState state)
    {
        using var doc = JsonDocument.Parse(Observation.Capture(state));
        return doc.RootElement.Clone();
    }

    /// <summary>
    /// The fight the in-combat cases run in.
    /// </summary>
    /// <remarks>
    /// Named here rather than passed per case: every case wants "a fight", not a
    /// particular one, and naming it once means the suite says which fight it
    /// tested when that starts to matter.
    ///
    /// Throws rather than returning the refusal. A case that silently ran
    /// outside combat would report on rules it never exercised.
    /// </remarks>
    private static async Task EnterFight(RunState state)
    {
        string? refused = await Driver.EnterFight(state, "BOWLBUGS_WEAK");
        if (refused != null)
        {
            throw new InvalidOperationException($"selftest could not enter its fight: {refused}");
        }
    }

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
    /// meaningful if the runs are otherwise identical — which is exactly D1's
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
