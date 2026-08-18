using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Reflection;
using System.Text;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer.Serialization;
using MegaCrit.Sts2.Core.Saves.Runs;

namespace Sts2Dump;

/// <summary>
/// Every modifier: whether it helps or hurts, what it rules out, and the two
/// things it changes about setting a run up.
/// </summary>
/// <remarks>
/// A modifier is a run-wide rule chosen before the run starts. Almost all of
/// what one <em>does</em> is hooks — <c>ModifyGeneratedMap</c>,
/// <c>TryModifyRewardsLate</c>, <c>ModifyRestSiteHealAmount</c> — and hooks are
/// code, ported by hand. Which hooks a modifier overrides is deliberately not
/// dumped: a simulator's ported Midas already knows it hooks rewards, so a list
/// of names would describe the game's API surface rather than the game.
///
/// What is table is what a caller has to know <em>before</em> the ported code
/// runs.
///
/// <c>GoodModifiers</c> and <c>BadModifiers</c> are the two lists a run offers
/// from, and they are the whole of the classification: a modifier in neither is
/// one no run can pick, which in this build is the deprecated one.
///
/// <c>MutuallyExclusiveModifiers</c> is a list of sets, and a set is dumped
/// sorted rather than in iteration order — it is a <c>HashSet</c> on the other
/// side, so its order is not a promise and a dump that recorded it would churn.
///
/// <c>ClearsPlayerDeck</c> happens in <c>OnRunCreated</c>, before anything the
/// player does, so a simulator building a run has to know it without running a
/// hook. Overriding <c>GenerateNeowOption</c> means the modifier puts an option
/// in front of the player at Neow, which is likewise a fact about how the run
/// begins.
///
/// One modifier carries a parameter: <c>CharacterCards</c> has a
/// <c>[SavedProperty] ModelId CharacterModel</c>, so which character it names is
/// run state and not content. The column says there is one; it cannot say what
/// it is, because with no run in progress there is no answer.
///
/// Presentation is deliberately not dumped: icons, titles, descriptions and the
/// Neow option's text.
/// </remarks>
public static class ModifierDump
{
    public static void Run()
    {
        var output = new StringBuilder();

        output.Append(Stamp.Header("modifier"));
        output.Append("# modifier <id> <kind> <clearsDeck> <offersNeowOption> <takesParameter>\n");
        output.Append("# exclusive <group> <modifier>\n");

        List<ModifierModel> modifiers = AllModifiers();
        List<string> good = ModelDb.GoodModifiers.Select(m => m.Id.Entry).ToList();
        List<string> bad = ModelDb.BadModifiers.Select(m => m.Id.Entry).ToList();

        foreach (ModifierModel modifier in modifiers)
        {
            string id = modifier.Id.Entry;

            string kind = (good.Contains(id), bad.Contains(id)) switch
            {
                (true, false) => "good",
                (false, true) => "bad",
                (false, false) => "unused",
                (true, true) => throw new InvalidOperationException(
                    $"{id} is both a good and a bad modifier; this dump needs rewriting"),
            };

            output.Append(string.Join(" ", new[]
            {
                "modifier",
                id,
                kind,
                Cell(modifier, nameof(ModifierModel.ClearsPlayerDeck)),
                Flag(Overrides(modifier, nameof(ModifierModel.GenerateNeowOption))),
                Flag(TakesParameter(modifier)),
            }));
            output.Append('\n');
        }

        // Sorted within a group, because the game holds each one in a `HashSet`
        // and iteration order is not something to record.
        int group = 0;
        foreach (IReadOnlySet<ModifierModel> exclusive in ModelDb.MutuallyExclusiveModifiers)
        {
            foreach (string id in exclusive
                .Select(m => m.Id.Entry)
                .OrderBy(id => id, StringComparer.Ordinal))
            {
                output.Append($"exclusive {group.ToString(CultureInfo.InvariantCulture)} {id}\n");
            }
            group++;
        }

        output.Append(
            $"# {modifiers.Count} modifiers, {good.Count} good, {bad.Count} bad, "
            + $"{group.ToString(CultureInfo.InvariantCulture)} exclusive groups\n");

        Log.Info("[modifierdump] begin\n" + output);
        Log.Info("[modifierdump] end");
    }

    private static string Flag(bool b) => b ? "1" : "0";

    /// <summary>
    /// One column: the value, or <c>dynamic</c> where the modifier works it out.
    /// </summary>
    private static string Cell(ModifierModel modifier, string member)
    {
        MethodInfo? method = Il.Getter(modifier.GetType(), member)
            ?? throw new InvalidOperationException(
                $"ModifierModel.{member} is gone; this dump needs rewriting");

        if (!Il.Constant(method, out int decoded))
        {
            return "dynamic";
        }

        object? read = method.Invoke(modifier, Array.Empty<object>());
        if (read is not bool b)
        {
            throw new InvalidOperationException(
                $"{modifier.Id.Entry}.{member} is a {read?.GetType().Name ?? "null"}, not a flag");
        }

        if (Convert.ToInt64(read, CultureInfo.InvariantCulture) != decoded)
        {
            throw new InvalidOperationException(
                $"{modifier.Id.Entry}.{member}: the IL and the model disagree");
        }

        return b ? "1" : "0";
    }

    /// <summary>
    /// Whether a modifier replaces a member with a base implementation.
    /// </summary>
    private static bool Overrides(ModifierModel modifier, string member)
    {
        MethodInfo? method = modifier
            .GetType()
            .GetMethods(BindingFlags.Public | BindingFlags.Instance)
            .FirstOrDefault(m => m.Name == member);

        return method != null && method.DeclaringType != typeof(ModifierModel);
    }

    /// <summary>
    /// Whether a modifier carries run state of its own.
    /// </summary>
    /// <remarks>
    /// <c>[SavedProperty]</c> is what the save system writes and reads back, so
    /// a modifier that declares one is parameterised: the same modifier means
    /// different things in two runs. Read as an attribute rather than by name,
    /// so a second parameterised modifier is found without editing this.
    /// </remarks>
    private static bool TakesParameter(ModifierModel modifier)
    {
        return modifier
            .GetType()
            .GetProperties(BindingFlags.Public | BindingFlags.Instance | BindingFlags.DeclaredOnly)
            .Any(p => p.GetCustomAttribute<SavedPropertyAttribute>() != null);
    }

    private static List<ModifierModel> AllModifiers()
    {
        MethodInfo get = typeof(ModelDb).GetMethod(
            "Get",
            BindingFlags.Static | BindingFlags.NonPublic,
            binder: null,
            types: new[] { typeof(Type) },
            modifiers: null)
            ?? throw new InvalidOperationException("ModelDb.Get(Type) is gone");

        var modifiers = new List<ModifierModel>();
        foreach (Type t in ModelDb.AllAbstractModelSubtypes)
        {
            if (!t.IsSubclassOf(typeof(ModifierModel)) || t.IsAbstract)
            {
                continue;
            }

            try
            {
                modifiers.Add((ModifierModel)get.Invoke(null, new object[] { t })!);
            }
            catch (Exception)
            {
                // Not registered under an id.
            }
        }

        return modifiers.OrderBy(m => m.Id.Entry, StringComparer.Ordinal).ToList();
    }
}
