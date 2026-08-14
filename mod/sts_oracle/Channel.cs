using System;
using System.IO;
using System.Net.Sockets;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Logging;

namespace StsOracle;

/// <summary>
/// A line-oriented duplex channel to whoever is driving the oracle.
/// </summary>
/// <remarks>
/// This exists as an interface because the transport is the least settled part
/// of R10 and the easiest to get wrong permanently. Everything above it — the
/// command vocabulary, the observation, the version handshake — is unaffected
/// by how bytes move, so the choice stays reversible as long as nothing above
/// this line knows which implementation it has.
///
/// The one thing every implementation must get right is flushing on write. A
/// buffered response is indistinguishable from a hung oracle: the client blocks
/// on a line that is sitting in a 4KB buffer, and the timeout that eventually
/// fires blames the wrong component.
/// </remarks>
/// <summary>
/// A loopback TCP connection back to the process that launched us.
/// </summary>
/// <remarks>
/// The oracle dials out rather than listening: the client binds
/// <c>127.0.0.1:0</c>, gets an ephemeral port from the OS and passes it in the
/// environment. That removes the port-allocation race entirely — nobody has to
/// pick a free number and hope — and it means the client is listening before
/// the game is launched, so there is no connect-retry loop.
///
/// TCP over fd inheritance for one reason above the others: it needs no
/// <c>unsafe</c>, no <c>dup2</c> and no platform-specific spawn code on the
/// client side, and it is full duplex on a single descriptor, so the peer
/// disappearing is one detectable event rather than two half-failures. The cost
/// is a loopback roundtrip of roughly 30µs against a ~4ms decision, which R8
/// measures rather than assumes.
///
/// There was an <c>IChannel</c> interface here with a stdio implementation
/// beside this one, kept because "the transport is the least settled part of
/// R10". The transport settled: stdio was unusable by anything but a human with
/// grep — the engine writes ~140 unprefixed lines to stdout before the oracle
/// says anything — and no client ever ran without a port. One implementation
/// behind an interface is a guess about a second one, so both are gone.
///
/// The one thing to get right is flushing on write. A buffered response is
/// indistinguishable from a hung oracle: the client blocks on a line sitting in
/// a 4KB buffer, and the timeout that eventually fires blames the wrong
/// component.
/// </remarks>
internal sealed class Channel
{
    private readonly TcpClient _client;
    private readonly StreamReader _in;
    private readonly StreamWriter _out;

    private Channel(TcpClient client)
    {
        _client = client;
        NetworkStream s = client.GetStream();
        _in = new StreamReader(s);
        _out = new StreamWriter(s) { AutoFlush = true };
    }

    /// <summary>A short description, for the log line that says how we are connected.</summary>
    public string Describe => $"tcp {_client.Client.RemoteEndPoint}";

    /// <summary>
    /// Dials the port the client passed in.
    /// </summary>
    /// <remarks>
    /// Selection is by environment rather than command line because Godot owns
    /// argv.
    /// </remarks>
    public static async Task<Channel> Open()
    {
        string? port = Environment.GetEnvironmentVariable("STS_ORACLE_PORT");
        if (string.IsNullOrEmpty(port) || !int.TryParse(port, out int p) || p is < 1 or > 65535)
        {
            throw new ArgumentException(
                $"STS_ORACLE_PORT must be the port the client is listening on; got '{port}'");
        }

        var client = new TcpClient();

        // Nagle batches small writes waiting for more; every message here is a
        // small write followed by a wait for the reply, which is the exact
        // pattern it penalises.
        client.NoDelay = true;

        await client.ConnectAsync("127.0.0.1", p);
        var channel = new Channel(client);

        // A loopback listener accepts from any local process. The token turns
        // "something else connected first" from a baffling protocol error into
        // a stated one — and it is the client, not us, that must reject a
        // mismatch, so we simply present it.
        string? token = Environment.GetEnvironmentVariable("STS_ORACLE_TOKEN");
        if (!string.IsNullOrEmpty(token))
        {
            await channel.WriteLine(token);
        }

        return channel;
    }

    /// <summary>The next line, or null once the peer has gone away.</summary>
    public async Task<string?> ReadLine() => await _in.ReadLineAsync();

    /// <summary>Writes one line and flushes it before returning.</summary>
    public async Task WriteLine(string line) => await _out.WriteLineAsync(line);
}
