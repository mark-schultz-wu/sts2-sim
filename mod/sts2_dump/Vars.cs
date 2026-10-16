using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Text;
using MegaCrit.Sts2.Core.Localization.DynamicVars;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.ValueProps;

namespace Sts2Dump;

/// <summary>
/// A model's <c>DynamicVars</c>, as <c>var</c> rows: the numbers its
/// <c>CanonicalVars</c> hold before a run touches them.
/// </summary>
internal static class Vars
{
    /// <summary>The header line naming the columns.</summary>
    public static string Header(string model) => $"# var <{model}> <name> <base> <calculated> <class>\n";

    /// <summary>
    /// One row per variable of model <paramref name="id"/>, ordered by name.
    /// </summary>
    public static void Append(StringBuilder output, string id, DynamicVarSet vars)
    {
        foreach (KeyValuePair<string, DynamicVar> entry in vars.OrderBy(v => v.Key, StringComparer.Ordinal))
        {
            output.Append(string.Join(" ", new[]
            {
                "var",
                id,
                entry.Key,
                entry.Value.BaseValue.ToString(CultureInfo.InvariantCulture),
                entry.Value is CalculatedVar ? "1" : "0",
                Class(entry.Value),
            }));
            output.Append('\n');
        }
    }

    /// <summary>
    /// The <c>DynamicVar</c> subclass this number is, by its own name.
    /// </summary>
    /// <remarks>
    /// The class is what decides the number's rules, and the key it is filed
    /// under says nothing: <c>SovereignBlade</c>'s block is a
    /// <c>CalculatedBlockVar</c> named <c>CalculatedBlock</c>, and its preview
    /// runs the block hooks exactly as a <c>BlockVar</c> named <c>Block</c>
    /// does.
    ///
    /// A generic var prints without the arity C# spells it with, so
    /// <c>PowerVar`1</c> is dumped as <c>PowerVar</c>.
    ///
    /// A <c>PowerVar</c> is dumped with the power it applies on its class, as
    /// <c>PowerVar+STRANGLE_POWER</c>. Its preview asks
    /// <c>Hook.ModifyPowerAmountGiven</c> about that power, and the key it is
    /// filed under need not name it: Mad Science's Choking is
    /// <c>ChokingDamage</c>.
    /// </remarks>
    public static string Class(DynamicVar var)
    {
        string name = var.GetType().Name;
        int arity = name.IndexOf('`');
        name = arity < 0 ? name : name.Substring(0, arity);
        if (var is CalculatedDamageVar { IsFromOsty: true })
        {
            name += "+FromOsty";
        }
        if (var is DamageVar damage && damage.Props.HasFlag(ValueProp.Unpowered))
        {
            name += "+Unpowered";
        }
        if (var is BlockVar block && block.Props.HasFlag(ValueProp.Unpowered))
        {
            name += "+Unpowered";
        }
        Type type = var.GetType();
        if (type.IsGenericType && type.GetGenericTypeDefinition() == typeof(PowerVar<>))
        {
            name += "+" + ModelDb.GetEntry(type.GetGenericArguments()[0]);
        }
        return name;
    }
}
