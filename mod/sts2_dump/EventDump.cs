using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Reflection;
using System.Text;
using MegaCrit.Sts2.Core.Events;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer.Serialization;

namespace Sts2Dump;

/// <summary>
/// Every event: which acts offer it, and the shell around its choices.
/// </summary>
/// <remarks>
/// What an event *does* is <c>GenerateInitialOptions</c>, which every one of
/// the 68 overrides and which builds its options from run state — the player's
/// gold, deck and relics. That is code and gets ported by hand. What is table
/// is which acts can offer an event at all, whether it carries a condition, and
/// the handful of facts the room reads before the first page is drawn.
///
/// Two memberships, not one. <c>ModelDb.AllSharedEvents</c> is a hardcoded
/// array of eighteen, while only eight events override <c>IsShared</c> — so
/// "shared" is two different claims and the dump writes both rather than
/// picking. <c>IsDeterministic</c> is <c>!IsShared</c>, so it follows the
/// property and not the array.
///
/// Ancients are a subclass rather than a flag, and nine of the 68 are one. They
/// are dumped alongside with <c>ancient</c> set, because an act offers them
/// from a separate list — <c>AllAncients</c> — and a simulator rolling a map
/// needs to know which list an event came from.
///
/// Presentation is deliberately not dumped: button and dialogue colours, the
/// localised description and its pages, and the info options a tooltip shows.
/// </remarks>
public static class EventDump
{
    /// <summary>Neow's own option lists, which are code and not a table.</summary>
    /// <remarks>
    /// <c>GenerateInitialOptions</c> picks one curse, drops the positive that
    /// pairs with it, adds one of each of three pairs on a coin flip, shuffles,
    /// and takes two. Which relics are in which list is hard-coded in the
    /// event, so reflection is the only way to read it without retyping it.
    ///
    /// The lists are ordered because both are shuffled or indexed into.
    /// </remarks>
    private static void Neow(StringBuilder output)
    {
        EventModel neow = ModelDb.Event<MegaCrit.Sts2.Core.Models.Events.Neow>();
        foreach (string list in new[]
        {
            "CurseOptions", "PositiveOptions", "LavaRockOption", "SmallCapsuleOption",
            "NutritiousOysterOption", "StoneHumidifierOption", "NeowsTalismanOption",
            "PomanderOption",
        })
        {
            PropertyInfo property = typeof(MegaCrit.Sts2.Core.Models.Events.Neow).GetProperty(
                list, BindingFlags.Instance | BindingFlags.NonPublic)
                ?? throw new InvalidOperationException($"Neow has no {list} any more");

            object? value = property.GetValue(neow);
            IEnumerable<EventOption> options = value is EventOption one
                ? new[] { one }
                : (IEnumerable<EventOption>)value!;

            int at = 0;
            foreach (EventOption option in options)
            {
                RelicModel relic = option.Relic
                    ?? throw new InvalidOperationException($"Neow's {list} holds a relicless option");
                output.Append($"neow {list} {at} {relic.Id.Entry}\n");
                at++;
            }
        }
    }

    /// <summary>One list, numbered, in the order the game holds it.</summary>
    /// <remarks>
    /// The index is what makes the order legible. A reader cannot tell an
    /// ordered list from a sorted one by looking, and this one is load bearing.
    /// </remarks>
    private static void Ordered<T>(
        StringBuilder output, string kind, string? act, IEnumerable<T> items)
        where T : EventModel
    {
        int at = 0;
        foreach (T item in items)
        {
            string where = act == null ? "" : act + " ";
            output.Append($"{kind} {where}{at} {item.Id.Entry}\n");
            at++;
        }
    }

    public static void Run()
    {
        var output = new StringBuilder();

        output.Append(Stamp.Header("event"));
        output.Append("# act <act> <index> <event> -- the act's own order, not sorted\n");
        output.Append("# ancient <act> <index> <event> -- likewise, the act's ancients\n");
        output.Append("# sharedpool <index> <event> -- the any-act pool, in order\n");
        output.Append("# sharedancient <index> <event> -- the run's shared ancients\n");
        output.Append("# event <id> <ancient> <isShared> <isDeterministic> <layout> ");
        output.Append("<canonicalEncounter> <gated>\n");
        output.Append("# neow <list> <index> <relic> -- Neow's option lists, in order\n");

        List<EventModel> events = AllEvents();

        // An act's two lists. `AllEvents` and `AllAncients` are separate on
        // `ActModel`, and a map rolls from them separately, so they are two
        // kinds of row here rather than one with a column saying which.
        //
        // In the act's own order, and not sorted. `GenerateRooms` shuffles the
        // event list, and a shuffle is a permutation of what it was handed, so
        // sorting these first is generating a different run.
        foreach (ActModel act in ModelDb.Acts)
        {
            Ordered(output, "act", act.Id.Entry, act.AllEvents.Where(e => e != null));
            Ordered(output, "ancient", act.Id.Entry, act.AllAncients.Where(e => e != null));
        }

        Ordered(output, "sharedpool", null, ModelDb.AllSharedEvents);
        Ordered(output, "sharedancient", null, ModelDb.AllSharedAncients);

        Neow(output);

        foreach (EventModel model in events)
        {
            output.Append(string.Join(" ", new[]
            {
                "event",
                model.Id.Entry,
                Flag(model is AncientEventModel),
                Flag(model.IsShared),
                Flag(model.IsDeterministic),
                model.LayoutType.ToString(),
                model.CanonicalEncounter?.Id.Entry ?? "-",
                Flag(Overrides(model, nameof(EventModel.IsAllowed))),
            }));
            output.Append('\n');
        }

        output.Append($"# {events.Count} events\n");

        foreach (string name in Enum.GetNames(typeof(EventLayoutType)))
        {
            long value = Convert.ToInt64(Enum.Parse(typeof(EventLayoutType), name));
            output.Append($"enum EventLayoutType {value.ToString(CultureInfo.InvariantCulture)} {name}\n");
        }

        Log.Info("[eventdump] begin\n" + output);
        Log.Info("[eventdump] end");
    }

    private static string Flag(bool b) => b ? "1" : "0";

    /// <summary>
    /// Whether an event replaces a member that has a base implementation.
    /// </summary>
    /// <remarks>
    /// <c>CardDump</c>'s rule, and the right one here: <c>IsAllowed</c> returns
    /// true on <c>EventModel</c>, so overriding it is the event saying it has a
    /// condition on being offered. What the condition is takes run state and
    /// gets ported by hand; that there is one is what a map roll needs.
    /// </remarks>
    private static bool Overrides(EventModel model, string member)
    {
        MethodInfo? method = model
            .GetType()
            .GetMethods(BindingFlags.Public | BindingFlags.Instance)
            .FirstOrDefault(m => m.Name == member);

        return method != null && method.DeclaringType != typeof(EventModel);
    }

    /// <summary>
    /// Every event in the build, ancients included.
    /// </summary>
    private static List<EventModel> AllEvents()
    {
        MethodInfo get = typeof(ModelDb).GetMethod(
            "Get",
            BindingFlags.Static | BindingFlags.NonPublic,
            binder: null,
            types: new[] { typeof(Type) },
            modifiers: null)
            ?? throw new InvalidOperationException("ModelDb.Get(Type) is gone");

        var events = new List<EventModel>();
        foreach (Type t in ModelDb.AllAbstractModelSubtypes)
        {
            if (!t.IsSubclassOf(typeof(EventModel)) || t.IsAbstract)
            {
                continue;
            }

            try
            {
                events.Add((EventModel)get.Invoke(null, new object[] { t })!);
            }
            catch (Exception)
            {
                // Not registered under an id.
            }
        }

        return events.OrderBy(e => e.Id.Entry, StringComparer.Ordinal).ToList();
    }
}
