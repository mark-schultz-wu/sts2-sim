using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Reflection;
using System.Text;
using HarmonyLib;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Creatures;
using MegaCrit.Sts2.Core.Extensions;
using MegaCrit.Sts2.Core.GameActions.Multiplayer;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Random;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Observe;

/// <summary>
/// The game's own calls, method by method, when a run asks for them.
/// </summary>
/// <remarks>
/// <c>STS2_HEADLESS_TRACE</c> is a comma-separated list of patterns. Each is a
/// type's name, short or full, or <c>Type.Method</c>, and a trailing <c>*</c>
/// matches any rest. Every call of a matching method goes to
/// <c>STS2_HEADLESS_TRACE_FILE</c>, with its arguments. Each protocol command
/// writes a <c>&gt;</c> line first, so a call reads against the move that made
/// it. <c>cargo xtask game-trace</c> sets both and reads the file back.
///
/// Off unless the variable is set, so a recorded transcript is the same either
/// way.
///
/// <c>STS2_HEADLESS_COVERAGE</c> names a file. Every method of the game's
/// rules code is counted as it is entered, and the counts go to that file as
/// the game quits: one line for each method, the ones never entered too.
/// <c>cargo xtask hunt --coverage</c> sets it for each run, and
/// <c>cargo xtask game-coverage</c> adds the files up.
///
/// A prefix on an <c>async</c> method runs when the call starts, before its
/// first await. That is the order the calls are made in, which is what a
/// trace is for.
/// </remarks>
public static class Trace
{
    /// <summary>
    /// The namespaces under <c>MegaCrit.Sts2.Core</c> that hold the game's
    /// rules. Screens, saves, audio, the network and the menus are left out.
    /// </summary>
    private static readonly string[] Rules =
    {
        "Models", "Commands", "Combat", "Entities", "GameActions", "Hooks", "MonsterMoves",
        "Rooms", "Rewards", "Runs", "Events", "Random", "ValueProps", "CardSelection", "Map",
        "Odds", "Extensions", "Helpers", "Context", "Unlocks",
    };

    private static StreamWriter? _out;
    private static string? _coverage;
    private static readonly Dictionary<MethodBase, int> Index = new();
    private static readonly List<MethodBase> Counted = new();
    private static int[] _counts = Array.Empty<int>();

    /// <summary>
    /// Patches the methods the patterns name. Called once, once the game has
    /// loaded, before the first command.
    /// </summary>
    public static void Arm(Harmony harmony)
    {
        string? patterns = Environment.GetEnvironmentVariable("STS2_HEADLESS_TRACE");
        _coverage = Environment.GetEnvironmentVariable("STS2_HEADLESS_COVERAGE");
        if (patterns == null && _coverage == null)
        {
            return;
        }
        string[] wanted = (patterns ?? "").Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
        if (patterns != null)
        {
            string path = Environment.GetEnvironmentVariable("STS2_HEADLESS_TRACE_FILE") ?? "trace.log";
            _out = new StreamWriter(path, append: false, new UTF8Encoding(false)) { AutoFlush = true };
        }

        HarmonyMethod Prefix(string name) =>
            new(typeof(Trace).GetMethod(name, BindingFlags.NonPublic | BindingFlags.Static));
        HarmonyMethod traced = Prefix(nameof(Called)), counted = Prefix(nameof(Entered));
        HarmonyMethod both = Prefix(nameof(CalledAndEntered));
        int patched = 0;
        foreach (MethodBase method in Methods())
        {
            string type = method.DeclaringType!.FullName!;
            bool trace = wanted.Any(pattern => Matches(pattern, type, method.Name));
            bool count = _coverage != null && IsRules(type);
            if (!trace && !count)
            {
                continue;
            }
            if (count)
            {
                Index[method] = Counted.Count;
                Counted.Add(method);
            }
            try
            {
                harmony.Patch(method, prefix: trace && count ? both : trace ? traced : counted);
                patched++;
            }
            catch (Exception e)
            {
                // A warning, not an error: an error stops the run, and a trace
                // with one method missing still says what it says.
                Log.Warn($"[trace] cannot patch {type}.{method.Name}: {e.Message}");
            }
        }
        _counts = new int[Counted.Count];
        _out?.WriteLine($"# {patched} methods for {patterns}");
        Log.Info($"[trace] {patched} methods patched, {Counted.Count} counted");
    }

    private static bool IsRules(string type)
    {
        const string root = "MegaCrit.Sts2.Core.";
        if (!type.StartsWith(root, StringComparison.Ordinal))
        {
            return false;
        }
        string rest = type.Substring(root.Length);
        return Rules.Any(ns => rest.StartsWith(ns + ".", StringComparison.Ordinal) || rest == ns);
    }

    private static void Entered(MethodBase __originalMethod)
    {
        if (Index.TryGetValue(__originalMethod, out int at))
        {
            _counts[at]++;
        }
    }

    private static void CalledAndEntered(MethodBase __originalMethod, object? __instance, object?[] __args)
    {
        Entered(__originalMethod);
        Called(__originalMethod, __instance, __args);
    }

    /// <summary>
    /// Writes the counts, where coverage is on. Called as the game quits.
    /// </summary>
    public static void Finish()
    {
        _out?.Flush();
        if (_coverage == null)
        {
            return;
        }
        using var file = new StreamWriter(_coverage, append: false, new UTF8Encoding(false));
        for (int at = 0; at < Counted.Count; at++)
        {
            MethodBase method = Counted[at];
            string parameters = string.Join(",", method.GetParameters().Select(p => p.ParameterType.Name));
            file.WriteLine($"{_counts[at]}\t{method.DeclaringType!.FullName}.{method.Name}({parameters})");
        }
    }

    /// <summary>
    /// Every method of the game's assembly with a body Harmony can patch: no
    /// abstract one, no open generic, nothing the compiler made.
    /// </summary>
    private static IEnumerable<MethodBase> Methods()
    {
        const BindingFlags all = BindingFlags.DeclaredOnly | BindingFlags.Public | BindingFlags.NonPublic
            | BindingFlags.Instance | BindingFlags.Static;
        foreach (Type type in typeof(AbstractModel).Assembly.GetTypes())
        {
            if (type.FullName == null || type.FullName.Contains('<') || type.ContainsGenericParameters)
            {
                continue;
            }
            IEnumerable<MethodBase> methods = type.GetMethods(all).Cast<MethodBase>()
                .Concat(type.GetConstructors(all).Where(ctor => !ctor.IsStatic));
            foreach (MethodBase method in methods)
            {
                if (method.IsAbstract || method.ContainsGenericParameters || method.Name.Contains('<')
                    || method.GetMethodBody() == null)
                {
                    continue;
                }
                yield return method;
            }
        }
    }

    /// <summary>
    /// A pattern against a method: <c>Type</c> or <c>Type.Method</c>, by the
    /// type's short or full name, where a trailing <c>*</c> matches any rest.
    /// </summary>
    private static bool Matches(string pattern, string type, string method)
    {
        string shortType = type.Substring(type.LastIndexOf('.') + 1);
        foreach (string name in new[] { type, shortType })
        {
            if (Like(pattern, name) || Like(pattern, name + "." + method))
            {
                return true;
            }
        }
        return false;
    }

    private static bool Like(string pattern, string text) =>
        pattern.EndsWith('*')
            ? text.StartsWith(pattern.Substring(0, pattern.Length - 1), StringComparison.Ordinal)
            : text == pattern;

    /// <summary>A protocol command, which the calls after it belong to.</summary>
    public static void Command(string line)
    {
        _out?.WriteLine("> " + line);
    }

    private static void Called(MethodBase __originalMethod, object? __instance, object?[] __args)
    {
        // A prefix that throws takes the game's method down with it.
        try
        {
            Write(__originalMethod, __instance, __args);
        }
        catch (Exception e)
        {
            _out!.WriteLine($"{__originalMethod.Name}: {e.GetType().Name} while tracing");
        }
    }

    private static void Write(MethodBase __originalMethod, object? __instance, object?[] __args)
    {
        var line = new StringBuilder();
        line.Append(__originalMethod.DeclaringType!.Name).Append('.').Append(__originalMethod.Name).Append('(');
        if (__instance != null)
        {
            line.Append("this=").Append(Show(__instance));
            if (__args.Length > 0)
            {
                line.Append(", ");
            }
        }
        line.AppendJoin(", ", __args.Select(Show));
        line.Append(')');
        _out!.WriteLine(line.ToString());
    }

    /// <summary>
    /// A random stream by the name the run's or the player's set keeps it
    /// under, and how far it was drawn before this call. A stream neither set
    /// holds, such as one a monster or an event builds, is <c>rng</c>.
    /// </summary>
    private static string ShowRng(Rng rng)
    {
        int counter = Traverse.Create(rng).Field("_counter").GetValue<int>();
        RunState? run = RunManager.Instance.DebugOnlyGetState();
        string? name = Named(run == null ? null : Traverse.Create(run.Rng).Field("_rngs").GetValue(), rng);
        foreach (var player in run?.Players ?? Enumerable.Empty<MegaCrit.Sts2.Core.Entities.Players.Player>())
        {
            name ??= Named(Traverse.Create(player.PlayerRng).Field("_rngs").GetValue(), rng);
        }
        return $"{name ?? "rng"}#{counter}";
    }

    private static string? Named(object? set, Rng rng)
    {
        if (set is not IDictionary streams)
        {
            return null;
        }
        foreach (DictionaryEntry entry in streams)
        {
            if (ReferenceEquals(entry.Value, rng))
            {
                return entry.Key.ToString();
            }
        }
        return null;
    }

    /// <summary>
    /// A card by its id, then the game's own numbers for which card it is.
    /// <c>#n</c> is its <c>NetCombatCardDb</c> id in a fight. <c>(deck n)</c>
    /// is the <c>NetDeckCard.DeckIndex</c> of the deck card it is, or was
    /// copied from for the fight. Like cards differ only in these.
    /// </summary>
    private static string ShowCard(CardModel card)
    {
        if (!card.IsMutable)
        {
            return card.Id.Entry;
        }
        var text = new StringBuilder(card.Id.Entry);
        if (NetCombatCardDb.Instance.TryGetCardId(card, out uint id))
        {
            text.Append('#').Append(id);
        }
        CardModel deckCard = card.DeckVersion ?? card;
        CardPile? deck = deckCard.Pile;
        if (deck?.Type == PileType.Deck)
        {
            text.Append("(deck ").Append(deck.Cards.IndexOf(deckCard)).Append(')');
        }
        return text.ToString();
    }

    /// <summary>
    /// An argument, short: a power by its id, amount and owner, a card as
    /// <see cref="ShowCard"/> shows it, any other
    /// model by its id, a creature by its combat id and name, a collection by
    /// its length, any other list by its type, and anything else by its own
    /// text.
    /// </summary>
    private static string Show(object? value) => value switch
    {
        null => "null",
        PowerModel power => $"{power.Id.Entry}[{power.Amount}]@{power.Owner?.CombatId}",
        Rng rng => ShowRng(rng),
        CardModel card => ShowCard(card),
        AbstractModel model => model.Id.Entry,
        Creature creature => $"#{creature.CombatId} {creature.LogName}",
        string text => '"' + text + '"',
        // Never enumerated: a lazy list can draw from an RNG as it goes.
        ICollection list => $"[{list.Count}]",
        IEnumerable => value.GetType().Name,
        _ => value.ToString() ?? value.GetType().Name,
    };
}
