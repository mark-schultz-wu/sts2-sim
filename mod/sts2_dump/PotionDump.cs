using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Reflection;
using System.Text;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Potions;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer.Serialization;

namespace Sts2Dump;

/// <summary>
/// Every potion's static data, and which pools stock it.
/// </summary>
/// <remarks>
/// What a potion does is <c>OnUse</c>, which 63 of the 64 override, and that is
/// ported by hand. What is table is where it comes from, when it can be drunk,
/// and what it can be pointed at.
///
/// <c>Rarity</c>, <c>Usage</c> and <c>TargetType</c> are all <c>abstract</c>, so
/// "does it override the base" says nothing about any of them and <see
/// cref="Il"/> reads the IL instead — the same reason the relic dump does.
///
/// There is no price column because <c>PotionModel</c> has no price: unlike a
/// relic, what a merchant charges for a potion is not a fact about the potion.
///
/// Presentation is deliberately not dumped: icons, colours, hover tips and the
/// localised name and description.
/// </remarks>
public static class PotionDump
{
    /// <summary>
    /// The members read off the IL, in the order they become columns.
    /// </summary>
    private static readonly string[] Values =
    {
        nameof(PotionModel.Rarity),
        nameof(PotionModel.Usage),
        nameof(PotionModel.TargetType),
        nameof(PotionModel.CanBeGeneratedInCombat),
        nameof(PotionModel.PassesCustomUsabilityCheck),
    };

    public static void Run()
    {
        var output = new StringBuilder();

        output.Append(Stamp.Header("potion"));
        output.Append("# pool <pool> <index> <potion>\n");
        output.Append("# potion <id> <rarity> <usage> <target> ");
        output.Append("<canBeGeneratedInCombat> <passesCustomUsabilityCheck>\n");

        List<PotionModel> potions = AllPotions();

        // The pool's own enumeration order, not the id order. A potion reward
        // indexes into the pool after filtering by rarity, so sorting here
        // would pick a different potion from the same draw.
        foreach (PotionPoolModel pool in ModelDb.AllPotionPools
            .OrderBy(p => p.Id.Entry, StringComparer.Ordinal))
        {
            int index = 0;
            foreach (PotionModel potion in pool.AllPotions)
            {
                output.Append($"pool {pool.Id.Entry} {index} {potion.Id.Entry}\n");
                index++;
            }
        }

        foreach (PotionModel potion in potions)
        {
            var columns = new List<string> { "potion", potion.Id.Entry };
            columns.AddRange(Values.Select(v => Cell(potion, v)));

            output.Append(string.Join(" ", columns));
            output.Append('\n');
        }

        output.Append($"# {potions.Count} potions\n");

        foreach (Type e in new[] { typeof(PotionRarity), typeof(PotionUsage), typeof(TargetType) })
        {
            foreach (string name in Enum.GetNames(e))
            {
                long value = Convert.ToInt64(Enum.Parse(e, name));
                output.Append($"enum {e.Name} {value.ToString(CultureInfo.InvariantCulture)} {name}\n");
            }
        }

        Log.Info("[potiondump] begin\n" + output);
        Log.Info("[potiondump] end");
    }

    /// <summary>
    /// One column: the value, or <c>dynamic</c> where the potion works it out.
    /// </summary>
    private static string Cell(PotionModel potion, string member)
    {
        MethodInfo? method = Il.Getter(potion.GetType(), member)
            ?? throw new InvalidOperationException(
                $"PotionModel.{member} is gone; this dump needs rewriting against whatever replaced it");

        if (!Il.Constant(method, out int decoded))
        {
            return "dynamic";
        }

        object? read = method.Invoke(potion, Array.Empty<object>());
        string value = read switch
        {
            bool b => b ? "1" : "0",
            Enum e => e.ToString(),
            _ => throw new InvalidOperationException(
                $"{potion.Id.Entry}.{member} is a {read?.GetType().Name ?? "null"}, which this dump has no column for"),
        };

        long actual = Convert.ToInt64(read, CultureInfo.InvariantCulture);
        if (actual != decoded)
        {
            throw new InvalidOperationException(
                $"{potion.Id.Entry}.{member}: the IL says {decoded} and the model says {actual}");
        }

        return value;
    }

    /// <summary>
    /// Every potion in the build, not just the ones a pool stocks.
    /// </summary>
    private static List<PotionModel> AllPotions()
    {
        MethodInfo get = typeof(ModelDb).GetMethod(
            "Get",
            BindingFlags.Static | BindingFlags.NonPublic,
            binder: null,
            types: new[] { typeof(Type) },
            modifiers: null)
            ?? throw new InvalidOperationException("ModelDb.Get(Type) is gone");

        var potions = new List<PotionModel>();
        foreach (Type t in ModelDb.AllAbstractModelSubtypes)
        {
            if (!t.IsSubclassOf(typeof(PotionModel)) || t.IsAbstract)
            {
                continue;
            }

            try
            {
                potions.Add((PotionModel)get.Invoke(null, new object[] { t })!);
            }
            catch (Exception)
            {
                // Not registered under an id.
            }
        }

        return potions.OrderBy(p => p.Id.Entry, StringComparer.Ordinal).ToList();
    }
}
