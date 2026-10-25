using System.Collections.Generic;

namespace Sts2Observe;

/// <summary>The id tables as this family keeps them.</summary>
internal static class ObserveApi
{
    /// <summary>
    /// How many bits a saved property's number takes on the wire. This family
    /// lists the names themselves in <c>ModelIdSerializationCache.Dump</c>.
    /// </summary>
    internal static string SavedPropertyNames() =>
        $"PropertyIdBitSize: {MegaCrit.Sts2.Core.Multiplayer.Serialization.ModelIdSerializationCache.PropertyIdBitSize}\n";
}
