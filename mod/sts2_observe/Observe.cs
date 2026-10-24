using HarmonyLib;
using MegaCrit.Sts2.Core.Modding;
using Sts2Headless;

namespace Sts2Observe;

/// <summary>
/// Reports what the game does inside, when a run asks: its calls, method by
/// method. Off unless asked, so a run with this loaded records what one
/// without it does.
/// </summary>
[ModInitializer(nameof(Init))]
public static class Observe
{
    public static void Init()
    {
        Driver.Loaded += () => Trace.Arm(new Harmony("sts2_observe"));
        Driver.CommandReceived += Trace.Command;
        Driver.Quitting += Trace.Finish;
    }
}
