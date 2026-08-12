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
internal interface IChannel
{
    /// <summary>The next line, or null once the peer has gone away.</summary>
    Task<string?> ReadLine();

    /// <summary>Writes one line and flushes it before returning.</summary>
    Task WriteLine(string line);

    /// <summary>A short description, for the log line that says how we are connected.</summary>
    string Describe { get; }
}

internal static class Channel
{
    /// <summary>
    /// Builds the channel the environment asks for.
    /// </summary>
    /// <remarks>
    /// TCP when <c>STS_ORACLE_PORT</c> is set, stdio otherwise. Selection is by
    /// environment rather than command line because Godot owns argv.
    /// </remarks>
    public static async Task<IChannel> Open()
    {
        string? port = Environment.GetEnvironmentVariable("STS_ORACLE_PORT");
        if (string.IsNullOrEmpty(port))
        {
            return new StdioChannel();
        }

        if (!int.TryParse(port, out int p) || p is < 1 or > 65535)
        {
            throw new ArgumentException($"STS_ORACLE_PORT is not a port number: '{port}'");
        }

        return await TcpChannel.Connect(p, Environment.GetEnvironmentVariable("STS_ORACLE_TOKEN"));
    }
}

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
/// </remarks>
internal sealed class TcpChannel : IChannel
{
    private readonly TcpClient _client;
    private readonly StreamReader _in;
    private readonly StreamWriter _out;

    private TcpChannel(TcpClient client)
    {
        _client = client;
        NetworkStream s = client.GetStream();
        _in = new StreamReader(s);
        _out = new StreamWriter(s) { AutoFlush = true };
    }

    public string Describe => $"tcp {_client.Client.RemoteEndPoint}";

    public static async Task<TcpChannel> Connect(int port, string? token)
    {
        var client = new TcpClient();

        // Nagle batches small writes waiting for more; every message here is a
        // small write followed by a wait for the reply, which is the exact
        // pattern it penalises.
        client.NoDelay = true;

        await client.ConnectAsync("127.0.0.1", port);
        var channel = new TcpChannel(client);

        // A loopback listener accepts from any local process. The token turns
        // "something else connected first" from a baffling protocol error into
        // a stated one — and it is the client, not us, that must reject a
        // mismatch, so we simply present it.
        if (!string.IsNullOrEmpty(token))
        {
            await channel.WriteLine(token);
        }

        return channel;
    }

    public async Task<string?> ReadLine() => await _in.ReadLineAsync();

    public async Task WriteLine(string line) => await _out.WriteLineAsync(line);
}

/// <summary>
/// Standard input and output, for driving the oracle by hand.
/// </summary>
/// <remarks>
/// Not usable by the simulator, and deliberately so: the engine writes 141
/// lines of its own to stdout before we emit anything — a Godot banner, FMOD,
/// the Sentry extension and an unprefixed system dump containing PATH and mount
/// names — so responses arrive interleaved with content we do not control.
/// A human with `grep` copes with that; a parser should not have to.
///
/// It earns its place because it makes the protocol testable from a shell
/// pipeline with no client written yet, which is how every command below was
/// first exercised.
/// </remarks>
internal sealed class StdioChannel : IChannel
{
    public StdioChannel() => Stdin.Start();

    public string Describe => "stdio (responses interleave with engine logs)";

    public Task<string?> ReadLine() => Stdin.ReadLine();

    public Task WriteLine(string line)
    {
        Console.Out.WriteLine(line);
        Console.Out.Flush();
        return Task.CompletedTask;
    }
}
