using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Reflection;
using System.Text;
using MegaCrit.Sts2.Core.Entities.Powers;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer.Serialization;

namespace Sts2Dump;

/// <summary>
/// Prints every power's static data, as ground truth for the simulator's
/// generated tables.
/// </summary>
/// <remarks>
/// Static means: true of the power with no combat running. Not what the power
/// does — that is hooks, and hooks get ported by hand.
///
/// The interesting part is telling a constant apart from a computed value, and
/// <c>CardDump</c>'s rule is no use here. That one marks any override at all,
/// which works because <c>CardModel.Type</c> has a real implementation to
/// inherit. <c>PowerModel.Type</c> and <c>StackType</c> are <c>abstract</c>, so
/// every one of the game's powers overrides them and the rule would mark all of
/// them dynamic.
///
/// So this reads the getter's IL instead and marks a value constant only when
/// the body is literally "push a number, return" — see <see cref="Constant"/>.
/// A whitelist, so an unfamiliar body is dynamic rather than assumed constant,
/// which is the same direction <c>CardDump</c> errs in and for the same reason:
/// marking a constant as dynamic costs one method, marking a dynamic as
/// constant costs a divergence nobody can see.
///
/// It matters more than it looks. <c>ShrinkPower.StackType</c> is
/// <c>Counter</c> or <c>Single</c> depending on whether its amount is negative,
/// <c>MonologuePower.StackType</c> reads a dynamic var, and the three
/// <c>Temporary*Power</c> bases flip <c>Type</c> between buff and debuff on the
/// sign of their amount. Read off a bare model, all five would report whatever
/// they happen to be at zero.
/// </remarks>
public static class PowerDump
{
    public static void Run()
    {
        var output = new StringBuilder();

        // Stamped, so regenerating against a different build fails visibly
        // rather than quietly replacing the table.
        output.Append($"# power data, contentHash {ModelIdSerializationCache.Hash}\n");
        output.Append("# id type instance stack allowNegative removedOnOwnerDeath ");
        output.Append("ownerDeathIsFatal scalesInMultiplayer ownerIsSecondaryEnemy\n");

        List<PowerModel> powers = ModelDb.AllPowers
            .OrderBy(p => p.Id.Entry, StringComparer.Ordinal)
            .ToList();

        foreach (PowerModel power in powers)
        {
            output.Append(string.Join(" ", new[]
            {
                "power",
                power.Id.Entry,
                Cell(power, nameof(PowerModel.Type)),
                Cell(power, nameof(PowerModel.InstanceType)),
                Cell(power, nameof(PowerModel.StackType)),
                Cell(power, nameof(PowerModel.AllowNegative)),
                Cell(power, nameof(PowerModel.ShouldPowerBeRemovedAfterOwnerDeath)),
                Cell(power, nameof(PowerModel.ShouldOwnerDeathTriggerFatal)),
                Cell(power, nameof(PowerModel.ShouldScaleInMultiplayer)),
                Cell(power, nameof(PowerModel.OwnerIsSecondaryEnemy)),
            }));
            output.Append('\n');
        }

        output.Append($"# {powers.Count} powers\n");

        // The enums the power fields draw from, in declaration order.
        //
        // Reflected rather than inferred from the values above, for the same
        // reason the card dump does it: a member no power happens to use is
        // still a member, and order is the discriminant.
        foreach (Type e in new[]
        {
            typeof(PowerType), typeof(PowerInstanceType), typeof(PowerStackType),
        })
        {
            foreach (string name in Enum.GetNames(e))
            {
                long value = Convert.ToInt64(Enum.Parse(e, name));
                output.Append($"enum {e.Name} {value.ToString(CultureInfo.InvariantCulture)} {name}\n");
            }
        }

        Log.Info("[powerdump] begin\n" + output);
        Log.Info("[powerdump] end");
    }

    /// <summary>
    /// One column: the value, or <c>dynamic</c> where the power works it out.
    /// </summary>
    /// <remarks>
    /// The constant is decoded from the IL and then read back off the model, so
    /// the two have to agree. Decoding alone would trust this file's opcode
    /// table; reading alone would trust that nothing on a bare model lies. A
    /// power whose value is dynamic is never invoked at all, which is also why
    /// this cannot trip over a canonical model refusing to be touched.
    /// </remarks>
    private static string Cell(PowerModel power, string member)
    {
        MethodInfo? method = Getter(power.GetType(), member);
        if (method == null)
        {
            throw new InvalidOperationException(
                $"PowerModel.{member} is gone; this dump needs rewriting against whatever replaced it");
        }

        if (!Constant(method, out int decoded))
        {
            return "dynamic";
        }

        object? read = method.Invoke(power, Array.Empty<object>());
        string value = read switch
        {
            bool b => b ? "1" : "0",
            Enum e => e.ToString(),
            _ => throw new InvalidOperationException(
                $"{power.Id.Entry}.{member} is a {read?.GetType().Name ?? "null"}, which this dump has no column for"),
        };

        long actual = Convert.ToInt64(read, CultureInfo.InvariantCulture);
        if (actual != decoded)
        {
            throw new InvalidOperationException(
                $"{power.Id.Entry}.{member}: the IL says {decoded} and the model says {actual}");
        }

        return value;
    }

    /// <summary>
    /// The method behind a member, whether it is written as a property or as a
    /// method. <c>PowerModel</c> spells the death rules as methods and
    /// everything else as properties.
    /// </summary>
    private static MethodInfo? Getter(Type power, string member)
    {
        const BindingFlags flags = BindingFlags.Public | BindingFlags.Instance;

        return power.GetProperty(member, flags)?.GetGetMethod()
            ?? power.GetMethod(member, flags, binder: null, Type.EmptyTypes, modifiers: null);
    }

    /// <summary>
    /// Whether a body is literally "push a number, return".
    /// </summary>
    /// <remarks>
    /// The three shapes a constant compiles to: <c>ldc.i4.0</c> through
    /// <c>ldc.i4.8</c> as single opcodes, <c>ldc.i4.s</c> with a byte, and
    /// <c>ldc.i4</c> with four; each followed by <c>ret</c>. Every enum and
    /// bool this dump reads fits in the first shape, but the other two cost a
    /// line each and their absence would be an unexplained gap.
    ///
    /// Anything longer reads something — a field, a property, a branch — and is
    /// dynamic by definition. That is what makes a whitelist right here: the
    /// question is not "does this look complicated" but "can I prove it is
    /// not".
    /// </remarks>
    private static bool Constant(MethodInfo method, out int value)
    {
        value = 0;

        byte[]? il = method.GetMethodBody()?.GetILAsByteArray();
        if (il == null)
        {
            return false;
        }

        const byte ldc_i4_0 = 0x16;
        const byte ldc_i4_8 = 0x1E;
        const byte ldc_i4_s = 0x1F;
        const byte ldc_i4 = 0x20;
        const byte ret = 0x2A;

        if (il.Length == 2 && il[0] >= ldc_i4_0 && il[0] <= ldc_i4_8 && il[1] == ret)
        {
            value = il[0] - ldc_i4_0;
            return true;
        }

        if (il.Length == 3 && il[0] == ldc_i4_s && il[2] == ret)
        {
            value = (sbyte)il[1];
            return true;
        }

        if (il.Length == 6 && il[0] == ldc_i4 && il[5] == ret)
        {
            value = BitConverter.ToInt32(il, 1);
            return true;
        }

        return false;
    }
}
