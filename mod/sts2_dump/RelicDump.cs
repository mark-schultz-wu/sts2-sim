using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Reflection;
using System.Text;
using MegaCrit.Sts2.Core.Entities.Relics;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer.Serialization;

namespace Sts2Dump;

/// <summary>
/// Every relic's static data, and which pool it is drafted from.
/// </summary>
/// <remarks>
/// A relic is a permanent passive, and almost all of what one *does* is hooks —
/// <c>AfterCombatEnd</c>, <c>BeforeCardPlayed</c>, <c>ModifyMaxEnergy</c> — so
/// almost all of it is ported by hand. What is table is where it comes from,
/// what it is worth, and the handful of yes-or-no facts the engine reads off it
/// without asking.
///
/// Two of those need care. <c>Rarity</c> is <c>abstract</c>, so every relic
/// overrides it and "does it override" says nothing — <see cref="Il"/> reads
/// the IL instead. <c>MerchantCost</c> derives from <c>Rarity</c> in the base
/// and nine relics override it, so it is swept at every ascension rather than
/// assumed: an override owes the base formula nothing.
///
/// <c>IsAllowed</c> and <c>IsAllowedAtNeow</c> are dumped as whether the relic
/// has them at all. They take run state, so they are code and get ported; but
/// which relics carry a condition on being offered is a fact about the build,
/// and one a simulator needs before it can roll a reward.
///
/// Presentation is deliberately not dumped: icons, outlines, flash sfx, hover
/// tips, and the localised title and flavour.
/// </remarks>
public static class RelicDump
{
    /// <summary>
    /// The yes-or-no members, in the order they become columns.
    /// </summary>
    private static readonly string[] Flags =
    {
        nameof(RelicModel.IsUsedUp),
        nameof(RelicModel.HasUponPickupEffect),
        nameof(RelicModel.SpawnsPets),
        nameof(RelicModel.IsStackable),
        nameof(RelicModel.AddsPet),
        nameof(RelicModel.IsAllowedInShops),
        nameof(RelicModel.ShowCounter),
    };

    public static void Run()
    {
        var output = new StringBuilder();

        output.Append($"# relic data, contentHash {ModelIdSerializationCache.Hash}\n");
        output.Append("# pool <pool> <relic>\n");
        output.Append("# relic <id> <rarity> <merchantCost> ");
        output.Append(string.Join(" ", Flags.Select(f => "<" + Lower(f) + ">")));
        output.Append(" <displayAmount> <gatedInRun> <gatedAtNeow>\n");
        output.Append("# scalecost <relic> <ascension> <merchantCost>\n");
        output.Append("# rows above are ascension 0; a scale* row is the level at which a value changes\n");

        List<RelicModel> relics = AllRelics();

        // Which pool drafts a relic. A relic in none is one only an event or a
        // character's starting kit ever hands out.
        foreach (RelicPoolModel pool in ModelDb.AllRelicPools)
        {
            foreach (string relic in pool.AllRelicIds
                .Select(id => id.Entry)
                .OrderBy(id => id, StringComparer.Ordinal))
            {
                output.Append($"pool {pool.Id.Entry} {relic}\n");
            }
        }

        // `MerchantCost` is `Rarity` in the base and an override in nine
        // relics, and an override owes the base formula nothing — so it is read
        // at every level rather than once. See `Ascension`.
        var cost = new int[Ascension.Levels.Length][];
        foreach (int level in Ascension.Levels)
        {
            Ascension.SetLevel(level);
            cost[level] = relics.Select(r => r.MerchantCost).ToArray();
        }

        Ascension.SetLevel(0);

        var scaling = new StringBuilder();
        for (int i = 0; i < relics.Count; i++)
        {
            RelicModel relic = relics[i];
            string id = relic.Id.Entry;

            var columns = new List<string>
            {
                "relic",
                id,
                Cell(relic, nameof(RelicModel.Rarity)),
                cost[0][i].ToString(CultureInfo.InvariantCulture),
            };
            columns.AddRange(Flags.Select(f => Cell(relic, f)));
            columns.Add(Cell(relic, nameof(RelicModel.DisplayAmount)));
            columns.Add(Flag(Overrides(relic, nameof(RelicModel.IsAllowed))));
            columns.Add(Flag(Overrides(relic, nameof(RelicModel.IsAllowedAtNeow))));

            output.Append(string.Join(" ", columns));
            output.Append('\n');

            for (int level = 1; level < cost.Length; level++)
            {
                if (cost[level][i] != cost[level - 1][i])
                {
                    scaling.Append($"scalecost {id} {level} {cost[level][i]}\n");
                }
            }
        }

        output.Append(scaling);
        output.Append($"# {relics.Count} relics\n");

        foreach (string name in Enum.GetNames(typeof(RelicRarity)))
        {
            long value = Convert.ToInt64(Enum.Parse(typeof(RelicRarity), name));
            output.Append($"enum RelicRarity {value.ToString(CultureInfo.InvariantCulture)} {name}\n");
        }

        Log.Info("[relicdump] begin\n" + output);
        Log.Info("[relicdump] end");
    }

    private static string Flag(bool b) => b ? "1" : "0";

    private static string Lower(string member) =>
        char.ToLowerInvariant(member[0]) + member.Substring(1);

    /// <summary>
    /// One column: the value, or <c>dynamic</c> where the relic works it out.
    /// </summary>
    /// <remarks>
    /// The constant is decoded from the IL and then read back off the model, so
    /// the two have to agree. A relic whose value is dynamic is never invoked
    /// at all, which is what keeps this from tripping over a member that wants
    /// a run underneath it.
    /// </remarks>
    private static string Cell(RelicModel relic, string member)
    {
        MethodInfo? method = Il.Getter(relic.GetType(), member)
            ?? throw new InvalidOperationException(
                $"RelicModel.{member} is gone; this dump needs rewriting against whatever replaced it");

        if (!Il.Constant(method, out int decoded))
        {
            return "dynamic";
        }

        object? read = method.Invoke(relic, Array.Empty<object>());
        string value = read switch
        {
            bool b => b ? "1" : "0",
            int n => n.ToString(CultureInfo.InvariantCulture),
            Enum e => e.ToString(),
            _ => throw new InvalidOperationException(
                $"{relic.Id.Entry}.{member} is a {read?.GetType().Name ?? "null"}, which this dump has no column for"),
        };

        long actual = Convert.ToInt64(read, CultureInfo.InvariantCulture);
        if (actual != decoded)
        {
            throw new InvalidOperationException(
                $"{relic.Id.Entry}.{member}: the IL says {decoded} and the model says {actual}");
        }

        return value;
    }

    /// <summary>
    /// Whether a relic replaces a member that has a base implementation.
    /// </summary>
    /// <remarks>
    /// This is <c>CardDump</c>'s rule, and it is the right one here:
    /// <c>IsAllowed</c> and <c>IsAllowedAtNeow</c> both return true on
    /// <c>RelicModel</c>, so overriding one is the relic saying it has a
    /// condition. What the condition *is* takes run state and gets ported by
    /// hand; that it exists is what the table records.
    /// </remarks>
    private static bool Overrides(RelicModel relic, string member)
    {
        MethodInfo? method = relic
            .GetType()
            .GetMethods(BindingFlags.Public | BindingFlags.Instance)
            .FirstOrDefault(m => m.Name == member);

        return method != null && method.DeclaringType != typeof(RelicModel);
    }

    /// <summary>
    /// Every relic in the build, not just the ones a pool drafts.
    /// </summary>
    /// <remarks>
    /// <c>ModelDb.AllRelics</c> walks the pools and the characters' starting
    /// kits, which is the same shape as <c>ModelDb.Monsters</c> and misses
    /// things the same way: a relic only an event hands out is in no pool. So
    /// this reflects over model subtypes, as the monster and encounter dumps
    /// do and for the reason they learned to.
    /// </remarks>
    private static List<RelicModel> AllRelics()
    {
        MethodInfo get = typeof(ModelDb).GetMethod(
            "Get",
            BindingFlags.Static | BindingFlags.NonPublic,
            binder: null,
            types: new[] { typeof(Type) },
            modifiers: null)
            ?? throw new InvalidOperationException("ModelDb.Get(Type) is gone");

        var relics = new List<RelicModel>();
        foreach (Type t in ModelDb.AllAbstractModelSubtypes)
        {
            if (!t.IsSubclassOf(typeof(RelicModel)) || t.IsAbstract)
            {
                continue;
            }

            try
            {
                relics.Add((RelicModel)get.Invoke(null, new object[] { t })!);
            }
            catch (Exception)
            {
                // Not registered under an id — nothing a pool, an event or a
                // character could name, so nothing the simulator needs.
            }
        }

        return relics.OrderBy(r => r.Id.Entry, StringComparer.Ordinal).ToList();
    }
}
