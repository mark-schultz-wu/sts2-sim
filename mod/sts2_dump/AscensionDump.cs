using System;
using System.Globalization;
using System.Text;
using MegaCrit.Sts2.Core.Entities.Ascension;
using MegaCrit.Sts2.Core.Logging;

namespace Sts2Dump;

/// <summary>
/// The ascension levels, in order.
/// </summary>
/// <remarks>
/// Eleven named levels, and ascension is a <em>scalar</em> over them rather than
/// a set of flags: <c>AscensionManager.HasLevel</c> is
/// <c>_level &gt;= (int)level</c>, so a run at <c>Scarcity</c> also has
/// everything below it and each name is a threshold rather than a switch.
///
/// This is content and it is version-dependent, which is the reason it has a
/// dump of its own rather than being read off the source once. Builds move what
/// a level means — a number shifts, a level is inserted, an effect swaps places
/// — so a hard-coded ladder would quietly describe an older game while every
/// monster's scaled damage was generated against a newer one.
///
/// What is not here is what each level <em>does</em>. Poverty cutting gold,
/// Ascender's Bane adding a curse, Inflation raising shop prices — those are
/// hooks and gated values, so they are ported by hand, and where a level moves a
/// number the monster and encounter dumps already measure it at every level and
/// emit a <c>scale*</c> row. This dump is the ladder those rows index into.
/// </remarks>
public static class AscensionDump
{
    public static void Run()
    {
        var output = new StringBuilder();

        output.Append(Stamp.Header("ascension"));
        output.Append("# level <value> <name>\n");

        string[] names = Enum.GetNames(typeof(AscensionLevel));
        foreach (string name in names)
        {
            long value = Convert.ToInt64(Enum.Parse(typeof(AscensionLevel), name));
            output.Append($"level {value.ToString(CultureInfo.InvariantCulture)} {name}\n");
        }

        output.Append($"# {names.Length} levels\n");

        Log.Info("[ascensiondump] begin\n" + output);
        Log.Info("[ascensiondump] end");
    }
}
