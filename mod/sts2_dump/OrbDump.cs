using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Reflection;
using System.Text;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;
using MegaCrit.Sts2.Core.Multiplayer.Serialization;

namespace Sts2Dump;

/// <summary>
/// Every orb: its two values, when its passive fires, and where it sits in the
/// list a random orb is drawn from.
/// </summary>
/// <remarks>
/// An orb is almost entirely behaviour — <c>Passive</c> and <c>Evoke</c> pick
/// targets, deal damage, grant block or energy, and they get ported by hand.
/// Three things around them are table, and one of them is a trap.
///
/// <b>The trap.</b> There are two lists of orbs in this build and they are not
/// the same list. <c>ModelDb.Orbs</c> holds four and is used by nothing that
/// plays; <c>OrbModel.GetRandomOrb</c> draws from a private <c>_validOrbs</c>
/// holding five, and that is what <c>Chaos</c> and <c>TrashToTreasurePower</c>
/// roll against. Generating from <c>ModelDb.Orbs</c> would quietly make Chaos
/// never channel a Glass orb. Both memberships are dumped, as a column each, so
/// the difference is visible rather than a choice someone made once.
///
/// <c>_validOrbs</c> is dumped with its index because <c>GetRandomOrb</c> is
/// <c>rng.NextItem(_validOrbs)</c>: the order of that array is which orb a given
/// roll produces, so it is part of the RNG contract and not a listing order.
///
/// <b>When the passive fires.</b> Both triggers are <c>virtual</c> and do
/// nothing by default, so which one an orb overrides is the whole statement —
/// and it is a whole turn of difference in when the value arrives.
///
/// <b>The values.</b> <c>PassiveVal</c> and <c>EvokeVal</c> are
/// <c>abstract</c>, so <c>CardDump</c>'s does-it-override rule says nothing, and
/// they cannot simply be read either: most of them run through
/// <c>ModifyOrbValue</c>, which asks a hook, which reads <c>Owner</c> — and a
/// canonical model has no owner, so reading the property throws. So they are
/// decoded from the getter's IL the way <c>Il</c> decodes an integer, with the
/// shapes a <c>decimal</c> takes.
///
/// A getter is one of three: a literal, a field the orb keeps, or an
/// expression over other values. The first two both yield a number, and
/// <c>stored</c> says which — for a field, what is dumped is its initialiser.
/// The third yields <c>dynamic</c>, and only Glass's evoke is one.
///
/// <c>dynamic</c> is not "a value that changes", and <c>stored</c> is not
/// "special". Glass's evoke is <c>PassiveVal * 2</c> and Plasma's evoke is
/// <c>2m</c>, which is the same relationship spelled two ways, so a caller that
/// treated the two differently would be reading MegaCrit's source rather than
/// the game. That is why the Rust side takes the number and leaves
/// <c>stored</c> to a test.
///
/// <c>hooked</c> is beside that and orthogonal to it: this getter runs
/// <c>ModifyOrbValue</c>, so Focus is applied at this level.
///
/// Presentation is deliberately not dumped: the darkened colour, the icon and
/// sprite paths, the sound events, and the localised title and description.
/// </remarks>
public static class OrbDump
{
    public static void Run()
    {
        var output = new StringBuilder();

        output.Append(Stamp.Header("orb"));
        output.Append("# orb <id> <trigger> <randomIndex> <listed>\n");
        output.Append("# value <orb> <which> <amount> <hooked> <stored>\n");

        List<OrbModel> orbs = AllOrbs();
        List<string> random = ValidOrbs();
        List<string> listed = ModelDb.Orbs.Select(o => o.Id.Entry).ToList();

        foreach (OrbModel orb in orbs)
        {
            string id = orb.Id.Entry;

            output.Append(string.Join(" ", new[]
            {
                "orb",
                id,
                Trigger(orb),
                random.IndexOf(id).ToString(CultureInfo.InvariantCulture),
                listed.Contains(id) ? "1" : "0",
            }));
            output.Append('\n');

            output.Append($"value {id} passive {Value(orb, nameof(OrbModel.PassiveVal))}\n");
            output.Append($"value {id} evoke {Value(orb, nameof(OrbModel.EvokeVal))}\n");
        }

        output.Append($"# {orbs.Count} orbs, {random.Count} rollable, {listed.Count} listed\n");

        Log.Info("[orbdump] begin\n" + output);
        Log.Info("[orbdump] end");
    }

    /// <summary>
    /// When an orb's passive fires, by which trigger it overrides.
    /// </summary>
    /// <remarks>
    /// An orb that overrode both would be a shape this dump does not describe,
    /// and it says so rather than picking one.
    /// </remarks>
    private static string Trigger(OrbModel orb)
    {
        bool end = Overrides(orb, nameof(OrbModel.BeforeTurnEndOrbTrigger));
        bool start = Overrides(orb, nameof(OrbModel.AfterTurnStartOrbTrigger));

        return (end, start) switch
        {
            (true, false) => "turnEnd",
            (false, true) => "turnStart",
            (false, false) => "none",
            (true, true) => throw new InvalidOperationException(
                $"{orb.Id.Entry} triggers at both ends of a turn; this dump needs rewriting"),
        };
    }

    /// <summary>
    /// Whether an orb replaces a member with a base implementation.
    /// </summary>
    private static bool Overrides(OrbModel orb, string member)
    {
        MethodInfo? method = orb
            .GetType()
            .GetMethods(BindingFlags.Public | BindingFlags.Instance)
            .FirstOrDefault(m => m.Name == member);

        return method != null && method.DeclaringType != typeof(OrbModel);
    }

    /// <summary>
    /// One value's three columns: the amount, whether Focus reaches it, and
    /// whether the number lives in a field the orb keeps.
    /// </summary>
    private static string Value(OrbModel orb, string member)
    {
        MethodInfo method = Il.Getter(orb.GetType(), member)
            ?? throw new InvalidOperationException(
                $"OrbModel.{member} is gone; this dump needs rewriting");

        if (!Decimal(method, orb, out decimal amount, out bool hooked, out bool stored))
        {
            return "dynamic 0 0";
        }

        return string.Join(" ", new[]
        {
            amount.ToString(CultureInfo.InvariantCulture),
            hooked ? "1" : "0",
            stored ? "1" : "0",
        });
    }

    // The opcodes the shapes below are made of.
    private const byte Ldarg0 = 0x02;
    private const byte LdcI40 = 0x16;
    private const byte LdcI48 = 0x1E;
    private const byte LdcI4S = 0x1F;
    private const byte LdcI4 = 0x20;
    private const byte Call = 0x28;
    private const byte Ret = 0x2A;
    private const byte Newobj = 0x73;
    private const byte Ldfld = 0x7B;
    private const byte Ldsfld = 0x7E;

    /// <summary>
    /// Whether a <c>decimal</c> getter is a number this dump can name.
    /// </summary>
    /// <remarks>
    /// A body is a receiver, a value, an optional hook call and a return, and a
    /// whitelist for the same reason <c>Il.Constant</c> is one: an unfamiliar
    /// body is <c>dynamic</c> rather than assumed constant.
    ///
    /// <code>
    ///   [ldarg.0]  ldc.i4.N; newobj Decimal(int32)  [call ModifyOrbValue]  ret
    ///   [ldarg.0]  ldsfld Decimal::One              [call ModifyOrbValue]  ret
    ///   [ldarg.0]  ldarg.0; ldfld F                 [call ModifyOrbValue]  ret
    /// </code>
    ///
    /// The leading <c>ldarg.0</c> is the receiver the hook call needs, which is
    /// why only a hooked body carries one — a getter that pushed the orb and
    /// never used it would be doing something this does not follow, so the
    /// receiver and the call have to agree.
    ///
    /// The second line is not the first with a smaller number in it: the C#
    /// compiler spells <c>1m</c>, <c>0m</c> and <c>-1m</c> as the static fields
    /// <c>Decimal.One</c>, <c>Decimal.Zero</c> and <c>Decimal.MinusOne</c>
    /// rather than boxing a literal, so Plasma's passive of 1 reads nothing like
    /// its evoke of 2.
    ///
    /// Every token is resolved rather than assumed: a <c>newobj</c> of something
    /// other than <c>Decimal(int32)</c>, a <c>ldsfld</c> of something that is
    /// not a constant on <c>Decimal</c>, or a <c>call</c> to something other
    /// than <c>ModifyOrbValue</c>, is a body this does not understand.
    ///
    /// A stored value is read off the canonical instance, which has never been
    /// in a fight, so what comes back is the field's initialiser — where the
    /// orb's own slot begins.
    /// </remarks>
    private static bool Decimal(
        MethodInfo method,
        OrbModel orb,
        out decimal value,
        out bool hooked,
        out bool stored)
    {
        value = 0;
        hooked = false;
        stored = false;

        byte[]? il = method.GetMethodBody()?.GetILAsByteArray();
        if (il == null || il.Length < 2)
        {
            return false;
        }

        // A body that reads a field opens with the field's own receiver, so the
        // spare one in front of it is what tells the hooked shapes apart.
        int at = 0;
        bool receiver = il[0] == Ldarg0 && il[1] != Ldfld;
        if (receiver)
        {
            at++;
        }

        if (at + 6 <= il.Length && il[at] == Ldarg0 && il[at + 1] == Ldfld)
        {
            FieldInfo? field = ResolveField(method, Token(il, at + 2));
            if (field == null)
            {
                return false;
            }

            value = Convert.ToDecimal(field.GetValue(orb), CultureInfo.InvariantCulture);
            stored = true;
            at += 6;
        }
        else if (Literal(il, ref at, out int literal))
        {
            // An integer literal is an int until it is boxed into a decimal.
            if (at + 5 > il.Length || il[at] != Newobj || !IsDecimalCtor(method, Token(il, at + 1)))
            {
                return false;
            }

            value = literal;
            at += 5;
        }
        else if (at + 5 <= il.Length && il[at] == Ldsfld)
        {
            FieldInfo? one = ResolveField(method, Token(il, at + 1));
            if (one == null || one.DeclaringType != typeof(decimal) || !one.IsStatic)
            {
                return false;
            }

            value = (decimal)one.GetValue(null)!;
            at += 5;
        }
        else
        {
            return false;
        }

        if (at + 5 <= il.Length && il[at] == Call)
        {
            if (!IsModifyOrbValue(method, Token(il, at + 1)))
            {
                return false;
            }

            at += 5;
            hooked = true;
        }

        if (receiver != hooked)
        {
            return false;
        }

        return at + 1 == il.Length && il[at] == Ret;
    }

    /// <summary>
    /// The three shapes an integer literal takes, advancing past whichever it
    /// is. The same three <c>Il.Constant</c> knows, without the trailing
    /// <c>ret</c> — here a literal is the middle of a body rather than all of
    /// it.
    /// </summary>
    private static bool Literal(byte[] il, ref int at, out int value)
    {
        value = 0;

        if (at < il.Length && il[at] >= LdcI40 && il[at] <= LdcI48)
        {
            value = il[at] - LdcI40;
            at++;
            return true;
        }

        if (at + 2 <= il.Length && il[at] == LdcI4S)
        {
            value = (sbyte)il[at + 1];
            at += 2;
            return true;
        }

        if (at + 5 <= il.Length && il[at] == LdcI4)
        {
            value = BitConverter.ToInt32(il, at + 1);
            at += 5;
            return true;
        }

        return false;
    }

    private static int Token(byte[] il, int at) => BitConverter.ToInt32(il, at);

    private static bool IsDecimalCtor(MethodInfo context, int token)
    {
        MethodBase? resolved = ResolveMethod(context, token);

        return resolved is ConstructorInfo ctor
            && ctor.DeclaringType == typeof(decimal)
            && ctor.GetParameters().Length == 1
            && ctor.GetParameters()[0].ParameterType == typeof(int);
    }

    private static bool IsModifyOrbValue(MethodInfo context, int token)
    {
        MethodBase? resolved = ResolveMethod(context, token);

        return resolved != null
            && resolved.Name == "ModifyOrbValue"
            && resolved.DeclaringType == typeof(OrbModel);
    }

    private static MethodBase? ResolveMethod(MethodInfo context, int token)
    {
        try
        {
            return context.Module.ResolveMethod(token);
        }
        catch (Exception)
        {
            return null;
        }
    }

    private static FieldInfo? ResolveField(MethodInfo context, int token)
    {
        try
        {
            return context.Module.ResolveField(token);
        }
        catch (Exception)
        {
            return null;
        }
    }

    /// <summary>
    /// The orbs <c>GetRandomOrb</c> draws from, in the order it indexes them.
    /// </summary>
    /// <remarks>
    /// Private, because nothing outside <c>OrbModel</c> is meant to know which
    /// orbs are rollable — nothing except a simulator rolling them.
    /// </remarks>
    private static List<string> ValidOrbs()
    {
        FieldInfo field = typeof(OrbModel).GetField(
            "_validOrbs",
            BindingFlags.Static | BindingFlags.NonPublic)
            ?? throw new InvalidOperationException(
                "OrbModel._validOrbs is gone; this dump needs rewriting");

        var ids = (ModelId[])field.GetValue(null)!;

        return ids.Select(id => id.Entry).ToList();
    }

    private static List<OrbModel> AllOrbs()
    {
        MethodInfo get = typeof(ModelDb).GetMethod(
            "Get",
            BindingFlags.Static | BindingFlags.NonPublic,
            binder: null,
            types: new[] { typeof(Type) },
            modifiers: null)
            ?? throw new InvalidOperationException("ModelDb.Get(Type) is gone");

        var orbs = new List<OrbModel>();
        foreach (Type t in ModelDb.AllAbstractModelSubtypes)
        {
            if (!t.IsSubclassOf(typeof(OrbModel)) || t.IsAbstract)
            {
                continue;
            }

            try
            {
                orbs.Add((OrbModel)get.Invoke(null, new object[] { t })!);
            }
            catch (Exception)
            {
                // Not registered under an id.
            }
        }

        return orbs.OrderBy(o => o.Id.Entry, StringComparer.Ordinal).ToList();
    }
}
