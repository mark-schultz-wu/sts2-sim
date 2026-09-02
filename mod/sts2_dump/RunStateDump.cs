using System;
using System.Globalization;
using System.Text;
using MegaCrit.Sts2.Core.Combat;
using MegaCrit.Sts2.Core.Entities.Cards;
using MegaCrit.Sts2.Core.Entities.Players;
using MegaCrit.Sts2.Core.Entities.Rngs;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Map;

namespace Sts2Dump;

/// <summary>
/// The enums a run's <em>state</em> is reported in, as opposed to the content it
/// is made of.
/// </summary>
/// <remarks>
/// Every other dump answers "what content does this build have" -- every card,
/// every relic, and a row of static facts for each. This one answers a different
/// question: what vocabulary does the game describe a position in. Which kind of
/// point you are standing on, whose side a creature fights on, what phase the
/// turn is in, why a card cannot be played, which random stream a draw came from.
///
/// None of them is a property of a card or a monster, so none of them belongs in
/// a content domain. They are here because an observation cannot be typed
/// without them: a snapshot names all six, and a stringly-typed snapshot cannot
/// be compared against a simulator's.
///
/// Reflected rather than collected from a run, for the reason the card dump
/// gives about its own enums, and which a captured run demonstrates: 514
/// observations of one Ironclad turned up 7 of the 15 <c>IntentType</c> members
/// and 2 of the 7 <c>UnplayableReason</c> flags. A member nothing happened to
/// produce is still a member.
/// </remarks>
public static class RunStateDump
{
    public static void Run()
    {
        var output = new StringBuilder();
        output.Append(Stamp.Header("run state"));
        output.Append("# enum <name> <discriminant> <member>\n");
        output.Append("# flags <name> -- the name is a [Flags] set, not one choice\n");

        // Declaration order, because that is the discriminant. UnplayableReason
        // is a [Flags] enum and its members are bit values rather than a count:
        // it skips 1, so a generated enum numbering members 0..n would be wrong
        // in a way nothing else would catch.
        foreach (Type e in new[]
        {
            typeof(MapPointType), typeof(CombatSide), typeof(PlayerTurnPhase),
            typeof(UnplayableReason), typeof(RunRngType), typeof(PlayerRngType),
        })
        {
            if (Attribute.IsDefined(e, typeof(FlagsAttribute)))
            {
                output.Append($"flags {e.Name}\n");
            }

            foreach (string name in Enum.GetNames(e))
            {
                output.Append(
                    $"enum {e.Name} "
                    + Convert.ToInt64(Enum.Parse(e, name)).ToString(CultureInfo.InvariantCulture)
                    + $" {name}\n");
            }
        }

        Log.Info("[runstatedump] begin\n" + output);
        Log.Info("[runstatedump] end");
    }
}
