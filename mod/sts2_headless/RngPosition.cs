namespace Sts2Headless;

/// <summary>Where one random stream stands.</summary>
/// <remarks>
/// <para>
/// The game's own save record for a stream is <c>SerializableRng</c>, which is
/// a draw count and the generator's four words. Not every build saves both:
/// the <c>counter-rng</c> family saves the count alone, and there is no state
/// to hand over. So <see cref="State"/> is what this build's save format can
/// report, and never a fact about the stream itself.
/// </para>
/// <para>
/// The adapters under <c>Compat/</c> are the only place that difference lives.
/// Everything above them reads this.
/// </para>
/// </remarks>
internal readonly struct RngPosition
{
    /// <summary>How far the stream has been drawn.</summary>
    internal readonly int Counter;

    /// <summary>The generator's four words, or null where the build saves none.</summary>
    internal readonly ulong[]? State;

    /// <summary>A stream a build reports by its draw count alone.</summary>
    internal RngPosition(int counter)
    {
        Counter = counter;
        State = null;
    }

    /// <summary>A stream a build saves whole.</summary>
    internal RngPosition(int counter, ulong s0, ulong s1, ulong s2, ulong s3)
    {
        Counter = counter;
        State = new[] { s0, s1, s2, s3 };
    }
}
