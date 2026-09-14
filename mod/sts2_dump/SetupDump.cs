using System;
using System.Collections;
using System.Collections.Generic;
using System.Linq;
using System.Reflection;
using System.Text;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Extensions;
using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Models.Characters;
using MegaCrit.Sts2.Core.Models.RelicPools;
using MegaCrit.Sts2.Core.Entities.Relics;
using MegaCrit.Sts2.Core.Random;
using MegaCrit.Sts2.Core.Rooms;
using MegaCrit.Sts2.Core.Runs;
using MegaCrit.Sts2.Core.Saves.Runs;
using MegaCrit.Sts2.Core.Unlocks;
using Sts2Headless;

namespace Sts2Dump;

/// <summary>
/// What a run's setup draws, phase by phase, and what each phase chose.
/// </summary>
/// <remarks>
/// Starting a run takes 413 draws from the <c>UpFront</c> stream before a
/// player has done anything: the relic bags are shuffled, the shared ancients
/// are dealt out, and every act's rooms are rolled. The observation reports the
/// total and nothing else, so a simulator reproducing 412 of them has no way to
/// find out where it went wrong.
///
/// So each phase is driven here on a stream of its own, and the counter is read
/// afterwards. That turns one number into a dozen, each against a phase small
/// enough to fix.
///
/// Not the game's own call sequence but the same calls in the same order. The
/// order is checked by the total: the observation's <c>UpFront</c> after a real
/// start has to equal what these add up to.
/// </remarks>
public static class SetupDump
{
    /// The seed every phase is measured on, and the run the recording uses.
    private const string Seed = "0RACLE1";

    public static void Run()
    {
        var output = new StringBuilder();
        output.Append(Stamp.Header("run setup"));
        output.Append("# phase <name> <counter> -- where UpFront stands after it\n");
        output.Append("# chose <phase> <id>... -- what it picked, in order\n");
        output.Append($"# seed {Seed}, character REGENT, ascension 0, one player\n");

        // A real run state, built the way the headless driver builds one, so
        // the unlock state and the act list are the run's own rather than this
        // dump's idea of them.
        var players = new List<Player>
        {
            Player.CreateForNewRun(ModelDb.Character<Regent>(), UnlockState.all, 1uL),
        };
        List<ActModel> acts = ActModel.GetDefaultList().Select(a => a.ToMutable()).ToList();
        RunState state = RunState.CreateForNewRun(
            players, acts, Array.Empty<ModifierModel>(), GameMode.Standard, 0,
            SeedHelper.CanonicalizeSeed(Seed));

        // One stream, read after each phase. Cumulative and not per phase: the
        // order is half of what is being measured, and isolated counts cannot
        // show it.
        Rng up = state.Rng.UpFront;
        output.Append($"phase created {GameApi.Counter(up)}\n");

        // `InitializeNewRun`.
        state.SharedRelicGrabBag.Populate(
            ModelDb.RelicPool<SharedRelicPool>().GetUnlockedRelics(state.UnlockState), up);
        output.Append($"phase sharedrelics {GameApi.Counter(up)}\n");
        Deques(output, "sharedrelics", state.SharedRelicGrabBag);

        foreach (Player player in state.Players)
        {
            player.PopulateRelicGrabBagIfNecessary(up);
        }
        output.Append($"phase playerrelics {GameApi.Counter(up)}\n");
        Deques(output, "playerrelics", state.Players[0].RelicGrabBag);

        // `RunManager.GenerateRooms`: the shared ancients dealt out, then every
        // act's rooms.
        output.Append("order sharedancients "
            + Ids(state.UnlockState.SharedAncients.Select(a => a.Id.Entry)) + "\n");
        List<AncientEventModel> pool = state.UnlockState.SharedAncients.ToList().UnstableShuffle(up);
        output.Append($"phase ancientshuffle {GameApi.Counter(up)}\n");
        output.Append("chose ancientshuffle " + Ids(pool.Select(a => a.Id.Entry)) + "\n");

        foreach (ActModel act in state.Acts.Skip(1))
        {
            int count = up.NextInt(pool.Count + 1);
            List<AncientEventModel> taken = pool.Take(count).ToList();
            pool = pool.Except(taken).ToList();
            act.SetSharedAncientSubset(taken);
            output.Append($"chose ancientsplit {act.Id.Entry} {count}\n");
        }
        output.Append($"phase ancientsplit {GameApi.Counter(up)}\n");

        foreach (ActModel act in state.Acts)
        {
            string name = act.Id.Entry;

            // The order the pools come in, before anything shuffles them. The
            // content dumps sort every list by id, which is right for a set and
            // wrong for an input to a shuffle.
            output.Append($"order events.{name} "
                + Ids(act.AllEvents.Concat(ModelDb.AllSharedEvents).Select(e => e.Id.Entry)) + "\n");
            output.Append($"order encounters.{name} "
                + Ids(act.AllEncounters.Select(e => e.Id.Entry)) + "\n");
            output.Append($"order ancients.{name} "
                + Ids(act.GetUnlockedAncients(state.UnlockState).Select(a => a.Id.Entry)) + "\n");

            act.GenerateRooms(up, state.UnlockState, isMultiplayer: false);
            RoomSet rooms = Rooms(act);
            output.Append($"phase rooms.{name} {GameApi.Counter(up)}\n");
            output.Append($"chose normal.{name} " + Ids(rooms.normalEncounters.Select(e => e.Id.Entry)) + "\n");
            output.Append($"chose elite.{name} " + Ids(rooms.eliteEncounters.Select(e => e.Id.Entry)) + "\n");
            output.Append($"chose boss.{name} {rooms.Boss.Id.Entry}\n");
            output.Append($"chose ancient.{name} {rooms.Ancient.Id.Entry}\n");
            output.Append($"chose events.{name} " + Ids(rooms.events.Select(e => e.Id.Entry)) + "\n");
        }

        Log.Info("[setupdump] begin\n" + output);
        Log.Info("[setupdump] end");
    }

    private static string Ids(IEnumerable<string> ids) => string.Join(" ", ids);

    /// <summary>One relic bag's shuffled order, a line per rarity.</summary>
    /// <remarks>
    /// The count of draws says a shuffle happened. Only the order says it
    /// happened the same way, and a simulator that groups the relics
    /// differently lands on the same count with the wrong bag.
    /// </remarks>
    private static void Deques(StringBuilder output, string phase, RelicGrabBag bag)
    {
        FieldInfo field = typeof(RelicGrabBag).GetField(
            "_deques", BindingFlags.Instance | BindingFlags.NonPublic)
            ?? throw new InvalidOperationException("RelicGrabBag has no _deques field any more");
        var deques = (IDictionary)(field.GetValue(bag)
            ?? throw new InvalidOperationException("the bag was not populated"));

        foreach (DictionaryEntry entry in deques)
        {
            var relics = (List<RelicModel>)entry.Value!;
            output.Append($"chose {phase}.{entry.Key} " + Ids(relics.Select(r => r.Id.Entry)) + "\n");
        }
    }

    /// <summary>An act's generated rooms, which it keeps to itself.</summary>
    /// <remarks>
    /// `ActModel._rooms` is protected and every public route off it pulls a
    /// room and marks it visited. Reading the lists is what this dump is for,
    /// so it reaches the field rather than consuming the act to see it.
    /// </remarks>
    private static RoomSet Rooms(ActModel act)
    {
        FieldInfo field = typeof(ActModel).GetField(
            "_rooms", BindingFlags.Instance | BindingFlags.NonPublic)
            ?? throw new InvalidOperationException("ActModel has no _rooms field any more");
        return (RoomSet)(field.GetValue(act)
            ?? throw new InvalidOperationException("the act generated no rooms"));
    }
}
