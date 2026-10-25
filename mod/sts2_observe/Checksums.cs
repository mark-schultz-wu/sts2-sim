using System;
using System.IO;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using MegaCrit.Sts2.Core.Entities.Multiplayer;
using MegaCrit.Sts2.Core.Multiplayer.Game;
using MegaCrit.Sts2.Core.Multiplayer.Serialization;

namespace Sts2Observe;

/// <summary>
/// The game's own desync checksums, and the bytes each one hashes, when a run
/// asks for them.
/// </summary>
/// <remarks>
/// <c>STS2_HEADLESS_CHECKSUMS</c> names a file. Each checksum
/// <c>ChecksumTracker</c> takes goes there as one line: its id, the checksum,
/// its context, and the bytes of <c>NetFullCombatState</c> it hashed, in hex.
/// The bytes are written as <c>ChecksumTracker.GenerateChecksum</c> writes
/// them, so a port of the encoding compares byte for byte.
///
/// Off unless the variable is set, so a recorded transcript is the same either
/// way.
/// </remarks>
public static class Checksums
{
    private static StreamWriter? _out;
    private static bool _opened;
    private static readonly PacketWriter Writer = new();
    private static readonly JsonSerializerOptions Json = new()
    {
        IncludeFields = true,
        Converters = { new JsonStringEnumConverter() },
        ReferenceHandler = ReferenceHandler.IgnoreCycles,
        MaxDepth = 16,
    };

    internal static void Write(NetChecksumData data, string context, NetFullCombatState state)
    {
        if (!_opened)
        {
            _opened = true;
            string? path = Environment.GetEnvironmentVariable("STS2_HEADLESS_CHECKSUMS");
            if (path != null)
            {
                _out = new StreamWriter(path, append: false, new UTF8Encoding(false)) { AutoFlush = true };
                File.WriteAllText(path + ".ids", ModelIdSerializationCache.Dump()
                    + $"EntryIdBitSize: {ModelIdSerializationCache.EntryIdBitSize}\n\n"
                    + ObserveApi.SavedPropertyNames());
            }
        }
        if (_out == null)
        {
            return;
        }
        Writer.Reset();
        Writer.Write(state);
        Writer.ZeroByteRemainder();
        string bytes = Convert.ToHexString(Writer.Buffer, 0, Writer.BytePosition);
        string json;
        try
        {
            json = JsonSerializer.Serialize(state, Json);
        }
        catch (Exception error)
        {
            json = $"\"unserializable: {error.GetType().Name}: {error.Message.Replace('\t', ' ').Replace('\n', ' ')}\"";
        }
        _out.WriteLine($"{data.id}\t{data.checksum}\t{context.Replace('\t', ' ')}\t{bytes}\t{json}");
    }
}
