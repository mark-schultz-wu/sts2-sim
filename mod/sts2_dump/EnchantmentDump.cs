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
/// Every enchantment: what it can go on, and the flags around it.
/// </summary>
/// <remarks>
/// An enchantment is a modifier stuck to a card. What it *does* is
/// <c>EnchantDamageAdditive</c> and friends, which are code and get ported by
/// hand.
///
/// What it can go on is a method rather than a list — <c>CanEnchantCardType</c>
/// takes a <c>CardType</c> and returns a bool — so it is probed rather than
/// read: every card type is offered to every enchantment and the answers become
/// rows. That is exact, because the method takes nothing but the type, with no
/// run or card underneath it.
///
/// Two things are deliberately left out of that. <c>CanEnchant</c> takes a whole
/// <c>CardModel</c> and five enchantments override it, so an enchantment can
/// refuse a card its type allows; that is code, and the dump records only that
/// there is such a rule. And the base <c>CanEnchant</c> refuses Status, Curse
/// and Quest outright for every enchantment, so those never appear here whatever
/// <c>CanEnchantCardType</c> says about them.
///
/// Presentation is deliberately not dumped: the gold and red glows, hover tips,
/// and the localised name and card text.
/// </remarks>
public static class EnchantmentDump
{
    /// <summary>
    /// The yes-or-no members, in the order they become columns.
    /// </summary>
    private static readonly string[] Flags =
    {
        nameof(EnchantmentModel.HasExtraCardText),
        nameof(EnchantmentModel.ShowAmount),
        nameof(EnchantmentModel.ShouldStartAtBottomOfDrawPile),
        nameof(EnchantmentModel.IsStackable),
    };

    public static void Run()
    {
        var output = new StringBuilder();

        output.Append(Stamp.Header("enchantment"));
        output.Append("# enchantment <id> ");
        output.Append(string.Join(" ", Flags.Select(f => "<" + Lower(f) + ">")));
        output.Append(" <refusesSomeCards>\n");
        output.Append("# enchants <enchantment> <cardType>\n");

        List<EnchantmentModel> enchantments = AllEnchantments();

        foreach (EnchantmentModel enchantment in enchantments)
        {
            string id = enchantment.Id.Entry;

            var columns = new List<string> { "enchantment", id };
            columns.AddRange(Flags.Select(f => Cell(enchantment, f)));
            columns.Add(Flag(Overrides(enchantment, nameof(EnchantmentModel.CanEnchant))));

            output.Append(string.Join(" ", columns));
            output.Append('\n');

            // Every card type, offered to the enchantment. `CanEnchantCardType`
            // takes nothing else, so asking it about all of them is the whole
            // answer rather than a sample of one.
            foreach (CardType type in Enum.GetValues(typeof(CardType)).Cast<CardType>())
            {
                if (enchantment.CanEnchantCardType(type))
                {
                    output.Append($"enchants {id} {type}\n");
                }
            }
        }

        output.Append($"# {enchantments.Count} enchantments\n");

        foreach (string name in Enum.GetNames(typeof(CardType)))
        {
            long value = Convert.ToInt64(Enum.Parse(typeof(CardType), name));
            output.Append($"enum CardType {value.ToString(CultureInfo.InvariantCulture)} {name}\n");
        }

        Log.Info("[enchantmentdump] begin\n" + output);
        Log.Info("[enchantmentdump] end");
    }

    private static string Flag(bool b) => b ? "1" : "0";

    private static string Lower(string member) =>
        char.ToLowerInvariant(member[0]) + member.Substring(1);

    /// <summary>
    /// One column: the value, or <c>dynamic</c> where the enchantment works it
    /// out. Every one of these is <c>virtual</c> with a constant default.
    /// </summary>
    private static string Cell(EnchantmentModel enchantment, string member)
    {
        MethodInfo? method = Il.Getter(enchantment.GetType(), member)
            ?? throw new InvalidOperationException(
                $"EnchantmentModel.{member} is gone; this dump needs rewriting");

        if (!Il.Constant(method, out int decoded))
        {
            return "dynamic";
        }

        object? read = method.Invoke(enchantment, Array.Empty<object>());
        if (read is not bool b)
        {
            throw new InvalidOperationException(
                $"{enchantment.Id.Entry}.{member} is a {read?.GetType().Name ?? "null"}, not a flag");
        }

        if (Convert.ToInt64(read, CultureInfo.InvariantCulture) != decoded)
        {
            throw new InvalidOperationException(
                $"{enchantment.Id.Entry}.{member}: the IL and the model disagree");
        }

        return b ? "1" : "0";
    }

    /// <summary>
    /// Whether an enchantment replaces a member with a base implementation.
    /// </summary>
    private static bool Overrides(EnchantmentModel enchantment, string member)
    {
        MethodInfo? method = enchantment
            .GetType()
            .GetMethods(BindingFlags.Public | BindingFlags.Instance)
            .FirstOrDefault(m => m.Name == member);

        return method != null && method.DeclaringType != typeof(EnchantmentModel);
    }

    private static List<EnchantmentModel> AllEnchantments()
    {
        MethodInfo get = typeof(ModelDb).GetMethod(
            "Get",
            BindingFlags.Static | BindingFlags.NonPublic,
            binder: null,
            types: new[] { typeof(Type) },
            modifiers: null)
            ?? throw new InvalidOperationException("ModelDb.Get(Type) is gone");

        var enchantments = new List<EnchantmentModel>();
        foreach (Type t in ModelDb.AllAbstractModelSubtypes)
        {
            if (!t.IsSubclassOf(typeof(EnchantmentModel)) || t.IsAbstract)
            {
                continue;
            }

            try
            {
                enchantments.Add((EnchantmentModel)get.Invoke(null, new object[] { t })!);
            }
            catch (Exception)
            {
                // Not registered under an id.
            }
        }

        return enchantments.OrderBy(e => e.Id.Entry, StringComparer.Ordinal).ToList();
    }
}
