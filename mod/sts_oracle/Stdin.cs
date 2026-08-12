using System;
using System.Collections.Concurrent;
using System.Threading;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Logging;

namespace StsOracle;

/// <summary>
/// Lines from standard input, readable from the game's own thread.
/// </summary>
/// <remarks>
/// The driver runs on Godot's main thread — everything it touches
/// (<c>ActionQueueSet</c>, the model layer) assumes it, and the async
/// continuations resume there because <c>GodotSynchronizationContext</c> pumps
/// them on the frame callback. Reading stdin must not break that, and
/// <c>Console.In.ReadLine</c> blocks, which on the main thread would deadlock
/// the engine.
///
/// So a dedicated thread does the blocking read and hands lines over a
/// <c>BlockingCollection</c>, and the driver polls with <c>await Task.Delay</c>
/// — the same shape as <c>Quiesce</c>, resuming on the same context. A
/// thread-pool <c>Task.Run(Console.ReadLine)</c> would post its continuation to
/// the same place, but ties up a pool thread indefinitely per read and makes
/// the shutdown story worse: a blocked pool thread cannot be abandoned, whereas
/// this one is a background thread the runtime will not wait for.
/// </remarks>
internal static class Stdin
{
    private static readonly BlockingCollection<string> Lines = new BlockingCollection<string>();
    private static volatile bool _closed;
    private static Thread? _reader;

    /// <summary>True once stdin has reached end-of-file.</summary>
    public static bool Closed => _closed && Lines.Count == 0;

    public static void Start()
    {
        if (_reader != null)
        {
            return;
        }

        _reader = new Thread(Pump)
        {
            IsBackground = true,
            Name = "oracle-stdin",
        };
        _reader.Start();
    }

    private static void Pump()
    {
        try
        {
            while (true)
            {
                string? line = Console.In.ReadLine();
                if (line == null)
                {
                    break;
                }

                Lines.Add(line);
            }
        }
        catch (Exception e)
        {
            // Never silently: a reader that dies leaves the driver waiting for
            // input that can no longer arrive, which looks like a hang.
            Log.Error($"[oracle] stdin reader failed: {e}");
        }
        finally
        {
            _closed = true;
        }
    }

    /// <summary>
    /// The next line, or null once stdin is closed and drained.
    /// </summary>
    public static async Task<string?> ReadLine()
    {
        while (true)
        {
            if (Lines.TryTake(out string? line))
            {
                return line;
            }

            if (_closed)
            {
                return null;
            }

            await Task.Delay(1);
        }
    }
}
