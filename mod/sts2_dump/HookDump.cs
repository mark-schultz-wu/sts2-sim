using System;
using System.Collections.Generic;
using System.Globalization;
using System.Linq;
using System.Reflection;
using System.Runtime.CompilerServices;
using System.Text;
using MegaCrit.Sts2.Core.Hooks;
using MegaCrit.Sts2.Core.Logging;
using MegaCrit.Sts2.Core.Models;

namespace Sts2Dump;

/// <summary>
/// Every hook a model can answer, and every model that answers one.
/// </summary>
/// <remarks>
/// Mostly not content. It is the shape of the interface between an engine and
/// the things that react to it, and it is dumped for two reasons a table cannot
/// serve. One part is also a table: which hooks each power's class overrides,
/// which the simulator reads so that an event is asked only of the powers that
/// answer it.
///
/// <b>The trait surface is too wide to design by hand.</b>
/// <c>AbstractModel</c> declares over 180 virtual hooks. Writing a Rust trait
/// from a reading of the source means guessing which ones matter and finding out
/// years later that one was missed. Dumped, it is a checklist: a test can say
/// "this build overrides a hook your trait has no slot for" and name it.
///
/// <b>It is the porting checklist.</b> Whoever ports a card wants to know which
/// hooks that card answers. <c>declaredBy</c> is what makes that usable: a model
/// inherits its base class's overrides, so <c>implements STRIKE X CardModel</c>
/// means every card answers X and Strike's own code does not, while
/// <c>implements STRIKE X Strike</c> means Strike itself does something.
///
/// <c>category</c> is the id's own, which tells apart two models that share an
/// entry. The relic <c>PAELS_LEGION</c> answers six hooks, and the pet monster
/// of the same entry answers none.
///
/// <c>dispatchers</c> is which classes fire the hook, and it is measured by
/// metadata token rather than by name. Name-matching against <c>Hook</c>'s
/// public methods was the obvious approach and it under-reports badly: it misses
/// 43 hooks that models plainly answer, because <c>Hook.ModifyDamage</c>
/// dispatches both <c>ModifyDamageAdditive</c> and
/// <c>ModifyDamageMultiplicative</c>, and several others fire from
/// <c>CombatManager</c> or the helpers rather than from <c>Hook</c> at all.
///
/// So every method body in the assembly is scanned for a <c>call</c> or
/// <c>callvirt</c> whose operand is one of these hooks' metadata tokens. That is
/// a byte scan rather than a full IL walk, which in principle could match a
/// token-shaped run of bytes inside another instruction's operand; in practice
/// the four bytes have to equal one of 183 specific tokens, so a false positive
/// would be a coincidence rather than a hazard. A hook nothing dispatches is a
/// different kind of dead from one nothing overrides, and worth telling apart.
///
/// Only hooks declared on <c>AbstractModel</c> are here. A kind-specific virtual
/// like <c>OrbModel.Passive</c> is that kind's own business and gets ported with
/// it; this is the surface every model shares and <c>Hook</c> dispatches.
///
/// What is deliberately not dumped is the order hooks fire in.
/// <c>CombatState.IterateHookListeners</c> is explicit code — allies then
/// enemies, powers before the creature's own model, relics in slot order, then
/// potions — and hooks fold, so the order is semantics. It is forty lines to
/// port and reading it as data would lose that.
/// </remarks>
public static class HookDump
{
    public static void Run()
    {
        var output = new StringBuilder();

        output.Append(Stamp.Header("hook"));
        output.Append("# hook <name> <returns> <params> <dispatchers>\n");
        output.Append("# implements <model> <hook> <declaredBy> <category>\n");
        output.Append("# unregistered <type> <hook>\n");

        List<MethodInfo> hooks = Hooks();
        Dictionary<string, SortedSet<string>> dispatchers = Dispatchers(hooks);

        foreach (MethodInfo hook in hooks)
        {
            SortedSet<string> from = dispatchers[hook.Name];

            output.Append(string.Join(" ", new[]
            {
                "hook",
                hook.Name,
                Short(hook.ReturnType),
                Parameters(hook),
                from.Count == 0 ? "-" : string.Join(",", from),
            }));
            output.Append('\n');
        }

        var names = new HashSet<string>(hooks.Select(h => h.Name), StringComparer.Ordinal);
        int answered = 0;

        // A concrete model with no id is invisible to every other dump, which
        // all swallow the same lookup failure. Here it would be a hole in a
        // completeness claim, so it is counted rather than assumed empty.
        List<Type> unregistered = Unregistered();
        int stranded = 0;
        foreach (Type type in unregistered)
        {
            foreach (string hook in Answers(type, names))
            {
                output.Append($"unregistered {type.Name} {hook}\n");
                stranded++;
            }
        }

        foreach (AbstractModel model in AllModels())
        {
            // `GetBaseDefinition` walks an override back to where it was first
            // declared, which is what tells a hook apart from a method that
            // merely shares a name, and handles overloads without ambiguity.
            foreach (MethodInfo m in Overrides(model.GetType(), names))
            {
                output.Append(
                    $"implements {model.Id.Entry} {m.Name} {m.DeclaringType!.Name} {model.Id.Category}\n");
                answered++;
            }
        }

        int fired = dispatchers.Values.Count(d => d.Count > 0);

        output.Append(
            $"# {hooks.Count} hooks, {fired} dispatched, "
            + $"{answered.ToString(CultureInfo.InvariantCulture)} answers, "
            + $"{unregistered.Count} unregistered types answering {stranded}\n");

        Log.Info("[hookdump] begin\n" + output);
        Log.Info("[hookdump] end");
    }

    /// <summary>
    /// Every hook <c>AbstractModel</c> declares.
    /// </summary>
    /// <remarks>
    /// Three filters, each removing something that is virtual and overridden
    /// and still not a hook.
    ///
    /// <c>IsSpecialName</c> drops property accessors: a virtual property like
    /// <c>ShouldReceiveCombatHooks</c> is configuration, not an event, and it
    /// compiles to a <c>get_</c> method that would otherwise land here.
    ///
    /// A base definition outside <c>AbstractModel</c> drops the object protocol
    /// — <c>ToString</c> is declared here as an override of <c>object</c>, and
    /// it turned up as the one "hook" nothing dispatched, which is how it was
    /// noticed.
    ///
    /// An interface implementation drops <c>CompareTo</c>. It is declared on
    /// <c>AbstractModel</c> and models do override it, so neither of the other
    /// two filters sees it, but it is a contract with the framework's sorting
    /// rather than something an engine fires.
    /// </remarks>
    private static List<MethodInfo> Hooks()
    {
        var contractual = new HashSet<MethodInfo>();
        foreach (Type contract in typeof(AbstractModel).GetInterfaces())
        {
            foreach (MethodInfo m in typeof(AbstractModel).GetInterfaceMap(contract).TargetMethods)
            {
                contractual.Add(m);
            }
        }

        return typeof(AbstractModel)
            .GetMethods(BindingFlags.Public | BindingFlags.Instance | BindingFlags.DeclaredOnly)
            .Where(m => m.IsVirtual
                && !m.IsFinal
                && !m.IsSpecialName
                && m.GetBaseDefinition().DeclaringType == typeof(AbstractModel)
                && !contractual.Contains(m))
            .OrderBy(m => m.Name, StringComparer.Ordinal)
            .ToList();
    }

    /// <summary>
    /// Which classes fire each hook, found by scanning the assembly for calls.
    /// </summary>
    /// <remarks>
    /// A hook is called through its base declaration — inside <c>Hook</c> the
    /// receiver is typed <c>AbstractModel</c>, so the compiler emits that
    /// method's own token rather than an override's. Matching tokens is
    /// therefore exact where matching names is not.
    ///
    /// Only <c>callvirt</c> counts, and that is what separates dispatch from
    /// inheritance. A model calling <c>base.AfterX()</c> emits a non-virtual
    /// <c>call</c> on the very same token; a dispatcher walking a listener list
    /// emits <c>callvirt</c>. Excluding model types instead would be the obvious
    /// shortcut and is wrong — <c>CardModel</c> is a model and also fires
    /// <c>AfterModifyingCardPlayResultPileOrPosition</c> at other models, so
    /// that rule hid the one hook it was meant to classify.
    /// </remarks>
    private static Dictionary<string, SortedSet<string>> Dispatchers(List<MethodInfo> hooks)
    {
        const byte callvirt = 0x6F;

        var byToken = new Dictionary<int, string>();
        var found = new Dictionary<string, SortedSet<string>>(StringComparer.Ordinal);
        foreach (MethodInfo hook in hooks)
        {
            byToken[hook.MetadataToken] = hook.Name;
            found[hook.Name] = new SortedSet<string>(StringComparer.Ordinal);
        }

        const BindingFlags any = BindingFlags.Public | BindingFlags.NonPublic
            | BindingFlags.Instance | BindingFlags.Static | BindingFlags.DeclaredOnly;

        foreach (Type type in typeof(AbstractModel).Assembly.GetTypes())
        {
            foreach (MethodBase method in type.GetMethods(any).Cast<MethodBase>()
                .Concat(type.GetConstructors(any)))
            {
                byte[]? il;
                try
                {
                    il = method.GetMethodBody()?.GetILAsByteArray();
                }
                catch (Exception)
                {
                    // Abstract, extern, or otherwise bodiless.
                    continue;
                }

                if (il == null)
                {
                    continue;
                }

                for (int at = 0; at + 5 <= il.Length; at++)
                {
                    if (il[at] != callvirt)
                    {
                        continue;
                    }

                    if (byToken.TryGetValue(BitConverter.ToInt32(il, at + 1), out string? name))
                    {
                        found[name].Add(Enclosing(type).Name);
                    }
                }
            }
        }

        return found;
    }

    /// <summary>
    /// The type a compiler-generated one was written inside.
    /// </summary>
    /// <remarks>
    /// An <c>async</c> method becomes a nested state machine, so the call to a
    /// hook lives in <c>&lt;AfterCardPlayed&gt;d__16</c> rather than in the
    /// class anyone wrote. Reporting that name would make the column unreadable
    /// and would hide that <c>CombatManager</c> is the thing doing the firing.
    /// </remarks>
    private static Type Enclosing(Type type)
    {
        while (type.DeclaringType != null
            && type.GetCustomAttribute<CompilerGeneratedAttribute>() != null)
        {
            type = type.DeclaringType;
        }

        return type;
    }

    private static string Parameters(MethodInfo hook)
    {
        ParameterInfo[] parameters = hook.GetParameters();

        return parameters.Length == 0
            ? "-"
            : string.Join(",", parameters.Select(p => Short(p.ParameterType)));
    }

    /// <summary>
    /// A type's bare name, without namespace or generic decoration.
    /// </summary>
    /// <remarks>
    /// The names are compared between builds rather than resolved, so what
    /// matters is that they are stable and readable — a signature that changes
    /// should be visible as a changed string.
    /// </remarks>
    private static string Short(Type type)
    {
        if (type == typeof(void))
        {
            return "void";
        }

        if (!type.IsGenericType)
        {
            return type.Name;
        }

        string bare = type.Name.Split('`')[0];
        string args = string.Join(",", type.GetGenericArguments().Select(Short));

        return $"{bare}<{args}>";
    }

    /// <summary>
    /// The hooks a type overrides, most-derived declaration first.
    /// </summary>
    /// <remarks>
    /// <c>GetBaseDefinition</c> walks an override back to where it was first
    /// declared, which tells a hook apart from a method that merely shares a
    /// name and handles overloads without ambiguity.
    /// </remarks>
    private static IEnumerable<MethodInfo> Overrides(Type type, HashSet<string> names)
    {
        return type
            .GetMethods(BindingFlags.Public | BindingFlags.Instance)
            .Where(m => m.DeclaringType != typeof(AbstractModel)
                && m.GetBaseDefinition().DeclaringType == typeof(AbstractModel)
                && names.Contains(m.Name))
            .OrderBy(m => m.Name, StringComparer.Ordinal);
    }

    private static IEnumerable<string> Answers(Type type, HashSet<string> names)
    {
        return Overrides(type, names).Select(m => m.Name);
    }

    /// <summary>
    /// Concrete models the model database has no id for.
    /// </summary>
    /// <remarks>
    /// Every dump in this mod swallows the same lookup failure, so a model
    /// without an id is missing from all of them and says so nowhere. It cannot
    /// be named by id here either — there is no id — so it is reported by type,
    /// and only where it answers a hook, which is the case that would make a
    /// completeness claim wrong.
    /// </remarks>
    private static List<Type> Unregistered()
    {
        MethodInfo get = Getter();

        var stranded = new List<Type>();
        foreach (Type t in ModelDb.AllAbstractModelSubtypes)
        {
            if (t.IsAbstract)
            {
                continue;
            }

            try
            {
                get.Invoke(null, new object[] { t });
            }
            catch (Exception)
            {
                stranded.Add(t);
            }
        }

        return stranded.OrderBy(t => t.Name, StringComparer.Ordinal).ToList();
    }

    private static MethodInfo Getter()
    {
        return typeof(ModelDb).GetMethod(
            "Get",
            BindingFlags.Static | BindingFlags.NonPublic,
            binder: null,
            types: new[] { typeof(Type) },
            modifiers: null)
            ?? throw new InvalidOperationException("ModelDb.Get(Type) is gone");
    }

    private static List<AbstractModel> AllModels()
    {
        MethodInfo get = Getter();

        var models = new List<AbstractModel>();
        foreach (Type t in ModelDb.AllAbstractModelSubtypes)
        {
            if (t.IsAbstract)
            {
                continue;
            }

            try
            {
                models.Add((AbstractModel)get.Invoke(null, new object[] { t })!);
            }
            catch (Exception)
            {
                // Not registered under an id.
            }
        }

        return models.OrderBy(m => m.Id.Entry, StringComparer.Ordinal).ToList();
    }
}
