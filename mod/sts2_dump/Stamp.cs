using System;
using System.Globalization;
using MegaCrit.Sts2.Core.Debug;
using MegaCrit.Sts2.Core.Multiplayer.Serialization;

namespace Sts2Dump;

/// <summary>
/// The two header lines every dump opens with: what it is, and which build it
/// came from.
/// </summary>
/// <remarks>
/// Two hashes, because they answer different questions and one of them is not
/// enough.
///
/// <c>content</c> is <c>ModelIdSerializationCache.Hash</c>: XxHash32 over every
/// model's id category and entry, the epoch ids, and three counts. That is the
/// <em>set of ids</em> and nothing else. It moves when content is added, removed
/// or renamed — which is most patches, since STS2 churns 20-50 cards a minor
/// version. It does not move when a card's damage goes from 6 to 7, and it does
/// not move when a hook is added, because neither changes an id.
///
/// So a dump anchored on the content hash alone would be accepted by a build
/// that had quietly rebalanced every number in it. <c>assembly</c> closes that:
/// <c>MainAssemblyHash</c> is shipped in <c>release_info.json</c> and identifies
/// the release, so it moves on any code change at all. It also survives this
/// project's IL-patched <c>sts2.dll</c>, because it describes the release rather
/// than the file on disk.
///
/// <c>sts2_determinism</c> already refuses to start against an unpinned assembly
/// hash and <c>sts2_headless</c> already reports it in the greeting. The dumps
/// were the one place still trusting the content hash by itself.
///
/// A missing <c>release_info.json</c> throws rather than degrading to a
/// placeholder. The greeting can afford "UNRELEASED" because a live protocol
/// should not die on a cosmetic field; a dump cannot, because an unidentifiable
/// build is exactly what the header exists to refuse.
/// </remarks>
internal static class Stamp
{
    public static string Header(string what)
    {
        ReleaseInfo release = ReleaseInfoManager.Instance.ReleaseInfo
            ?? throw new InvalidOperationException(
                "no release_info.json, so the build this dump came from cannot be "
                + "identified and the dump would be unsafe to generate from");

        string assembly = release.MainAssemblyHash.ToString(CultureInfo.InvariantCulture);
        string content = ModelIdSerializationCache.Hash.ToString(CultureInfo.InvariantCulture);

        return $"# {what} data\n"
            + $"# build {release.Version} commit {release.Commit} "
            + $"assembly {assembly} content {content}\n";
    }
}
