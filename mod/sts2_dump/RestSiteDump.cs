using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Reflection;
using System.Text;
using MegaCrit.Sts2.Core.Entities.RestSite;
using MegaCrit.Sts2.Core.Logging;

namespace Sts2Dump;

/// <summary>
/// The options a rest site can offer, by the name each one answers to.
/// </summary>
/// <remarks>
/// A rest site option is not a model. It has no id in the model database, no
/// row in <c>ModelDb</c>, and no dump found it until now — <c>OptionId</c> is an
/// <c>abstract string</c> and each subclass returns a literal. So an observation
/// reports <c>SMITH</c> and the Rust side had nothing to turn that into but a
/// <c>String</c>, which is the one thing a typed snapshot must not contain: a
/// misspelled option and an option this build does not have read the same.
///
/// Names and nothing else, because there is nothing else that is table. What an
/// option does is <c>OnSelect</c>, which is code; whether it is offered is
/// <c>IsEnabled</c> and the <c>ModifyRestSiteOptions</c> hook, which are a
/// position rather than a fact about the build. <c>RestSiteOption.Generate</c>
/// makes two of these itself and a third in multiplayer, and every other one
/// arrives because a relic added it — that is behaviour, and the observation
/// reports the result of it.
///
/// Found by walking the assembly rather than by asking <c>Generate</c>: the
/// options a run can see depend on its relics, and a list built from one run
/// would be that run's list. Every concrete subclass is every option the build
/// has.
///
/// Read off the getter's IL rather than off an instance. Constructing one needs
/// a <c>Player</c>, and there is no run — a fabricated owner would be a second
/// thing that could be wrong about the answer.
/// </remarks>
public static class RestSiteDump
{
    public static void Run()
    {
        var output = new StringBuilder();
        output.Append(Stamp.Header("rest site"));
        output.Append("# option <id>\n");

        List<string> ids = Options();

        foreach (string id in ids)
        {
            output.Append($"option {id}\n");
        }

        output.Append(
            "# " + ids.Count.ToString(CultureInfo.InvariantCulture) + " options\n");

        Log.Info("[restsitedump] begin\n" + output);
        Log.Info("[restsitedump] end");
    }

    /// <summary>
    /// Every concrete <c>RestSiteOption</c>'s id, sorted.
    /// </summary>
    /// <remarks>
    /// Sorted rather than in reflection order, which is the assembly's and not
    /// the game's: nothing indexes these, so there is no order to preserve, and
    /// a stable one keeps a re-taken dump byte-for-byte identical.
    /// </remarks>
    private static List<string> Options()
    {
        var ids = new List<string>();

        foreach (Type t in typeof(RestSiteOption).Assembly.GetTypes())
        {
            if (!t.IsSubclassOf(typeof(RestSiteOption)) || t.IsAbstract)
            {
                continue;
            }

            MethodInfo getter = Il.Getter(t, nameof(RestSiteOption.OptionId))
                ?? throw new InvalidOperationException(
                    $"{t.Name} has no OptionId; this dump needs rewriting");

            if (!Il.Text(getter, out string id))
            {
                throw new InvalidOperationException(
                    $"{t.Name}.OptionId is not a literal; this dump needs rewriting");
            }

            ids.Add(id);
        }

        if (ids.Count == 0)
        {
            throw new InvalidOperationException(
                "no RestSiteOption subclasses; this dump needs rewriting");
        }

        ids.Sort(StringComparer.Ordinal);

        List<string> repeated = ids
            .GroupBy(id => id)
            .Where(g => g.Count() > 1)
            .Select(g => g.Key)
            .ToList();

        if (repeated.Count > 0)
        {
            throw new InvalidOperationException(
                $"two options answer to {string.Join(", ", repeated)}, "
                + "so the name is not an identity; this dump needs rewriting");
        }

        return ids;
    }
}
