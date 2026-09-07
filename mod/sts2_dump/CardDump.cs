using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Reflection;
using System.Text;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Localization.DynamicVars;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer.Serialization;

namespace Sts2Dump;

/// <summary>
/// Prints every card's static data, as ground truth for the simulator's
/// generated tables.
/// </summary>
/// <remarks>
/// Static means: true of the card with no run in progress. Not what playing it
/// does — that needs the engine and comes later.
///
/// Two fields are not static for every card, and pretending otherwise is the
/// trap this dump exists to avoid. <c>Type</c> and <c>TargetType</c> are
/// virtual, and a handful of cards compute them from combat state: Shiv and
/// Sovereign Blade widen to <c>AllEnemies</c> when a power is out, Mad Science
/// becomes whatever card it stored. Reading those off a bare model gives
/// "whatever this is with no combat running", which would bake a lie into a
/// permanent table. So overrides are detected by reflection and the cell is
/// marked <c>dynamic</c> instead — the simulator has to ask, not look up.
///
/// Detection is deliberately conservative: any override at all is marked, even
/// a constant one. Marking a constant as dynamic costs one trait method;
/// marking a dynamic as constant costs a divergence nobody can see.
///
/// Keywords come from <c>KeywordSources.Local</c>, not <c>All</c>. <c>All</c>
/// folds in run-level modifiers, so dumping it would bake one run's state into
/// the table.
///
/// A card's variables are the numbers printed on its face: Strike's 6, Falling
/// Star's 8 damage and its two 1s. <c>BaseValue</c> and not
/// <c>PreviewValue</c> — the preview is what the numbers become once
/// Strength, Vulnerable and every other hook has had its say, which is a fact
/// about a fight rather than about a card. A <c>CalculatedVar</c> is marked,
/// because its printed number is a base plus a count taken off live combat
/// state, and a simulator has to run that rule rather than read this.
/// </remarks>
public static class CardDump
{
    public static void Run()
    {
        var output = new StringBuilder();

        // Stamped, so regenerating against a different build fails visibly
        // rather than quietly replacing the table.
        output.Append(Stamp.Header("card"));
        output.Append("# card <id> <pool> <type> <rarity> <target> <cost> <costsX> <starCost> <starCostsX> <maxUpgrade> <keywords>\n");
        output.Append("# var <card> <name> <base> <calculated>\n");

        List<CardModel> cards = ModelDb.AllCards
            .OrderBy(c => c.Id.Entry, StringComparer.Ordinal)
            .ToList();

        foreach (CardModel card in cards)
        {
            Type t = card.GetType();

            string type = Computed(t, nameof(CardModel.Type)) ? "dynamic" : card.Type.ToString();
            string target = Computed(t, nameof(CardModel.TargetType))
                ? "dynamic"
                : card.TargetType.ToString();

            IReadOnlySet<CardKeyword> keywords = card.GetKeywordsWithSources(KeywordSources.Local);
            string keywordList = keywords.Count == 0
                ? "-"
                : string.Join(",", keywords.Select(k => k.ToString()).OrderBy(k => k, StringComparer.Ordinal));

            output.Append(string.Join(" ", new[]
            {
                "card",
                card.Id.Entry,
                card.Pool?.Id.Entry ?? "NONE",
                type,
                card.Rarity.ToString(),
                target,
                card.EnergyCost.Canonical.ToString(CultureInfo.InvariantCulture),
                card.EnergyCost.CostsX ? "1" : "0",
                card.CanonicalStarCost.ToString(CultureInfo.InvariantCulture),
                card.HasStarCostX ? "1" : "0",
                card.MaxUpgradeLevel.ToString(CultureInfo.InvariantCulture),
                keywordList,
            }));
            output.Append('\n');

            foreach (KeyValuePair<string, DynamicVar> entry in card.DynamicVars
                .OrderBy(v => v.Key, StringComparer.Ordinal))
            {
                output.Append(string.Join(" ", new[]
                {
                    "var",
                    card.Id.Entry,
                    entry.Key,
                    entry.Value.BaseValue.ToString(CultureInfo.InvariantCulture),
                    entry.Value is CalculatedVar ? "1" : "0",
                }));
                output.Append('\n');
            }
        }

        output.Append($"# {cards.Count} cards\n");

        // The enums the card fields draw from, in declaration order.
        //
        // Reflected rather than inferred from the values above: a member no
        // card happens to use is still a member, and a generated enum missing
        // one fails to decode the first time content grows into it. Order
        // matters because it is the discriminant.
        foreach (Type e in new[]
        {
            typeof(CardType), typeof(CardRarity), typeof(TargetType), typeof(CardKeyword),
        })
        {
            foreach (string name in Enum.GetNames(e))
            {
                output.Append($"enum {e.Name} {Convert.ToInt64(Enum.Parse(e, name)).ToString(CultureInfo.InvariantCulture)} {name}\n");
            }
        }

        Log.Info("[carddump] begin\n" + output);
        Log.Info("[carddump] end");
    }

    /// <summary>
    /// Whether this card computes the property rather than inheriting
    /// <c>CardModel</c>'s own.
    /// </summary>
    private static bool Computed(Type card, string property)
    {
        MethodInfo? getter = card
            .GetProperty(property, BindingFlags.Public | BindingFlags.Instance)
            ?.GetGetMethod();

        return getter != null && getter.DeclaringType != typeof(CardModel);
    }
}
