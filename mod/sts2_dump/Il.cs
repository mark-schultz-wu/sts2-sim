using System;
using System.Reflection;

namespace Sts2Dump;

/// <summary>
/// Reading a value off a model's compiled body rather than off the model.
/// </summary>
/// <remarks>
/// The question every dump has to answer is whether a member is a constant or
/// something the game works out, and <c>CardDump</c>'s rule — does it override
/// the base — only works where there is a base implementation to inherit.
/// <c>PowerModel.Type</c> and <c>RelicModel.Rarity</c> are <c>abstract</c>, so
/// every model overrides them and that rule would mark all of them dynamic.
///
/// So this reads the getter's IL and calls a value constant only when the body
/// is literally "push a number, return". A whitelist, so an unfamiliar body is
/// dynamic rather than assumed constant — the same direction <c>CardDump</c>
/// errs in and for the same reason: marking a constant as dynamic costs one
/// method, marking a dynamic as constant costs a divergence nobody can see.
/// </remarks>
internal static class Il
{
    /// <summary>
    /// The method behind a member, whether it is written as a property or as a
    /// method. A model spells some of these as methods and
    /// most as properties, and a dump does not care which.
    /// </summary>
    public static MethodInfo? Getter(Type model, string member)
    {
        const BindingFlags flags = BindingFlags.Public | BindingFlags.Instance;

        return model.GetProperty(member, flags)?.GetGetMethod()
            ?? model.GetMethod(member, flags, binder: null, Type.EmptyTypes, modifiers: null);
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
    public static bool Constant(MethodInfo method, out int value)
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
