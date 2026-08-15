using System;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

namespace Sts2Headless;

/// <summary>
/// The last resort that ends a headless process: a wall-clock budget, and a
/// signal handler that does not negotiate.
/// </summary>
/// <remarks>
/// H11 ends the process when the driver <em>finishes</em>. This ends it when the
/// driver never finishes at all — a body that awaits something that never
/// completes, or a fight loop whose exit condition the game never satisfies.
/// In that state <c>Driver.Run</c>'s <c>finally</c> is unreachable, so nothing
/// inside the driver can help.
///
/// Everything here runs off Godot's main loop and touches no managed lock,
/// because the situation it exists for is precisely the one where the main loop
/// is stuck and whatever lock it holds is never coming back. That rules out
/// <c>Log</c>, <c>Console</c>, and <c>Environment.Exit</c> — the first two can
/// block on a writer the stuck thread owns, and the third runs finalizers on
/// its way out and has been observed here never to complete. What is left is
/// <c>write(2)</c> to stderr and <c>_exit(2)</c>, neither of which can block,
/// deadlock, or be intercepted.
/// </remarks>
public static class Deadline
{
    /// <summary>What <c>timeout(1)</c> reports, for the same reason.</summary>
    private const int TimedOutCode = 124;

    /// <summary>128 + SIGTERM, the shell's convention for a signalled death.</summary>
    private const int TermCode = 143;

    /// <summary>128 + SIGINT.</summary>
    private const int IntCode = 130;

    private static readonly TimeSpan DefaultBudget = TimeSpan.FromMinutes(10);

    // Held in statics so the GC cannot collect the registrations and quietly
    // unhook the handlers. A local would leave the process un-killable again
    // after the first collection, which is the bug this file exists to fix.
    private static PosixSignalRegistration? _sigterm;
    private static PosixSignalRegistration? _sigint;
    private static Thread? _clock;

    /// <summary>
    /// Starts the clock and takes over SIGTERM and SIGINT.
    /// </summary>
    /// <remarks>
    /// Called once, from <see cref="Headless.Init"/>, and only for a headless
    /// mode. A player's game must keep Godot's own signal handling, which
    /// notifies the scene tree so the game saves on the way out; hard-exiting
    /// that process on SIGTERM would lose the run on logout.
    ///
    /// The budget is <c>STS2_HEADLESS_TIMEOUT</c> in whole seconds, defaulting
    /// to ten minutes — two orders of magnitude above the ~3s a full selftest
    /// costs, so it can only fire on a genuine hang. <c>0</c> turns the clock
    /// off for a session that is meant to outlive it; the signal handlers stay
    /// either way, since those cost nothing and are what make an external
    /// <c>timeout</c> or <c>pkill</c> work at all.
    /// </remarks>
    public static void Arm()
    {
        CatchSignals();

        TimeSpan budget = Budget();
        if (budget <= TimeSpan.Zero)
        {
            return;
        }

        // IsBackground, so the watchdog can never be the reason the process
        // stays up -- the opposite of its job.
        _clock = new Thread(() =>
        {
            Thread.Sleep(budget);
            Die(
                TimedOutCode,
                $"[headless] deadline: no exit after {budget.TotalSeconds:F0}s, killing this process. "
                + "Something the driver awaited never completed. Raise or disable the budget with "
                + "STS2_HEADLESS_TIMEOUT (seconds, 0 for none).");
        })
        {
            IsBackground = true,
            Name = "sts2-headless-deadline",
        };
        _clock.Start();
    }

    private static TimeSpan Budget()
    {
        string? raw = Environment.GetEnvironmentVariable("STS2_HEADLESS_TIMEOUT");
        if (string.IsNullOrWhiteSpace(raw))
        {
            return DefaultBudget;
        }

        // A budget nobody can parse is a budget nobody has. Falling back to the
        // default rather than to "no limit" keeps a typo from being the thing
        // that leaks the process.
        if (!int.TryParse(raw, out int seconds) || seconds < 0)
        {
            Write($"[headless] deadline: cannot read STS2_HEADLESS_TIMEOUT '{raw}', using the default\n");
            return DefaultBudget;
        }

        return TimeSpan.FromSeconds(seconds);
    }

    /// <summary>
    /// Makes SIGTERM and SIGINT fatal.
    /// </summary>
    /// <remarks>
    /// Godot answers both by asking its main loop to close, which is exactly the
    /// thread that is not answering. That is why instances here survived
    /// <c>pkill</c> for thirteen hours, why <c>timeout</c> without <c>-k</c>
    /// leaves the process running, and why a hung instance also refuses to die
    /// during system shutdown — systemd sends SIGTERM to everything, waits, and
    /// eventually gives up on a process that will never answer.
    /// </remarks>
    private static void CatchSignals()
    {
        _sigterm = PosixSignalRegistration.Create(PosixSignal.SIGTERM, ctx =>
        {
            ctx.Cancel = true;
            Die(TermCode, "[headless] SIGTERM: exiting now rather than through the scene tree\n");
        });

        _sigint = PosixSignalRegistration.Create(PosixSignal.SIGINT, ctx =>
        {
            ctx.Cancel = true;
            Die(IntCode, "[headless] SIGINT: exiting now rather than through the scene tree\n");
        });
    }

    /// <summary>
    /// Says why, then goes, using nothing that can block.
    /// </summary>
    private static void Die(int code, string why)
    {
        Write(why.EndsWith('\n') ? why : why + "\n");
        LibcExit(code);
    }

    /// <summary>Straight to stderr, around every buffer and lock in the runtime.</summary>
    private static void Write(string message)
    {
        byte[] bytes = Encoding.UTF8.GetBytes(message);
        LibcWrite(2, bytes, bytes.Length);
    }

    // _exit, not exit: no atexit handlers, no static destructors, no flush of a
    // stdio buffer whose lock the stuck thread may hold. The process is gone
    // when this returns, which it does not.
    [DllImport("libc", EntryPoint = "_exit")]
    private static extern void LibcExit(int status);

    [DllImport("libc", EntryPoint = "write")]
    private static extern nint LibcWrite(int fd, byte[] buffer, nint count);
}
