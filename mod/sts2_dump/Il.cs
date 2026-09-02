using System;
using System.Collections.Generic;
using System.Reflection;
using System.Reflection.Emit;

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
    /// Whether a body is literally "push a string, return".
    /// </summary>
    /// <remarks>
    /// The same whitelist as <see cref="Constant"/>, for the members whose
    /// value is a name rather than a number: <c>ldstr</c> with a metadata token,
    /// then <c>ret</c>. A getter that built its string from anything else is not
    /// a constant this can read, and says so.
    /// </remarks>
    public static bool Text(MethodInfo method, out string value)
    {
        value = string.Empty;

        byte[]? il = method.GetMethodBody()?.GetILAsByteArray();
        if (il == null)
        {
            return false;
        }

        const byte ldstr = 0x72;
        const byte ret = 0x2A;

        if (il.Length != 6 || il[0] != ldstr || il[5] != ret)
        {
            return false;
        }

        value = method.Module.ResolveString(BitConverter.ToInt32(il, 1));
        return true;
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

    /// <summary>
    /// The string pushed for the first argument of each <c>newobj</c> of
    /// <paramref name="target"/> in a body.
    /// </summary>
    /// <remarks>
    /// For the ids the game builds at runtime rather than declaring —
    /// <c>new MoveState("STUNNED", …)</c> — which no walk of the declared data
    /// can reach. The last <c>ldstr</c> before the <c>newobj</c> is that
    /// argument: the remaining arguments are delegates and intents, which push
    /// no strings, and a construction where something else did is reported as
    /// null rather than guessed at.
    ///
    /// A real instruction walk, not a byte scan. Operand widths come from
    /// <c>OpCodes</c> itself, so an opcode this does not expect cannot be
    /// mistaken for data and shift everything after it.
    /// </remarks>
    public static List<string?> Constructed(MethodBase method, Type target, out int unresolved)
    {
        var found = new List<string?>();
        unresolved = 0;

        byte[]? il;
        try
        {
            il = method.GetMethodBody()?.GetILAsByteArray();
        }
        catch (Exception)
        {
            // A body the runtime will not hand over -- a generic definition it
            // cannot instantiate, chiefly. Nothing to read, and nothing to
            // claim about it.
            return found;
        }

        if (il == null)
        {
            return found;
        }

        Type[]? typeArgs = method.DeclaringType is { IsGenericType: true } t
            ? t.GetGenericArguments()
            : null;
        Type[]? methodArgs = method.IsGenericMethodDefinition ? method.GetGenericArguments() : null;

        string? pushed = null;

        for (int i = 0; i < il.Length;)
        {
            short code = il[i];
            i++;
            if (code == 0xFE && i < il.Length)
            {
                code = (short)(0xFE00 | il[i]);
                i++;
            }

            if (!Opcodes.TryGetValue(code, out OpCode op))
            {
                // An opcode this does not know means every offset after it is a
                // guess, so stop reading this body rather than read noise.
                unresolved++;
                return found;
            }

            int operand = Width(op, il, i);
            if (operand < 0 || i + operand > il.Length)
            {
                unresolved++;
                return found;
            }

            if (op == OpCodes.Ldstr)
            {
                try
                {
                    pushed = method.Module.ResolveString(BitConverter.ToInt32(il, i));
                }
                catch (Exception)
                {
                    pushed = null;
                }
            }
            else if (op == OpCodes.Newobj || op == OpCodes.Call)
            {
                Type? built = null;
                try
                {
                    MethodBase? called = method.Module
                        .ResolveMethod(BitConverter.ToInt32(il, i), typeArgs, methodArgs);

                    // `call` as well as `newobj`, for a constructor chaining to
                    // another of its own: `MoveState()` is `this("UNSET_MOVE",
                    // UnsetMove)`, so the only place that id is written is a
                    // `call` the caller never sees. A plain call to something
                    // else is not a construction and is skipped by the same
                    // test the `newobj` uses.
                    if (called is ConstructorInfo || op == OpCodes.Newobj)
                    {
                        built = called?.DeclaringType;
                    }
                }
                catch (Exception)
                {
                    unresolved++;
                }

                if (built == target)
                {
                    // Shaped like an id, or nothing. Arguments evaluate left to
                    // right, so the id is the last string pushed -- but a
                    // construction whose id is a field or a parameter would
                    // otherwise pick up whatever string came before it, and
                    // saying "unreadable" is the only honest answer there.
                    found.Add(Id(pushed) ? pushed : null);

                    // Spent: the next construction needs its own.
                    pushed = null;
                }
            }

            i += operand;
        }

        return found;
    }

    /// <summary>Whether a string is shaped like one of the game's ids.</summary>
    /// <remarks>
    /// <c>StringHelper.Slugify</c> ends in a <c>[^A-Z0-9_]</c> strip, so an id
    /// is exactly those characters. Anything else that reached here is a string
    /// belonging to some other argument.
    /// </remarks>
    private static bool Id(string? text)
    {
        if (string.IsNullOrEmpty(text))
        {
            return false;
        }

        foreach (char c in text!)
        {
            if (!(c is (>= 'A' and <= 'Z') or (>= '0' and <= '9') or '_'))
            {
                return false;
            }
        }

        return true;
    }

    /// <summary>How many bytes of operand an instruction carries.</summary>
    private static int Width(OpCode op, byte[] il, int at) => op.OperandType switch
    {
        OperandType.InlineNone => 0,
        OperandType.ShortInlineBrTarget or OperandType.ShortInlineI or OperandType.ShortInlineVar => 1,
        OperandType.InlineVar => 2,
        OperandType.InlineBrTarget or OperandType.InlineField or OperandType.InlineI
            or OperandType.InlineMethod or OperandType.InlineSig or OperandType.InlineString
            or OperandType.InlineTok or OperandType.InlineType or OperandType.ShortInlineR => 4,
        OperandType.InlineI8 or OperandType.InlineR => 8,
        OperandType.InlineSwitch => at + 4 <= il.Length
            ? 4 + (4 * BitConverter.ToInt32(il, at))
            : -1,
        _ => -1,
    };

    /// <summary>Every opcode by its value, off <c>OpCodes</c> itself.</summary>
    private static readonly Dictionary<short, OpCode> Opcodes = BuildOpcodes();

    private static Dictionary<short, OpCode> BuildOpcodes()
    {
        var all = new Dictionary<short, OpCode>();
        foreach (FieldInfo field in typeof(OpCodes).GetFields(BindingFlags.Public | BindingFlags.Static))
        {
            if (field.GetValue(null) is OpCode op)
            {
                all[op.Value] = op;
            }
        }

        return all;
    }
}
