using System.Collections.Generic;
using MegaCrit.Sts2.Core.Logging;

namespace StsOracle;

/// <summary>
/// Errors the game logged while handling a command.
/// </summary>
/// <remarks>
/// The game catches most of its own exceptions and logs them rather than letting
/// them escape, so an action can fail completely while the driver sees it
/// complete normally. That is not hypothetical: granting a card to hand without
/// registering it with the <c>CombatState</c> made
/// <c>CardModel.OnPlayWrapper</c> throw "must be added to a CombatState before
/// playing it", the game swallowed it, the energy was still spent — and the
/// protocol answered <c>"resolved": true</c> with a clean observation. The
/// client had no way to know the decision had not happened.
///
/// That is the exact failure R12 exists to prevent, and no amount of care in the
/// driver would have caught it, because nothing in the driver's view was wrong.
/// So the game's own error stream is captured and attached to whatever response
/// the command produces. The oracle does not judge which errors matter; it
/// refuses to hide any of them.
/// </remarks>
internal static class GameErrors
{
    private static readonly List<string> Seen = new List<string>();

    public static void Watch()
    {
        Log.LogCallback += OnLog;
    }

    private static void OnLog(LogLevel level, string text, int skipFrames)
    {
        if (level == LogLevel.Error)
        {
            Seen.Add(text);
        }
    }

    /// <summary>
    /// Takes and clears what has accumulated, so errors are attributed to the
    /// command they occurred under rather than to every later one.
    /// </summary>
    public static List<string> Drain()
    {
        var drained = new List<string>(Seen);
        Seen.Clear();
        return drained;
    }
}
