using System;
using System.Collections;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Reflection;
using System.Text;
using MegaCrit.Sts2.Core.Entities.Ascension;
using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.MonsterMoves;
using MegaCrit.Sts2.Core.MonsterMoves.Intents;
using MegaCrit.Sts2.Core.MonsterMoves.MonsterMoveStateMachine;
using MegaCrit.Sts2.Core.Multiplayer.Serialization;
using MegaCrit.Sts2.Core.Runs;

namespace Sts2Dump;

/// <summary>
/// Prints every monster's HP range and move state machine, as ground truth for
/// the simulator's generated tables.
/// </summary>
/// <remarks>
/// The state machine is mostly data. <c>RandomBranchState.States</c> is a list
/// of plain fields — target state, <c>MoveRepeatType</c>, repeat limit,
/// cooldown — and only the weight is a delegate. That matters more than it
/// sounds: <c>RandomBranchState.GetNextState</c> sums the weights, takes
/// exactly one <c>rng.NextFloat(max)</c>, and walks the list in declaration
/// order. So the dumped topology fixes how much RNG a monster consumes and in
/// what order, which is the part a divergence would show up in first.
/// <c>ConditionalBranchState</c> draws nothing at all.
///
/// What is not data: the weight lambdas, the conditional predicates, and the
/// move bodies. Those are ported by hand, and this dump exists partly to say
/// exactly how many of them there are.
///
/// A weight is only recorded as constant when it provably cannot change — see
/// <see cref="Constant"/>. Guessing wrong in that direction bakes a number that
/// looks right and is not, which is the failure this whole dump is meant to
/// prevent.
///
/// HP ranges and intent damage go through
/// <c>AscensionHelper.GetValueIfAscension</c>, so they are true of one
/// ascension. Which one is stamped in the header rather than assumed.
/// </remarks>
public static class MonsterDump
{
    public static void Run()
    {
        var output = new StringBuilder();

        output.Append($"# monster data, contentHash {ModelIdSerializationCache.Hash}\n");
        output.Append("# act <act> <monster>\n");
        output.Append("# monster <id> <minHp> <maxHp> <initialState> <stateCount>\n");
        output.Append("# move <monster> <state> <followUp> <mustPerformOnce> <intentCount>\n");
        output.Append("# intent <monster> <state> <index> <type> <count> <damage>\n");
        output.Append("# rand <monster> <state> <branchCount>\n");
        output.Append("# branch <monster> <state> <index> <target> <repeatType> <maxTimes> <cooldown> <weight>\n");
        output.Append("# cond <monster> <state> <branchCount>\n");
        output.Append("# condbranch <monster> <state> <index> <target>\n");
        output.Append("# scalehp <monster> <ascension> <minHp> <maxHp>\n");
        output.Append("# scaleintent <monster> <state> <index> <ascension> <count> <damage>\n");
        output.Append("# scalebranch <monster> <state> <index> <ascension> <weight>\n");
        output.Append("# rows above are ascension 0; a scale* row is the level at which a value changes\n");

        foreach (string name in Enum.GetNames(typeof(AscensionLevel)))
        {
            output.Append($"enum AscensionLevel {Convert.ToInt64(Enum.Parse(typeof(AscensionLevel), name)).ToString(CultureInfo.InvariantCulture)} {name}\n");
        }

        // Which act a monster can be met in, via the encounters that can spawn
        // it. A monster appearing in two acts gets two rows.
        foreach (ActModel act in ModelDb.Acts)
        {
            foreach (MonsterModel monster in act.AllMonsters.OrderBy(m => m.Id.Entry, StringComparer.Ordinal))
            {
                output.Append($"act {act.Id.Entry} {monster.Id.Entry}\n");
            }
        }

        // Every ascension, not one — see `Ascension`. A whole body per level,
        // because what scales is a number inside a row and the rows are what
        // `Scaling` diffs.
        string[] bodies = new string[Ascension.Levels.Length];
        foreach (int level in Ascension.Levels)
        {
            Ascension.SetLevel(level);
            bodies[level] = Body();
        }

        Ascension.SetLevel(0);
        output.Append(bodies[0]);
        output.Append(Scaling(bodies));

        Log.Info("[monsterdump] begin\n" + output);
        Log.Info("[monsterdump] end");
    }

    private static string Body()
    {
        var output = new StringBuilder();

        List<MonsterModel> monsters = AllMonsters();

        int constantWeights = 0;
        int computedWeights = 0;
        int conditions = 0;

        foreach (MonsterModel monster in monsters)
        {
            string id = monster.Id.Entry;

            MonsterMoveStateMachine? machine = null;
            string? failure = null;
            try
            {
                machine = Generate(monster);
            }
            catch (Exception e)
            {
                failure = (e.InnerException ?? e).GetType().Name;
            }

            if (machine == null)
            {
                // Reported rather than skipped: a monster whose graph could not
                // be built is a hole in the table, and a hole nobody can see is
                // the thing to avoid.
                output.Append($"monster {id} {Hp(monster, min: true)} {Hp(monster, min: false)} - 0\n");
                output.Append($"nograph {id} {failure ?? "null"}\n");
                continue;
            }

            // `States` plus the initial state, which is not always in it.
            //
            // `MonsterMoveStateMachine` registers whatever list it was handed,
            // and the initial state is a separate argument. Myte passes a
            // `ConditionalBranchState` as its start without adding it to the
            // list, and the game does not mind: `FindNextMoveState` calls
            // `GetNextState` on the current state directly and only looks the
            // *result* up in `States`. So an unregistered start works as long as
            // nothing transitions back to it — and enumerating `States` alone
            // would silently drop the node that picks Myte's opening move.
            MonsterState? initial = InitialState(machine);
            List<MonsterState> states = machine.States.Values
                .Concat(initial == null || machine.States.ContainsKey(initial.Id)
                    ? Array.Empty<MonsterState>()
                    : new[] { initial })
                .OrderBy(s => s.Id, StringComparer.Ordinal)
                .ToList();

            output.Append(string.Join(" ", new[]
            {
                "monster",
                id,
                Hp(monster, min: true),
                Hp(monster, min: false),
                initial?.Id ?? "-",
                states.Count.ToString(CultureInfo.InvariantCulture),
            }));
            output.Append('\n');

            foreach (MonsterState state in states)
            {
                switch (state)
                {
                    case MoveState move:
                        EmitMove(output, id, move);
                        break;
                    case RandomBranchState branch:
                        EmitRandomBranch(output, id, branch, ref constantWeights, ref computedWeights);
                        break;
                    case ConditionalBranchState conditional:
                        EmitConditionalBranch(output, id, conditional, ref conditions);
                        break;
                    default:
                        output.Append($"unknownstate {id} {state.Id} {state.GetType().Name}\n");
                        break;
                }
            }
        }

        output.Append($"# {monsters.Count} monsters\n");
        output.Append($"# {constantWeights} constant weights, {computedWeights} computed, {conditions} conditions\n");

        // The enums the rows draw from, in declaration order, for the same
        // reason the card dump reflects its own: a member no monster happens to
        // use is still a member, and the order is the discriminant.
        foreach (Type e in new[] { typeof(MoveRepeatType), typeof(IntentType) })
        {
            foreach (string name in Enum.GetNames(e))
            {
                output.Append($"enum {e.Name} {Convert.ToInt64(Enum.Parse(e, name)).ToString(CultureInfo.InvariantCulture)} {name}\n");
            }
        }

        return output.ToString();
    }


    /// <summary>
    /// Where a value moves as ascension rises, as one row per change.
    /// </summary>
    /// <remarks>
    /// The bodies differ only in columns that scale, so a row's other columns
    /// are its key. If the keys themselves differ between levels then ascension
    /// changed a monster's *graph* rather than its numbers, which nothing here
    /// models — so that is reported rather than flattened.
    ///
    /// A row is emitted where a value differs from the level below, not from
    /// level 0, so what comes out is the thresholds themselves.
    /// </remarks>
    private static string Scaling(string[] bodies)
    {
        var output = new StringBuilder();

        // Which columns of which rows can scale. Everything else is structure.
        var scaling = new Dictionary<string, int[]>
        {
            ["monster"] = new[] { 2, 3 },
            ["intent"] = new[] { 5, 6 },
            ["branch"] = new[] { 8 },
        };

        List<string[]> Rows(string body) => body
            .Split('\n')
            .Where(l => l.Length > 0 && !l.StartsWith("#"))
            .Select(l => l.Split(' '))
            .Where(f => scaling.ContainsKey(f[0]))
            .ToList();

        List<string[]> baseRows = Rows(bodies[0]);

        for (int level = 1; level < bodies.Length; level++)
        {
            List<string[]> rows = Rows(bodies[level]);
            List<string[]> below = Rows(bodies[level - 1]);

            if (rows.Count != baseRows.Count)
            {
                output.Append($"structurechange {level} rowcount {baseRows.Count} -> {rows.Count}\n");
                continue;
            }

            for (int i = 0; i < rows.Count; i++)
            {
                string[] row = rows[i];
                int[] columns = scaling[row[0]];

                if (!SameExcept(row, baseRows[i], columns))
                {
                    output.Append($"structurechange {level} {string.Join(" ", row)}\n");
                    continue;
                }

                if (SameAt(row, below[i], columns))
                {
                    continue;
                }

                string values = string.Join(" ", columns.Select(c => row[c]));
                switch (row[0])
                {
                    case "monster":
                        output.Append($"scalehp {row[1]} {level} {values}\n");
                        break;
                    case "intent":
                        output.Append($"scaleintent {row[1]} {row[2]} {row[3]} {level} {values}\n");
                        break;
                    case "branch":
                        output.Append($"scalebranch {row[1]} {row[2]} {row[3]} {level} {values}\n");
                        break;
                }
            }
        }

        return output.ToString();
    }

    private static bool SameExcept(string[] a, string[] b, int[] columns)
    {
        if (a.Length != b.Length)
        {
            return false;
        }

        for (int i = 0; i < a.Length; i++)
        {
            if (!columns.Contains(i) && a[i] != b[i])
            {
                return false;
            }
        }

        return true;
    }

    private static bool SameAt(string[] a, string[] b, int[] columns)
    {
        return columns.All(c => a[c] == b[c]);
    }

    private static void EmitMove(StringBuilder output, string monsterId, MoveState move)
    {
        // `FollowUpState` is set by the model when it wires the graph;
        // `FollowUpStateId` is the init-only spelling for models that name the
        // successor before building it. `GetNextState` prefers the former.
        string followUp = move.FollowUpState?.Id ?? move.FollowUpStateId ?? "-";

        output.Append(string.Join(" ", new[]
        {
            "move",
            monsterId,
            move.Id,
            followUp,
            move.MustPerformOnceBeforeTransitioning ? "1" : "0",
            move.Intents.Count.ToString(CultureInfo.InvariantCulture),
        }));
        output.Append('\n');

        for (int i = 0; i < move.Intents.Count; i++)
        {
            EmitIntent(output, monsterId, move.Id, i, move.Intents[i]);
        }
    }

    /// <summary>
    /// One intent, as the type plus its numbers when they are constant.
    /// </summary>
    /// <remarks>
    /// Intents are what a player sees before a move resolves, so the simulator
    /// needs them to produce a fair observation. They are not what the move
    /// does — that is the move body, ported by hand — and the two are declared
    /// separately in the game, so the numbers here are the displayed ones.
    /// </remarks>
    private static void EmitIntent(StringBuilder output, string monsterId, string stateId, int index, AbstractIntent intent)
    {
        string type;
        try
        {
            type = intent.IntentType.ToString();
        }
        catch (Exception)
        {
            type = "dynamic";
        }

        string damage = "-";

        // "How many", whatever the intent counts: attack repeats for an attack,
        // cards for a status. Defend, Buff, Heal and the rest carry no number
        // at all — their amounts live in the move body, not the intent.
        string count = "-";

        if (intent is StatusIntent status)
        {
            count = status.CardCount.ToString(CultureInfo.InvariantCulture);
        }

        if (intent is AttackIntent attack)
        {
            damage = Value(attack.DamageCalc, () => attack.DamageCalc!().ToString(CultureInfo.InvariantCulture));

            // `MultiAttackIntent.Repeats` invokes a lambda; everything else
            // returns a field. Reading the property blind would evaluate code
            // in a monster with no combat around it.
            if (intent is MultiAttackIntent multi)
            {
                var calc = (Delegate?)Field(multi, "_repeatCalc");
                count = calc == null
                    ? Convert.ToInt32(Field(multi, "_repeat")).ToString(CultureInfo.InvariantCulture)
                    : Value(calc, () => ((Func<int>)calc)().ToString(CultureInfo.InvariantCulture));
            }
            else
            {
                try
                {
                    count = attack.Repeats.ToString(CultureInfo.InvariantCulture);
                }
                catch (Exception)
                {
                    count = "dynamic";
                }
            }
        }

        output.Append($"intent {monsterId} {stateId} {index} {type} {count} {damage}\n");
    }

    private static void EmitRandomBranch(
        StringBuilder output,
        string monsterId,
        RandomBranchState branch,
        ref int constant,
        ref int computed)
    {
        List<RandomBranchState.StateWeight> branches = branch.States;
        output.Append($"rand {monsterId} {branch.Id} {branches.Count}\n");

        // Index, not sorted: `GetNextState` subtracts weights in list order,
        // so which branch a given roll lands on depends on this order.
        for (int i = 0; i < branches.Count; i++)
        {
            RandomBranchState.StateWeight w = branches[i];
            string weight = Value(w.weightLambda, () => w.weightLambda().ToString("R", CultureInfo.InvariantCulture));

            if (weight == "dynamic")
            {
                computed++;
            }
            else
            {
                constant++;
            }

            output.Append(string.Join(" ", new[]
            {
                "branch",
                monsterId,
                branch.Id,
                i.ToString(CultureInfo.InvariantCulture),
                w.stateId,
                w.repeatType.ToString(),
                w.maxTimes.ToString(CultureInfo.InvariantCulture),
                w.cooldown.ToString(CultureInfo.InvariantCulture),
                weight,
            }));
            output.Append('\n');
        }
    }

    /// <summary>
    /// A conditional branch: the first predicate that holds wins, and no
    /// predicate is ever constant enough to record, so only the order and the
    /// targets are dumped.
    /// </summary>
    private static void EmitConditionalBranch(
        StringBuilder output,
        string monsterId,
        ConditionalBranchState branch,
        ref int conditions)
    {
        // `States` is a private property of a private struct type, so both the
        // list and the `id` field come out by reflection.
        var list = (IEnumerable?)Property(branch, "States");
        var entries = list == null ? new List<object>() : list.Cast<object>().ToList();

        output.Append($"cond {monsterId} {branch.Id} {entries.Count}\n");

        for (int i = 0; i < entries.Count; i++)
        {
            conditions++;
            string target = Field(entries[i], "id")?.ToString() ?? "-";
            output.Append($"condbranch {monsterId} {branch.Id} {i} {target}\n");
        }
    }

    /// <summary>
    /// Whether a delegate's result provably cannot change, so its value can be
    /// baked into a permanent table.
    /// </summary>
    /// <remarks>
    /// The obvious test — invoke it and see whether it throws — is wrong, and
    /// wrong in the direction that bakes a lie. <c>TwoTailedRat</c>'s weight is
    /// <c>() =&gt; !CanSummon() ? 1f : 1f/12f</c>; on a fresh model
    /// <c>CanSummon()</c> returns false immediately because
    /// <c>TurnsUntilSummonable</c> starts at 2, so invoking it returns 1f and
    /// throws nothing.
    ///
    /// "Captures nothing" is wrong too, in the other direction:
    /// <c>AddBranch(state, repeatType, 1f)</c> compiles to
    /// <c>() =&gt; weight</c>, a closure over the float parameter, so most of
    /// the genuine constants do capture.
    ///
    /// What separates them is who owns the captured state. A lambda that reads
    /// only locals gets a compiler-generated closure holding copies of them; a
    /// lambda that touches the monster is compiled as a method on the monster,
    /// or gets a closure holding a reference back to it. So: the target must be
    /// compiler-generated, and every field it holds must be a value the closure
    /// owns outright.
    /// </remarks>
    private static bool Constant(Delegate d)
    {
        object? target = d.Target;
        if (target == null)
        {
            return true;
        }

        Type owner = target.GetType();
        if (!owner.IsDefined(typeof(System.Runtime.CompilerServices.CompilerGeneratedAttribute), inherit: false))
        {
            return false;
        }

        foreach (FieldInfo f in owner.GetFields(BindingFlags.Public | BindingFlags.NonPublic | BindingFlags.Instance))
        {
            if (!f.FieldType.IsPrimitive && !f.FieldType.IsEnum && f.FieldType != typeof(decimal))
            {
                return false;
            }
        }

        return true;
    }

    /// <summary>
    /// A delegate's value if it is constant, <c>dynamic</c> if not, <c>-</c> if
    /// there is no delegate at all.
    /// </summary>
    private static string Value(Delegate? d, Func<string> read)
    {
        if (d == null)
        {
            return "-";
        }

        if (!Constant(d))
        {
            return "dynamic";
        }

        try
        {
            return read();
        }
        catch (Exception)
        {
            // Provably constant and still threw: not something to paper over
            // with a number.
            return "dynamic";
        }
    }

    private static string Hp(MonsterModel monster, bool min)
    {
        try
        {
            int hp = min ? monster.MinInitialHp : monster.MaxInitialHp;
            return hp.ToString(CultureInfo.InvariantCulture);
        }
        catch (Exception)
        {
            return "dynamic";
        }
    }

    /// <summary>
    /// Every monster in the build, not just the ones an encounter can roll.
    /// </summary>
    /// <remarks>
    /// <c>ModelDb.Monsters</c> is <c>Acts.SelectMany(a =&gt; a.AllMonsters)</c>,
    /// which walks encounters — so it finds the 102 monsters that can start a
    /// fight and misses every one that only ever arrives mid-combat.
    /// TwoTailedRat's Call For Backup does <c>CreatureCmd.Add&lt;TwoTailedRat&gt;</c>;
    /// other monsters summon things that are in no encounter table at all. A
    /// simulator that cannot build those cannot finish the fight.
    ///
    /// So this reflects over model subtypes the way <c>ModelDb.AllPowers</c>
    /// does, which reaches every registered monster whether or not an encounter
    /// names it.
    /// </remarks>
    private static List<MonsterModel> AllMonsters()
    {
        MethodInfo get = typeof(ModelDb).GetMethod(
            "Get",
            BindingFlags.Static | BindingFlags.NonPublic,
            binder: null,
            types: new[] { typeof(Type) },
            modifiers: null)
            ?? throw new InvalidOperationException("ModelDb.Get(Type) is gone");

        var monsters = new List<MonsterModel>();
        foreach (Type t in ModelDb.AllAbstractModelSubtypes)
        {
            if (!t.IsSubclassOf(typeof(MonsterModel)) || t.IsAbstract)
            {
                continue;
            }

            try
            {
                monsters.Add((MonsterModel)get.Invoke(null, new object[] { t })!);
            }
            catch (Exception)
            {
                // Not registered under an id — nothing an encounter or a summon
                // could ever name, so nothing the simulator needs.
            }
        }

        return monsters.OrderBy(m => m.Id.Entry, StringComparer.Ordinal).ToList();
    }

    /// <summary>
    /// Builds the monster's graph without putting it into combat.
    /// </summary>
    /// <remarks>
    /// <c>GenerateMoveStateMachine</c> is protected, and the public route —
    /// <c>SetUpForCombat</c> — asserts mutability and refuses to run twice. The
    /// game already calls the protected one straight off a canonical model in
    /// <c>MonsterModel.GetIntents</c>, so this is the same thing that path does.
    ///
    /// Nine monsters throw <c>CanonicalModelException</c> from there, because
    /// building their graph writes to the model — <c>TwoTailedRat</c> reads
    /// <c>StarterMoveIndex</c>, others set a field — and a canonical model
    /// refuses writes. <c>ToMutable</c> is the game's own answer to that, so
    /// the graph gets built on a mutable clone.
    /// </remarks>
    private static MonsterMoveStateMachine? Generate(MonsterModel monster)
    {
        MonsterModel target = monster.ToMutable();

        MethodInfo? generate = target
            .GetType()
            .GetMethod(
                "GenerateMoveStateMachine",
                BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.Public);

        return (MonsterMoveStateMachine?)generate?.Invoke(target, null);
    }

    private static MonsterState? InitialState(MonsterMoveStateMachine machine)
    {
        return (MonsterState?)Field(machine, "_initialState");
    }

    private static object? Field(object o, string name)
    {
        return o
            .GetType()
            .GetField(name, BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.Public)
            ?.GetValue(o);
    }

    private static object? Property(object o, string name)
    {
        return o
            .GetType()
            .GetProperty(name, BindingFlags.Instance | BindingFlags.NonPublic | BindingFlags.Public)
            ?.GetValue(o);
    }
}
