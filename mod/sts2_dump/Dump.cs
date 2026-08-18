using System;
using System.Threading.Tasks;
using MegaCrit.Sts2.Core.Modding;
using Sts2Headless;

namespace Sts2Dump;

/// <summary>
/// Prints the game's own tables so the simulator can be generated from them
/// rather than from someone reading the decompiled source and typing numbers.
/// </summary>
/// <remarks>
/// One mode per kind of content, each claiming its own
/// <c>STS2_HEADLESS_MODE</c>. They share a mod rather than a mode because they
/// share everything else: none of them drives the game, none asserts anything,
/// and all of them want the model database populated and nothing more.
///
/// Every dump stamps <c>ModelIdSerializationCache.Hash</c> in its header, and
/// the Rust side refuses a table whose hash does not match the version module
/// it is generating into. That is the whole safety story: a dump taken against
/// another build fails loudly instead of quietly replacing a table.
///
/// Output goes to the log, between <c>[&lt;mode&gt;] begin</c> and
/// <c>[&lt;mode&gt;] end</c> markers, because a mod writing files into the
/// game directory is a worse habit than a caller running `awk`.
/// </remarks>
[ModInitializer(nameof(Init))]
public static class Dump
{
    public static void Init()
    {
        string? mode = Environment.GetEnvironmentVariable("STS2_HEADLESS_MODE");

        Action? dump = mode switch
        {
            "actdump" => ActDump.Run,
            "afflictiondump" => AfflictionDump.Run,
            "ascensiondump" => AscensionDump.Run,
            "carddump" => CardDump.Run,
            "characterdump" => CharacterDump.Run,
            "enchantmentdump" => EnchantmentDump.Run,
            "encounterdump" => EncounterDump.Run,
            "eventdump" => EventDump.Run,
            "hookdump" => HookDump.Run,
            "modifierdump" => ModifierDump.Run,
            "monsterdump" => MonsterDump.Run,
            "orbdump" => OrbDump.Run,
            "potiondump" => PotionDump.Run,
            "powerdump" => PowerDump.Run,
            "relicdump" => RelicDump.Run,
            "rngdump" => RngDump.Run,
            _ => null,
        };

        if (dump == null)
        {
            return;
        }

        // Not awaited: Init must return so startup can finish populating the
        // model database, which the driver's first stage waits for.
        _ = Driver.Run(_ =>
        {
            dump();
            return Task.CompletedTask;
        });
    }
}
