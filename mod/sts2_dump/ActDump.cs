using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Reflection;
using System.Text;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer.Serialization;

namespace Sts2Dump;

/// <summary>
/// Every act: how long it is, and which bosses it reveals in what order.
/// </summary>
/// <remarks>
/// The smallest dump and the one the others already point at. Monsters,
/// encounters and events all say which act they belong to; nothing until now
/// said what an act is.
///
/// What is table is the shape of the act: its position in a run, how many rooms
/// it is, how many of its first fights come from the weak pool, and the order
/// its bosses are discovered in. What is not is <c>GetMapPointTypes</c>, which
/// takes an <c>Rng</c> and rolls the map's composition — that is the map
/// generator, and it is ported by hand.
///
/// <c>AllEvents</c>, <c>AllAncients</c> and <c>GenerateAllEncounters</c> are not
/// dumped here either. They are the same relations the event and encounter
/// dumps already write from the other side, and printing them twice would be
/// two sources for one fact.
///
/// Five acts, not the four <c>ModelDb.Acts</c> lists: the deprecated one is
/// reflected in like every other dump's placeholders, and `IsDefault` and the
/// run order tell it apart from content.
/// </remarks>
public static class ActDump
{
    public static void Run()
    {
        var output = new StringBuilder();

        output.Append(Stamp.Header("act"));
        output.Append("# act <id> <index> <isDefault> <rooms> <weakEncounters>\n");
        output.Append("# boss <act> <order> <encounter>\n");

        List<ActModel> acts = AllActs();

        foreach (ActModel act in acts)
        {
            string id = act.Id.Entry;

            output.Append(string.Join(" ", new[]
            {
                "act",
                id,
                act.Index.ToString(CultureInfo.InvariantCulture),
                act.IsDefault ? "1" : "0",
                Number(act, "BaseNumberOfRooms"),
                Number(act, "NumberOfWeakEncounters"),
            }));
            output.Append('\n');

            // Order is the whole point: `BossDiscoveryOrder` is the sequence a
            // player meets an act's bosses in across runs, so a reordering
            // changes which boss a first run ends against.
            int order = 0;
            foreach (EncounterModel boss in act.BossDiscoveryOrder)
            {
                output.Append($"boss {id} {order} {boss.Id.Entry}\n");
                order++;
            }
        }

        output.Append($"# {acts.Count} acts\n");

        Log.Info("[actdump] begin\n" + output);
        Log.Info("[actdump] end");
    }

    /// <summary>
    /// One numeric member, whether the game spells it public or protected.
    /// </summary>
    /// <remarks>
    /// <c>BaseNumberOfRooms</c> and <c>NumberOfWeakEncounters</c> are both
    /// <c>protected</c>, because nothing outside an act needs them — nothing
    /// except a simulator rebuilding the map.
    /// </remarks>
    private static string Number(ActModel act, string member)
    {
        object? read = act
            .GetType()
            .GetProperty(
                member,
                BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.Public)
            ?.GetValue(act)
            ?? throw new InvalidOperationException(
                $"ActModel.{member} is gone; this dump needs rewriting");

        return Convert.ToInt32(read, CultureInfo.InvariantCulture)
            .ToString(CultureInfo.InvariantCulture);
    }

    /// <summary>
    /// Every act in the build, ordered by the position a run visits them in.
    /// </summary>
    private static List<ActModel> AllActs()
    {
        MethodInfo get = typeof(ModelDb).GetMethod(
            "Get",
            BindingFlags.Static | BindingFlags.NonPublic,
            binder: null,
            types: new[] { typeof(Type) },
            modifiers: null)
            ?? throw new InvalidOperationException("ModelDb.Get(Type) is gone");

        var acts = new List<ActModel>();
        foreach (Type t in ModelDb.AllAbstractModelSubtypes)
        {
            if (!t.IsSubclassOf(typeof(ActModel)) || t.IsAbstract)
            {
                continue;
            }

            try
            {
                acts.Add((ActModel)get.Invoke(null, new object[] { t })!);
            }
            catch (Exception)
            {
                // Not registered under an id.
            }
        }

        // By `Index`, because that is the order a run visits them and the order
        // every other dump's act rows were written in.
        return acts
            .OrderBy(a => a.Index)
            .ThenBy(a => a.Id.Entry, StringComparer.Ordinal)
            .ToList();
    }
}
