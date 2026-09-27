using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Reflection;
using System.Text;
using MegaCrit.Sts2.Core.Entities.Encounters;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer.Serialization;
using MegaCrit.Sts2.Core.Random;
using MegaCrit.Sts2.Core.Rooms;

namespace Sts2Dump;

/// <summary>
/// Every encounter: what room it is, which monsters it can field, and where
/// they stand.
/// </summary>
/// <remarks>
/// An encounter is how a fight starts. The monster dump says what each monster
/// does once it is on the board; this says which monsters are on the board, in
/// which slots, in which acts.
///
/// The interesting question is that <c>GenerateMonsters</c> is a method rather
/// than a list. Most of them return the same fight every time, but fourteen
/// draw from an <c>Rng</c> seeded per floor, and those cannot be a table. The
/// two are told apart by observation rather than by reading: the encounter is
/// generated against a fresh <c>Rng</c> whose <c>Counter</c> is then read, so
/// "did this consume randomness" is answered by the randomness itself. See
/// <see cref="Spawns"/>.
///
/// Presentation is deliberately not dumped: camera scaling and offset, scene
/// paths, custom backgrounds, BGM and ambient SFX, boss map-node art. A
/// simulator that never draws a frame has no use for any of it, and the
/// encounters that override them are exactly the ones with a set piece.
/// </remarks>
public static class EncounterDump
{
    public static void Run()
    {
        var output = new StringBuilder();

        output.Append(Stamp.Header("encounter"));
        output.Append("# act <act> <index> <encounter> -- the act's own order, not sorted\n");
        output.Append("# encounter <id> <roomType> <isWeak> <givesRewards> <isDebug> ");
        output.Append("<minGold> <maxGold> <spawns>\n");
        output.Append("# slot <encounter> <index> <name>\n");
        output.Append("# tag <encounter> <tag>\n");
        output.Append("# possible <encounter> <monster>\n");
        output.Append("# spawn <encounter> <index> <monster> <slot> <opens>\n");
        output.Append("# scalegold <encounter> <ascension> <minGold> <maxGold>\n");
        output.Append("# rows above are ascension 0; a scale* row is the level at which a value changes\n");

        List<EncounterModel> encounters = AllEncounters();

        // Which acts can roll an encounter, in the act's own order.
        //
        // Not sorted, unlike every other list here. `GenerateRooms` fills its
        // grab bags by walking this, and a bag's weights land at the index they
        // were added at, so the order decides which encounter a draw picks. The
        // index column is what carries it, because a reader cannot tell an
        // ordered list from a sorted one by looking.
        foreach (ActModel act in ModelDb.Acts)
        {
            int at = 0;
            foreach (EncounterModel encounter in act.AllEncounters.Where(e => e != null))
            {
                output.Append($"act {act.Id.Entry} {at} {encounter.Id.Entry}\n");
                at++;
            }
        }

        // Gold is the only ascension-dependent number here: `MinGoldReward` and
        // `MaxGoldReward` multiply by `PovertyAscensionGoldMultiplier` once the
        // run is at Poverty. Swept rather than assumed, because two encounters
        // override the properties outright and an override owes that rule
        // nothing.
        var gold = new (int Min, int Max)[Ascension.Levels.Length][];
        foreach (int level in Ascension.Levels)
        {
            Ascension.SetLevel(level);
            gold[level] = encounters.Select(e => (e.MinGoldReward, e.MaxGoldReward)).ToArray();
        }

        Ascension.SetLevel(0);

        var scaling = new StringBuilder();
        for (int i = 0; i < encounters.Count; i++)
        {
            EncounterModel encounter = encounters[i];
            string id = encounter.Id.Entry;
            string spawns = Spawns(encounter, out List<(string Monster, string Slot, string Opens)> spawned);

            output.Append(string.Join(" ", new[]
            {
                "encounter",
                id,
                encounter.RoomType.ToString(),
                Flag(encounter.IsWeak),
                Flag(encounter.ShouldGiveRewards),
                Flag(GameApi.IsDebugEncounter(encounter)),
                gold[0][i].Min.ToString(CultureInfo.InvariantCulture),
                gold[0][i].Max.ToString(CultureInfo.InvariantCulture),
                spawns,
            }));
            output.Append('\n');

            // Order is the whole meaning of a slot list: `GetNextSlot` takes the
            // first slot no living enemy is standing in, so a reordering puts a
            // summon somewhere else.
            IReadOnlyList<string> slots = encounter.Slots;
            for (int s = 0; s < slots.Count; s++)
            {
                output.Append($"slot {id} {s} {slots[s]}\n");
            }

            foreach (EncounterTag tag in encounter.Tags)
            {
                output.Append($"tag {id} {tag}\n");
            }

            // Every monster the encounter could ever field, including the ones
            // only a summon brings in. Sorted, because it is a set.
            foreach (string monster in encounter.AllPossibleMonsters
                .Where(m => m != null)
                .Select(m => m.Id.Entry)
                .Distinct()
                .OrderBy(m => m, StringComparer.Ordinal))
            {
                output.Append($"possible {id} {monster}\n");
            }

            for (int s = 0; s < spawned.Count; s++)
            {
                output.Append($"spawn {id} {s} {spawned[s].Monster} {spawned[s].Slot} {spawned[s].Opens}\n");
            }

            // A row per level at which gold moves, not per level that differs
            // from zero, so what comes out is the thresholds themselves.
            for (int level = 1; level < gold.Length; level++)
            {
                if (gold[level][i] != gold[level - 1][i])
                {
                    scaling.Append(
                        $"scalegold {id} {level} {gold[level][i].Min} {gold[level][i].Max}\n");
                }
            }
        }

        output.Append(scaling);
        output.Append($"# {encounters.Count} encounters\n");

        foreach (Type e in new[] { typeof(RoomType), typeof(EncounterTag) })
        {
            foreach (string name in Enum.GetNames(e))
            {
                long value = Convert.ToInt64(Enum.Parse(e, name));
                output.Append($"enum {e.Name} {value.ToString(CultureInfo.InvariantCulture)} {name}\n");
            }
        }

        Log.Info("[encounterdump] begin\n" + output);
        Log.Info("[encounterdump] end");
    }

    private static string Flag(bool b) => b ? "1" : "0";

    /// <summary>
    /// The fight an encounter sets up: a list, a roll, or somebody else's
    /// choice.
    /// </summary>
    /// <remarks>
    /// <c>GenerateMonsters</c> is abstract, so there is no signature to read the
    /// answer off. Instead the method is run against a fresh <c>Rng</c> and the
    /// <c>Counter</c> is read afterwards: an encounter that drew nothing cannot
    /// have rolled anything, whatever its body looks like. That is exact rather
    /// than a heuristic, which matters because the alternative — deciding from
    /// the IL — has to chase every call the method makes.
    ///
    /// Run several times, with a different seed each time, for the failure the
    /// counter cannot see. <c>FabricatorNormal.AllPossibleMonsters</c> reads two
    /// <c>HashSet</c>s, and a method returning a set's iteration order would be
    /// stable within one process and not across builds. Same counter, different
    /// answer — so a disagreement here is an error rather than a `random`,
    /// because randomness is not what it would mean.
    ///
    /// A third answer, <c>configured</c>, is for an encounter that refuses to
    /// generate at all until something is set on it. Its roster is chosen by
    /// whatever starts the fight rather than by the encounter, so it is not a
    /// table either, and saying so is better than a `fixed` row recording
    /// whichever branch happened to be reachable.
    ///
    /// The generation happens on a mutable clone. That is the game's own route:
    /// <c>GenerateMonstersWithSlots</c> asserts mutability, and the monsters
    /// that come back have <c>ToMutable</c> called on them.
    /// </remarks>
    private static string Spawns(
        EncounterModel encounter,
        out List<(string Monster, string Slot, string Opens)> spawned)
    {
        MethodInfo generate = typeof(EncounterModel).GetMethod(
            "GenerateMonsters",
            BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.Public)
            ?? throw new InvalidOperationException(
                "EncounterModel.GenerateMonsters is gone; this dump needs rewriting");

        FieldInfo rng = typeof(EncounterModel).GetField(
            "_rng",
            BindingFlags.Instance | BindingFlags.NonPublic)
            ?? throw new InvalidOperationException(
                "EncounterModel._rng is gone; this dump can no longer tell a roll from a list");

        spawned = new List<(string, string, string)>();
        string? first = null;
        bool drew = false;

        // Seeds chosen only to differ. Eight of them, because the point is to
        // disagree if anything is going to, not to sample a distribution.
        foreach (uint seed in new uint[] { 0, 1, 2, 7, 99, 1234, 65535, 2147483647 })
        {
            EncounterModel target = encounter.ToMutable();
            var draw = new Rng(seed);
            rng.SetValue(target, draw);

            IReadOnlyList<(MonsterModel Monster, string? Slot)> monsters;
            try
            {
                monsters =
                    (IReadOnlyList<(MonsterModel Monster, string? Slot)>)generate.Invoke(target, null)!;
            }
            catch (TargetInvocationException e) when (e.InnerException != null)
            {
                // The encounter wants something set on it first, which means
                // its roster is neither a list nor a roll: whatever starts it
                // chooses. One encounter does this — the Battleworn Dummy
                // event picks which of three friends turns up — and the Rust
                // side pins that it is still only one.
                spawned = new List<(string, string, string)>();
                return "configured";
            }

            var got = monsters
                .Select(m => (Monster: m.Monster.Id.Entry, Slot: m.Slot ?? "-", Opens: Opens(m.Monster)))
                .ToList();

            string spelled = string.Join(",", got.Select(m => $"{m.Monster}@{m.Slot}"));
            if (first == null)
            {
                first = spelled;
                spawned = got;
            }
            else if (spelled != first && GameApi.Counter(draw) == 0)
            {
                throw new InvalidOperationException(
                    $"{encounter.Id.Entry}: generates a different fight without drawing any randomness "
                    + $"({first} then {spelled}); something unordered is leaking into the roster");
            }

            drew |= GameApi.Counter(draw) != 0;
        }

        if (drew)
        {
            spawned = new List<(string, string, string)>();
            return "random";
        }

        return "fixed";
    }

    /// <summary>
    /// The move a spawned monster opens on, where the encounter wrote something
    /// onto it that changes it, or <c>-</c>.
    /// </summary>
    /// <remarks>
    /// <c>TheKinBoss</c> sets <c>StartsWithDance</c> on one of its followers,
    /// which moves where its graph starts. The graph is built on the monster
    /// the encounter returned, and on a fresh clone of the same model, and the
    /// two starting states compared.
    /// </remarks>
    private static string Opens(MonsterModel monster)
    {
        string? written = Initial(monster);
        string? own = Initial(ModelDb.GetById<MonsterModel>(monster.Id).ToMutable());
        return written == own ? "-" : written ?? "-";
    }

    private static string? Initial(MonsterModel monster)
    {
        MethodInfo generate = monster
            .GetType()
            .GetMethod(
                "GenerateMoveStateMachine",
                BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.Public)
            ?? throw new InvalidOperationException(
                "MonsterModel.GenerateMoveStateMachine is gone; this dump needs rewriting");
        object machine = generate.Invoke(monster, null)!;
        object? initial = machine
            .GetType()
            .GetField("_initialState", BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.Public)
            ?.GetValue(machine);
        return (string?)initial?.GetType().GetProperty("Id")?.GetValue(initial);
    }

    /// <summary>
    /// Every encounter in the build, not just the ones an act can roll.
    /// </summary>
    /// <remarks>
    /// <c>ModelDb.AllEncounters</c> is <c>Acts.SelectMany(a =&gt;
    /// a.AllEncounters)</c>, which is the same shape as <c>ModelDb.Monsters</c>
    /// and misses things the same way: an encounter an event starts rather than
    /// the map is in no act's list. So this reflects over model subtypes, as the
    /// monster dump does and for the reason it learned to.
    /// </remarks>
    private static List<EncounterModel> AllEncounters()
    {
        var encounters = new List<EncounterModel>();
        foreach (Type t in ModelDb.AllAbstractModelSubtypes)
        {
            if (!t.IsSubclassOf(typeof(EncounterModel)) || t.IsAbstract)
            {
                continue;
            }

            try
            {
                encounters.Add((EncounterModel)Model(t));
            }
            catch (Exception)
            {
                // Not registered under an id — nothing an act or an event could
                // name, so nothing the simulator needs.
            }
        }

        return encounters.OrderBy(e => e.Id.Entry, StringComparer.Ordinal).ToList();
    }

    private static AbstractModel Model(Type type)
    {
        MethodInfo get = typeof(ModelDb).GetMethod(
            "Get",
            BindingFlags.Static | BindingFlags.NonPublic,
            binder: null,
            types: new[] { typeof(Type) },
            modifiers: null)
            ?? throw new InvalidOperationException("ModelDb.Get(Type) is gone");

        return (AbstractModel)get.Invoke(null, new object[] { type })!;
    }
}
