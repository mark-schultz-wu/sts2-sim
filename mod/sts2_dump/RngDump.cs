using System;
using System.Globalization;
using System.Text;
using MegaCrit.Sts2.Core.Helpers;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Random;

namespace Sts2Dump;

/// <summary>
/// Prints what the game's <c>Rng</c> produces, as ground truth for the
/// simulator's own.
/// </summary>
/// <remarks>
/// The simulator has to reproduce this exactly or nothing downstream can be
/// compared: a shuffle differing by one draw makes every later observation
/// disagree for reasons unrelated to whatever was under test.
///
/// <c>Rng</c> rather than <c>MegaRandom</c>, because <c>Rng</c> is what gameplay
/// draws from and it is not a pass-through — <c>Rng.NextBool</c> is
/// <c>Next(2) == 0</c>, a scaled double against zero, where
/// <c>MegaRandom.NextBool</c> reads the top bit. Only the raw draw is taken from
/// the generator, since that is the single primitive underneath.
///
/// Bit patterns, not decimals: a decimal rendering would accept a simulator
/// wrong in the last ulp, which is the error that survives for hours and then
/// diverges a shuffle.
///
/// Counters are dumped beside each value. Every method should cost exactly one
/// draw, and the rejection loops two per attempt — how far the counter moves is
/// part of what has to be reproduced.
/// </remarks>
public static class RngDump
{
    private static readonly uint[] Seeds = { 0u, 1u, 42u, 4242424u };

    private const int Draws = 16;

    public static void Run()
    {
        var output = new StringBuilder();
        output.Append("# Rng ground truth. Regenerate with STS2_HEADLESS_MODE=rngdump.\n");
        output.Append("# columns: method seed [bound...] index value [counter]\n");

        foreach (uint seed in Seeds)
        {
            // The one primitive. No counter: a bare generator has none.
            var raw = new MegaRandom(seed);
            for (int i = 0; i < Draws; i++)
            {
                output.Append($"raw {seed} {i} {Hex(raw.NextULong())}\n");
            }

            Block(output, "bool", seed, r => r.NextBool() ? "1" : "0");
            Block(output, "double", seed, r => Hex(Bits(r.NextDouble())));

            // 3 and 5 are not powers of two, so the truncation bias shows; 1 is
            // the degenerate case; int.MaxValue is the widest single argument.
            foreach (int bound in new[] { 1, 2, 3, 5, 100, int.MaxValue })
            {
                Bounded(output, seed, bound);
            }

            // The second span is wider than int.MaxValue, taking the long path.
            Range(output, seed, -7, 7);
            Range(output, seed, int.MinValue, int.MaxValue);

            Block(output, "uint", seed, r => r.NextUnsignedInt(3u, 99u).ToString(CultureInfo.InvariantCulture));
            Block(output, "float", seed, r => Hex(BitConverter.SingleToUInt32Bits(r.NextFloat(2f, 5f))));
            Block(output, "dblrange", seed, r => Hex(Bits(r.NextDouble(-3.5, 7.25))));

            // Non-default parameters on purpose. With mean 0, stdDev 1 and a
            // 0..1 range every scaling step is an identity, so a fixture built
            // on the defaults cannot tell `mean + x` from `mean - x`.
            Block(output, "gauss", seed, r => Hex(Bits(r.NextGaussianDouble(0.4, 0.3, 10.0, 20.0))));

            // Tight bounds, so the rejection loop actually rejects.
            Block(output, "gaussint", seed, r => r.NextGaussianInt(10, 3, 9, 11).ToString(CultureInfo.InvariantCulture));

            // Advancing eight primitive draws must land on the ninth.
            Rng forwarded = GameApi.NewRngAtDraw(seed, 8);
            output.Append($"forward {seed} 8 0 {forwarded.NextInt(1000)} {GameApi.Counter(forwarded)}\n");
        }

        Hashes(output);

        Log.Info("[rngdump] begin\n" + output);
        Log.Info("[rngdump] end");
    }

    /// <summary>
    /// Stream identity: the game seeds a stream by mixing in the hash of a
    /// name, so a wrong hash draws from a real stream that is the wrong one.
    /// The seed a named stream lands on is what actually matters, so that is
    /// dumped alongside the hash itself.
    /// </summary>
    private static void Hashes(StringBuilder output)
    {
        string[] samples =
        {
            "", "a", "ab", "abc", "STRIKE_IRONCLAD", "DEFEND_IRONCLAD", "BASH",
            "GORGET", "BOWLBUGS_WEAK", "shuffleRng", "aiRng", "cardRandomRng",
            // Odd length, non-ASCII, and a repeat: the loop steps two at a time
            // and branches on the last one.
            "abcde", "é", "ééé",
        };

        foreach (string s in samples)
        {
            output.Append($"hash {StringHelper.GetDeterministicHashCode(s)} "
                + $"{GameApi.NamedSeed(1000u, s)} {Escape(s)}\n");
        }
    }

    private static void Block(StringBuilder output, string method, uint seed, Func<Rng, string> draw)
    {
        var rng = new Rng(seed);
        for (int i = 0; i < Draws; i++)
        {
            output.Append($"{method} {seed} {i} {draw(rng)} {GameApi.Counter(rng)}\n");
        }
    }

    private static void Bounded(StringBuilder output, uint seed, int bound)
    {
        var rng = new Rng(seed);
        for (int i = 0; i < Draws; i++)
        {
            output.Append($"int {seed} {bound} {i} {rng.NextInt(bound)} {GameApi.Counter(rng)}\n");
        }
    }

    private static void Range(StringBuilder output, uint seed, int min, int max)
    {
        var rng = new Rng(seed);
        for (int i = 0; i < Draws; i++)
        {
            output.Append($"range {seed} {min} {max} {i} {rng.NextInt(min, max)} {GameApi.Counter(rng)}\n");
        }
    }

    private static string Escape(string s)
    {
        var sb = new StringBuilder();
        foreach (char c in s)
        {
            sb.Append(((int)c).ToString("x4", CultureInfo.InvariantCulture));
        }
        return sb.Length == 0 ? "-" : sb.ToString();
    }

    private static ulong Bits(double value) => (ulong)BitConverter.DoubleToInt64Bits(value);

    private static string Hex(ulong value) => value.ToString("x16", CultureInfo.InvariantCulture);

    private static string Hex(uint value) => value.ToString("x8", CultureInfo.InvariantCulture);
}
