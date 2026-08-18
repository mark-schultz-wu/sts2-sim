using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Reflection;
using System.Text;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer.Serialization;

namespace Sts2Dump;

/// <summary>
/// Every affliction: what it can be stuck to, and the rules around it.
/// </summary>
/// <remarks>
/// An affliction is a debuff attached to a single card rather than to a
/// creature — the card-side twin of an enchantment, and dumped the same way.
/// What one *does* is <c>OnPlay</c>, <c>AfterApplied</c> and
/// <c>BeforeRemoved</c>, which are code and get ported by hand, along with the
/// <c>Amount</c> each carries, which is combat state and not a table.
///
/// What it can go on is a method rather than a list —
/// <c>CanAfflictCardType</c> takes a <c>CardType</c> and returns a bool — so it
/// is probed rather than read: every card type is offered to every affliction
/// and the answers become rows. That is exact, because the method takes nothing
/// but the type, with no run or card underneath it.
///
/// Unlike <c>EnchantmentModel.CanEnchant</c>, the base <c>CanAfflict</c> has no
/// blanket refusal of card types, so the probe is the whole type rule. What the
/// base *does* add is two conditions on the card rather than its type: an
/// unplayable card is refused unless <c>CanAfflictUnplayableCards</c>, and a
/// card that already has an affliction is refused unless <c>IsStackable</c> and
/// it is the same affliction. Both are columns.
///
/// Presentation is deliberately not dumped: the overlay scene, the hover tips,
/// and the localised title and description.
/// </remarks>
public static class AfflictionDump
{
    /// <summary>
    /// The yes-or-no members, in the order they become columns.
    /// </summary>
    private static readonly string[] Flags =
    {
        nameof(AfflictionModel.IsStackable),
        nameof(AfflictionModel.CanAfflictUnplayableCards),
        nameof(AfflictionModel.HasExtraCardText),
    };

    public static void Run()
    {
        var output = new StringBuilder();

        output.Append(Stamp.Header("affliction"));
        output.Append("# affliction <id> ");
        output.Append(string.Join(" ", Flags.Select(f => "<" + Lower(f) + ">")));
        output.Append(" <refusesSomeCards>\n");
        output.Append("# afflicts <affliction> <cardType>\n");

        List<AfflictionModel> afflictions = AllAfflictions();

        foreach (AfflictionModel affliction in afflictions)
        {
            string id = affliction.Id.Entry;

            var columns = new List<string> { "affliction", id };
            columns.AddRange(Flags.Select(f => Cell(affliction, f)));
            columns.Add(Flag(Overrides(affliction, nameof(AfflictionModel.CanAfflict))));

            output.Append(string.Join(" ", columns));
            output.Append('\n');

            // Every card type, offered to the affliction. `CanAfflictCardType`
            // takes nothing else, so asking it about all of them is the whole
            // answer rather than a sample of one.
            foreach (CardType type in Enum.GetValues(typeof(CardType)).Cast<CardType>())
            {
                if (affliction.CanAfflictCardType(type))
                {
                    output.Append($"afflicts {id} {type}\n");
                }
            }
        }

        output.Append($"# {afflictions.Count} afflictions\n");

        foreach (string name in Enum.GetNames(typeof(CardType)))
        {
            long value = Convert.ToInt64(Enum.Parse(typeof(CardType), name));
            output.Append($"enum CardType {value.ToString(CultureInfo.InvariantCulture)} {name}\n");
        }

        Log.Info("[afflictiondump] begin\n" + output);
        Log.Info("[afflictiondump] end");
    }

    private static string Flag(bool b) => b ? "1" : "0";

    private static string Lower(string member) =>
        char.ToLowerInvariant(member[0]) + member.Substring(1);

    /// <summary>
    /// One column: the value, or <c>dynamic</c> where the affliction works it
    /// out. Every one of these is <c>virtual</c> with a constant default.
    /// </summary>
    private static string Cell(AfflictionModel affliction, string member)
    {
        MethodInfo? method = Il.Getter(affliction.GetType(), member)
            ?? throw new InvalidOperationException(
                $"AfflictionModel.{member} is gone; this dump needs rewriting");

        if (!Il.Constant(method, out int decoded))
        {
            return "dynamic";
        }

        object? read = method.Invoke(affliction, Array.Empty<object>());
        if (read is not bool b)
        {
            throw new InvalidOperationException(
                $"{affliction.Id.Entry}.{member} is a {read?.GetType().Name ?? "null"}, not a flag");
        }

        if (Convert.ToInt64(read, CultureInfo.InvariantCulture) != decoded)
        {
            throw new InvalidOperationException(
                $"{affliction.Id.Entry}.{member}: the IL and the model disagree");
        }

        return b ? "1" : "0";
    }

    /// <summary>
    /// Whether an affliction replaces a member with a base implementation.
    /// </summary>
    private static bool Overrides(AfflictionModel affliction, string member)
    {
        MethodInfo? method = affliction
            .GetType()
            .GetMethods(BindingFlags.Public | BindingFlags.Instance)
            .FirstOrDefault(m => m.Name == member);

        return method != null && method.DeclaringType != typeof(AfflictionModel);
    }

    private static List<AfflictionModel> AllAfflictions()
    {
        MethodInfo get = typeof(ModelDb).GetMethod(
            "Get",
            BindingFlags.Static | BindingFlags.NonPublic,
            binder: null,
            types: new[] { typeof(Type) },
            modifiers: null)
            ?? throw new InvalidOperationException("ModelDb.Get(Type) is gone");

        var afflictions = new List<AfflictionModel>();
        foreach (Type t in ModelDb.AllAbstractModelSubtypes)
        {
            if (!t.IsSubclassOf(typeof(AfflictionModel)) || t.IsAbstract)
            {
                continue;
            }

            try
            {
                afflictions.Add((AfflictionModel)get.Invoke(null, new object[] { t })!);
            }
            catch (Exception)
            {
                // Not registered under an id.
            }
        }

        return afflictions.OrderBy(a => a.Id.Entry, StringComparer.Ordinal).ToList();
    }
}
