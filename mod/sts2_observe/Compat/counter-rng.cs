using System.Collections.Generic;

namespace Sts2Observe;

/// <summary>The id tables as this family keeps them.</summary>
internal static class ObserveApi
{
    /// <summary>
    /// The names a saved property is numbered by on the wire, in their numbers'
    /// order, and how many bits a number takes. This family keeps them in
    /// <c>SavedPropertiesTypeCache</c>, out of reach but for reflection.
    /// </summary>
    internal static string SavedPropertyNames()
    {
        var cache = typeof(MegaCrit.Sts2.Core.Saves.Runs.SavedPropertiesTypeCache);
        var names = (List<string>)cache
            .GetField("_netIdToPropertyNameMap", System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Static)!
            .GetValue(null)!;
        var text = new System.Text.StringBuilder("PROPERTIES\n");
        for (int i = 0; i < names.Count; i++)
        {
            text.Append(i).Append(": ").Append(names[i]).Append('\n');
        }
        return text
            .Append($"PropertyIdBitSize: {MegaCrit.Sts2.Core.Saves.Runs.SavedPropertiesTypeCache.NetIdBitSize}\n")
            .ToString();
    }
}
