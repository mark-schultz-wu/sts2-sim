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
    public static void Run()
    {
        var output = new StringBuilder();

        output.Append($"# event data, contentHash {ModelIdSerializationCache.Hash}\n");
        output.Append("# act <act> <event>\n");
        output.Append("# sharedpool <event>\n");
        output.Append("# event <id> <ancient> <isShared> <isDeterministic> <layout> ");
        output.Append("<canonicalEncounter> <gated>\n");

        List<EventModel> events = AllEvents();

        // An act's two lists. `AllEvents` and `AllAncients` are separate on
        // `ActModel`, and a map rolls from them separately, so the row says
        // which act offers an event and the `ancient` column says from where.
        foreach (ActModel act in ModelDb.Acts)
        {
            var offered = act.AllEvents
                .Cast<EventModel>()
                .Concat(act.AllAncients)
                .Where(e => e != null)
                .Select(e => e.Id.Entry)
                .Distinct()
                .OrderBy(id => id, StringComparer.Ordinal);

            foreach (string id in offered)
            {
                output.Append($"act {act.Id.Entry} {id}\n");
            }
        }

        foreach (string id in ModelDb.AllSharedEvents
            .Cast<EventModel>()
            .Concat(ModelDb.AllSharedAncients)
            .Select(e => e.Id.Entry)
            .Distinct()
            .OrderBy(id => id, StringComparer.Ordinal))
        {
            output.Append($"sharedpool {id}\n");
        }

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
